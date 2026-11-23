// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! the engine owns the one active test session. it receives every raw event,
//! runs the exit rules and the math, and flushes batched frames to the ui.
//! `Session` is pure (no threads, no os calls) so it can be tested with
//! synthetic input; `spawn` wraps it with the thread and the platform.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;

use crate::capture::{exit_info, ExitInfo, ExitRule, ExitTracker, SessionSettings};
use crate::devices::gamepad::{PadCmd, PadMsg};
use crate::devices::hub::Nudge;
use crate::hid::analog::AnalogProfile;
use crate::hid::descriptor::{Layout, LayoutInfo};
use crate::hid::reader::{HidReader, HidReport};
use crate::devices::{DeviceInfo, Viewer};
use crate::events::{EndReason, Ev, Frame, PadItem};
use crate::keys;
use crate::platform::{CaptureMode, CaptureReport, CaptureSpec, Platform, RawEvent, RawSink, ReservedCombo, SrcId};
use crate::timing::dpi::DpiTest;
use crate::timing::keystats::KeyTracker;
use crate::timing::poll::PollEstimator;
use crate::timing::sensor::{ButtonTracker, SensorTest};
use crate::timing::stick::{RestTest, StickTest};
use crate::timing::{now_ns, ns_to_ms};

const MS: u64 = 1_000_000;
const FLUSH_EVERY: u64 = 8 * MS;
const TICK_EVERY: u64 = 4 * MS;
const FOCUS_CHECK_EVERY: u64 = 100 * MS;
const REFRESH_CAPTURE_EVERY: u64 = 250 * MS;
/// focus can flicker while the capture is being armed
const FOCUS_GRACE: u64 = 600 * MS;
const HEARTBEAT_TIMEOUT: u64 = 10_000 * MS;
const SNAP_EVERY: u64 = 50 * MS;

pub trait FrameSink: Send {
    /// false when the receiver is gone
    fn send(&self, frame: Frame) -> bool;
}

impl FrameSink for Channel<Frame> {
    fn send(&self, frame: Frame) -> bool {
        Channel::send(self, frame).is_ok()
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub device: DeviceInfo,
    pub mode: CaptureMode,
    pub exit: ExitInfo,
    pub capture: CaptureReport,
    pub reserved: Vec<ReservedCombo>,
    pub combo_warning: Option<String>,
    pub no_capture: bool,
    pub hid: Option<HidSessionInfo>,
    pub analog: Option<AnalogInfo>,
    pub midi_error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HidSessionInfo {
    pub layout: Option<LayoutInfo>,
    /// why the device could not be read, when it could not
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnalogInfo {
    pub profile: String,
    pub verified: bool,
    pub error: Option<String>,
}

pub fn mode_for(viewer: Viewer) -> CaptureMode {
    match viewer {
        Viewer::Mouse => CaptureMode::Mouse,
        Viewer::Keyboard => CaptureMode::Keyboard,
        _ => CaptureMode::Other,
    }
}

/// everything a mouse session measures
struct MouseState {
    poll: PollEstimator,
    dpi: DpiTest,
    sensor: SensorTest,
    buttons: ButtonTracker,
    dx: i32,
    dy: i32,
    n: u32,
    last_snap: u64,
    last_reports: u64,
    idle_sent: bool,
}

impl MouseState {
    fn new(chatter_ms: f64) -> Self {
        Self { poll: PollEstimator::new(), dpi: DpiTest::new(), sensor: SensorTest::new(), buttons: ButtonTracker::new(chatter_ms), dx: 0, dy: 0, n: 0, last_snap: 0, last_reports: 0, idle_sent: true }
    }
}

/// raw report tracking shared by the generic hid view and the analog-keyboard dump
struct HidState {
    layout: Option<Layout>,
    last: Vec<u8>,
    changed: std::collections::BTreeSet<u32>,
    n: u32,
    fresh: bool,
    poll: PollEstimator,
    last_snap: u64,
    last_reports: u64,
    idle_sent: bool,
}

impl HidState {
    fn new(layout: Option<Layout>) -> Self {
        Self { layout, last: vec![], changed: Default::default(), n: 0, fresh: false, poll: PollEstimator::new(), last_snap: 0, last_reports: 0, idle_sent: true }
    }
    fn on_report(&mut self, r: &HidReport) {
        self.poll.push(r.ts);
        self.n += 1;
        if self.last.len() == r.data.len() {
            for (i, (a, b)) in self.last.iter().zip(&r.data).enumerate() {
                if a != b {
                    self.changed.insert(i as u32);
                }
            }
        } else {
            self.changed.extend(0..r.data.len() as u32);
        }
        self.last = r.data.clone();
        self.fresh = true;
    }
    fn flush(&mut self, buf: &mut Vec<Ev>) {
        if !self.fresh {
            return;
        }
        self.fresh = false;
        let d = self.layout.as_ref().map(|l| l.decode(&self.last)).unwrap_or_default();
        buf.push(Ev::Hid { id: d.id, report: self.last.clone(), changed: std::mem::take(&mut self.changed).into_iter().collect(), n: std::mem::take(&mut self.n), buttons: d.buttons, axes: d.axes });
    }
    fn tick(&mut self, now: u64, buf: &mut Vec<Ev>) {
        if now.saturating_sub(self.last_snap) >= SNAP_EVERY {
            self.last_snap = now;
            let snap = self.poll.snapshot(now);
            let moving = snap.hz.is_some();
            if snap.reports != self.last_reports || (!moving && !self.idle_sent) {
                self.idle_sent = !moving;
                self.last_reports = snap.reports;
                buf.push(Ev::Poll(snap));
            }
        }
    }
}

struct AnalogState {
    profile: Box<dyn AnalogProfile>,
    last: std::collections::HashMap<&'static str, f32>,
    raw: HidState,
}

/// everything a controller session measures
struct PadState {
    pad: usize,
    events: PollEstimator,
    hid: PollEstimator,
    hid_seen: bool,
    left: StickTest,
    right: StickTest,
    rest: RestTest,
    pending: Vec<PadItem>,
    last_snap: u64,
    last_reports: u64,
    idle_sent: bool,
    /// pads read straight from hid reports (playstation) have no gilrs events
    hid_kind: Option<crate::hid::pads::HidPad>,
    hid_last: std::collections::HashMap<&'static str, f32>,
    bluetooth: bool,
    hid_power: Option<(&'static str, Option<u8>)>,
    last_motion: u64,
    /// raw report bytes for the pad's hex view (only used for pads read over hid)
    raw: HidState,
}

fn bus_name(bluetooth: bool) -> String {
    if bluetooth { "bluetooth" } else { "usb" }.into()
}

fn hid_pad_name(k: crate::hid::pads::HidPad) -> String {
    match k {
        crate::hid::pads::HidPad::DualSense => "dualsense",
        crate::hid::pads::HidPad::DualShock4 => "dualshock4",
    }
    .into()
}

impl PadState {
    fn new(pad: usize) -> Self {
        Self { hid_kind: None, hid_last: Default::default(), bluetooth: false, hid_power: None, last_motion: 0, raw: HidState::new(None), pad, events: PollEstimator::new(), hid: PollEstimator::new(), hid_seen: false, left: StickTest::new(), right: StickTest::new(), rest: RestTest::new(), pending: vec![], last_snap: 0, last_reports: 0, idle_sent: true }
    }
    fn poll(&self) -> &PollEstimator {
        if self.hid_seen { &self.hid } else { &self.events }
    }
}

pub struct Session {
    pub device: DeviceInfo,
    pub mode: CaptureMode,
    srcs: HashSet<SrcId>,
    exit: ExitTracker,
    started: u64,
    last_heartbeat: u64,
    last_exit_progress: f32,
    all_down: HashSet<&'static str>,
    keys: KeyTracker,
    mouse: Option<MouseState>,
    pad: Option<PadState>,
    hid: Option<HidState>,
    analog: Option<AnalogState>,
    midi: bool,
    buf: Vec<Ev>,
    seq: u64,
}

impl Session {
    pub fn new(device: DeviceInfo, settings: &SessionSettings, now: u64) -> Self {
        let mode = mode_for(device.viewer);
        let (device_viewer, device_pad_id, device_vid, device_pid, device_bt) = (device.viewer, device.pad_id, device.vid, device.pid, device.bluetooth);
        Self {
            srcs: device.srcs.iter().copied().collect(),
            exit: ExitTracker::new(ExitRule::for_mode(mode, settings)),
            device,
            mode,
            started: now,
            last_heartbeat: now,
            last_exit_progress: 0.0,
            all_down: HashSet::new(),
            keys: KeyTracker::new(settings.chatter_ms),
            mouse: (mode == CaptureMode::Mouse).then(|| MouseState::new(settings.chatter_ms)),
            hid: None,
            analog: None,
            midi: device_viewer == Viewer::Midi,
            pad: (device_viewer == Viewer::Gamepad).then(|| {
                let mut p = PadState::new(device_pad_id.unwrap_or(usize::MAX));
                p.bluetooth = device_bt;
                if device_pad_id.is_none() {
                    if let (Some(v), Some(pid)) = (device_vid, device_pid) {
                        p.hid_kind = crate::hid::pads::kind_for(v, pid);
                    }
                }
                p
            }),
            buf: Vec::new(),
            seq: 0,
        }
    }

    fn rel_ms(&self, ts: u64) -> f64 {
        ns_to_ms(ts.saturating_sub(self.started))
    }

    pub fn on_raw(&mut self, ev: &RawEvent) {
        match ev {
            RawEvent::Key { src, code, down, ts } => {
                // the exit rules listen to every keyboard, not just the one under test
                self.exit.key(*src, code, *down, *ts);
                if *down {
                    self.all_down.insert(code);
                } else {
                    self.all_down.remove(code);
                }
                // src 0 is the hook fallback: it has no device identity
                if self.device.viewer == Viewer::Keyboard && (*src == 0 || self.srcs.contains(src)) {
                    if let Some(o) = self.keys.event(code, *down, *ts) {
                        self.buf.push(Ev::Key { code, down: *down, ts: self.rel_ms(*ts), hold: o.hold_ms, gap: o.gap_ms, chatter: o.chatter, held: o.held, max: o.max_held });
                    }
                }
            }
            RawEvent::MouseMove { src, dx, dy, ts, absolute } => {
                if let Some(m) = self.mouse.as_mut().filter(|_| self.srcs.contains(src)) {
                    // absolute devices (tablets, remote desktop) report positions, not counts
                    if !*absolute {
                        m.poll.push(*ts);
                        m.dpi.add(*dx, *dy);
                        m.sensor.add(*dx, *dy);
                        m.dx += *dx;
                        m.dy += *dy;
                        m.n += 1;
                    }
                }
            }
            RawEvent::MouseButton { src, button, down, ts } => {
                let rel = ns_to_ms(ts.saturating_sub(self.started));
                if let Some(m) = self.mouse.as_mut().filter(|_| *src == 0 || self.srcs.contains(src)) {
                    if let Some(o) = m.buttons.event(*button, *down, *ts) {
                        self.buf.push(Ev::Button { n: *button, down: *down, ts: rel, hold: o.hold_ms, gap: o.gap_ms, chatter: o.chatter });
                        if !*down {
                            self.buf.push(Ev::Clicks { buttons: m.buttons.stats() });
                        }
                    }
                }
            }
            RawEvent::Wheel { src, dx, dy, .. } => {
                if self.mouse.is_some() && (*src == 0 || self.srcs.contains(src)) {
                    self.buf.push(Ev::Wheel { dx: *dx, dy: *dy });
                }
            }
            RawEvent::RawBlind => self.buf.push(Ev::Notice { text: "windows is not delivering raw input to this app, so keys and clicks are being read from the system hooks instead. polling rate and movement need raw input and will be missing.".into() }),
            RawEvent::FocusLost { .. } | RawEvent::DevicesChanged => {}
        }
    }

    pub fn set_hid(&mut self, layout: Option<Layout>) {
        self.hid = Some(HidState::new(layout));
    }

    pub fn set_analog(&mut self, profile: Box<dyn AnalogProfile>) {
        self.analog = Some(AnalogState { profile, last: Default::default(), raw: HidState::new(None) });
    }

    pub fn on_midi(&mut self, ts: u64, bytes: &[u8]) {
        if !self.midi {
            return;
        }
        if let Some(m) = crate::devices::midi::decode(bytes) {
            self.buf.push(Ev::Midi { kind: m.kind.into(), channel: m.channel, a: m.a, b: m.b, value: m.value, ts: ns_to_ms(ts.saturating_sub(self.started)), text: m.text });
        }
    }

    /// (on, off) output reports for a pad read over hid, when it can rumble
    pub fn hid_rumble(&self, strong: f32, weak: f32) -> Option<(Vec<u8>, Vec<u8>)> {
        let p = self.pad.as_ref()?;
        let k = p.hid_kind?;
        Some((crate::hid::pads::rumble_report(k, p.bluetooth, strong, weak)?, crate::hid::pads::rumble_report(k, p.bluetooth, 0.0, 0.0)?))
    }

    /// (effect on, effect off) output reports for one adaptive trigger
    pub fn hid_trigger(&self, right: bool, start: f32, force: f32) -> Option<(Vec<u8>, Vec<u8>)> {
        let p = self.pad.as_ref()?;
        let k = p.hid_kind?;
        let off = crate::hid::pads::trigger_report(k, p.bluetooth, right, None)?;
        let on = if force > 0.0 { crate::hid::pads::trigger_report(k, p.bluetooth, right, Some((start, force)))? } else { off.clone() };
        Some((on, off))
    }

    pub fn hid_audio(&self, speaker: f32, mic: f32) -> Option<Vec<u8>> {
        let p = self.pad.as_ref()?;
        crate::hid::pads::audio_report(p.hid_kind?, p.bluetooth, speaker, mic)
    }

    pub fn pad_id(&self) -> Option<usize> {
        self.pad.as_ref().map(|p| p.pad)
    }

    pub fn on_pad(&mut self, msg: &PadMsg, now: u64) {
        let Some(p) = self.pad.as_mut() else { return };
        match msg {
            PadMsg::Status(s) if s.pad == p.pad => {
                self.buf.push(Ev::PadInfo { force_feedback: s.force_feedback, power: s.power.clone(), battery: s.battery, poll_source: if p.hid_seen { "hid reports".into() } else { "events".into() }, bus: String::new(), hid_pad: String::new() });
            }
            PadMsg::Event(e) if e.pad == p.pad => {
                p.events.push(e.ts);
                Self::apply_item(p, &e.name, e.code.clone(), e.value, e.axis);
                let _ = now;
            }
            _ => {}
        }
    }

    /// one control changed: update the stick and drift tests, and queue it for the ui
    fn apply_item(p: &mut PadState, name: &str, code: String, value: f32, axis: bool) {
        let stick = |t: &mut StickTest, rest: &mut RestTest, which: &str, x: Option<f32>, y: Option<f32>| {
            let (cx, cy) = t.pos();
            let (nx, ny) = (x.unwrap_or(cx), y.unwrap_or(cy));
            t.feed(nx, ny);
            rest.feed(which, true, nx, ny);
        };
        match name {
            "LeftStickX" => stick(&mut p.left, &mut p.rest, "left stick", Some(value), None),
            "LeftStickY" => stick(&mut p.left, &mut p.rest, "left stick", None, Some(value)),
            "RightStickX" => stick(&mut p.right, &mut p.rest, "right stick", Some(value), None),
            "RightStickY" => stick(&mut p.right, &mut p.rest, "right stick", None, Some(value)),
            "LeftTrigger2" | "LeftZ" => p.rest.feed("left trigger", false, value, 0.0),
            "RightTrigger2" | "RightZ" => p.rest.feed("right trigger", false, value, 0.0),
            _ => {}
        }
        let item = PadItem { name: name.to_string(), code, value, axis };
        match p.pending.iter_mut().find(|i| i.name == item.name && i.code == item.code) {
            Some(old) => *old = item,
            None => p.pending.push(item),
        }
    }

    pub fn on_hid(&mut self, r: &HidReport) {
        if let Some(h) = self.hid.as_mut() {
            h.on_report(r);
        }
        if let Some(a) = self.analog.as_mut() {
            a.raw.on_report(r);
            let now_down: std::collections::HashMap<&'static str, f32> = a.profile.parse(&r.data).into_iter().collect();
            let mut changed: Vec<(String, f32)> = vec![];
            for (k, v) in &now_down {
                if a.last.get(k).is_none_or(|o| (o - v).abs() > 0.004) {
                    changed.push((k.to_string(), *v));
                }
            }
            for k in a.last.keys() {
                if !now_down.contains_key(k) {
                    changed.push((k.to_string(), 0.0));
                }
            }
            a.last = now_down;
            if !changed.is_empty() {
                self.buf.push(Ev::Analog { keys: changed });
            }
        }
        if let Some(p) = self.pad.as_mut() {
            if !p.hid_seen {
                p.hid_seen = true;
                if let Some(k) = p.hid_kind {
                    let ff = crate::hid::pads::rumble_report(k, p.bluetooth, 0.0, 0.0).is_some();
                    self.buf.push(Ev::PadInfo { force_feedback: ff, power: "unknown".into(), battery: None, poll_source: "hid reports".into(), bus: bus_name(p.bluetooth), hid_pad: hid_pad_name(k) });
                }
                p.last_reports = 0; // switch the poll source cleanly
                p.idle_sent = false;
            }
            p.hid.push(r.ts);
            if p.hid_kind.is_some() {
                p.raw.on_report(r);
            }
            if let Some(kind) = p.hid_kind {
                if let Some(pw) = crate::hid::pads::battery(kind, p.bluetooth, &r.data) {
                    if p.hid_power != Some(pw) {
                        p.hid_power = Some(pw);
                        let ff = crate::hid::pads::rumble_report(kind, p.bluetooth, 0.0, 0.0).is_some();
                        self.buf.push(Ev::PadInfo { force_feedback: ff, power: pw.0.into(), battery: pw.1, poll_source: "hid reports".into(), bus: bus_name(p.bluetooth), hid_pad: hid_pad_name(kind) });
                    }
                }
                // gyro and accelerometer, about 60 times a second is plenty for a readout
                if r.ts.saturating_sub(p.last_motion) >= 16_000_000 {
                    if let Some(m) = crate::hid::pads::motion(kind, p.bluetooth, &r.data) {
                        p.last_motion = r.ts;
                        let g = |v: i16| v as f32 / 16.4; // about +-2000 degrees per second full scale
                        let a = |v: i16| v as f32 / 8192.0; // 8192 per g
                        self.buf.push(Ev::Motion { gyro: [g(m.gyro[0]), g(m.gyro[1]), g(m.gyro[2])], accel: [a(m.accel[0]), a(m.accel[1]), a(m.accel[2])] });
                    }
                }
                if let Some(items) = crate::hid::pads::decode(kind, p.bluetooth, &r.data) {
                    for (name, v, axis) in items {
                        let changed = p.hid_last.get(name).is_none_or(|o| (o - v).abs() > 0.002);
                        if changed {
                            p.hid_last.insert(name, v);
                            Self::apply_item(p, name, "hid".to_string(), v, axis);
                        }
                    }
                }
            }
        }
    }

    /// window lost the foreground. returns the end reason unless it is inside the arming grace.
    pub fn on_focus_lost(&mut self, now: u64) -> Option<EndReason> {
        if now.saturating_sub(self.started) < FOCUS_GRACE {
            return None;
        }
        // os-reserved combos swallow the keys and steal focus: say so
        let ctrl_alt = self.all_down.iter().any(|k| keys::is_ctrl(k)) && self.all_down.iter().any(|k| keys::is_alt(k));
        if ctrl_alt {
            self.buf.push(Ev::Reserved { label: "ctrl + alt + del".into() });
        }
        if self.all_down.iter().any(|k| keys::is_meta(k)) && self.all_down.contains("KeyL") {
            self.buf.push(Ev::Reserved { label: "win + l".into() });
        }
        Some(EndReason::FocusLost)
    }

    /// reset the per-session key stats (the "reset" button of the tested tracker)
    pub fn reset_keys(&mut self) {
        self.keys.reset();
    }

    /// ui commands for the per-session tests
    pub fn command(&mut self, cmd: SessionCommand, now: u64) {
        match cmd {
            SessionCommand::ResetKeys => self.reset_keys(),
            _ => {}
        }
        if let Some(p) = self.pad.as_mut() {
            match cmd {
                SessionCommand::PadCircleStart => { p.left.start(); p.right.start(); }
                SessionCommand::PadCircleStop => { p.left.stop(); p.right.stop(); }
                SessionCommand::PadCircleReset => { p.left.reset(); p.right.reset(); }
                SessionCommand::PadRestStart { secs } => p.rest.start(now, secs),
                _ => {}
            }
            self.buf.push(Ev::Sticks { left: p.left.snapshot(), right: p.right.snapshot(), rest: p.rest.snapshot(now) });
            return;
        }
        let Some(m) = self.mouse.as_mut() else { return };
        match cmd {
            SessionCommand::DpiStart { distance_in } => m.dpi.start(distance_in),
            SessionCommand::DpiStop => m.dpi.stop(),
            SessionCommand::DpiReset => m.dpi.reset(),
            SessionCommand::SensorStart => m.sensor.start(),
            SessionCommand::SensorStop => m.sensor.stop(),
            SessionCommand::SensorReset => m.sensor.reset(),
            SessionCommand::MouseReset => {
                m.poll.reset();
                m.buttons.reset();
                m.last_reports = 0;
                m.idle_sent = false;
            }
            _ => {}
        }
        // answer immediately so the ui never shows stale numbers after a command
        self.buf.push(Ev::Dpi(m.dpi.snapshot()));
        self.buf.push(Ev::Sensor(m.sensor.snapshot()));
        self.buf.push(Ev::Poll(m.poll.snapshot(now)));
        self.buf.push(Ev::Clicks { buttons: m.buttons.stats() });
    }

    pub fn heartbeat(&mut self, now: u64) {
        self.last_heartbeat = now;
    }

    pub fn allows_button(&self, capture_disabled: bool) -> bool {
        ExitRule::button_allowed(self.mode) || capture_disabled
    }

    /// advance timers. returns a reason when the session must end.
    pub fn tick(&mut self, now: u64) -> Option<EndReason> {
        let st = self.exit.tick(now);
        if let Some(kind) = self.exit.rule().kind() {
            if (st.progress - self.last_exit_progress).abs() >= 0.01 || (st.progress == 0.0) != (self.last_exit_progress == 0.0) {
                self.last_exit_progress = st.progress;
                self.buf.push(Ev::Exit { kind, progress: st.progress });
            }
            if st.fired {
                return Some(match kind {
                    crate::capture::ExitKind::Esc => EndReason::EscHold,
                    crate::capture::ExitKind::Combo => EndReason::Combo,
                });
            }
        }
        if now.saturating_sub(self.last_heartbeat) > HEARTBEAT_TIMEOUT {
            return Some(EndReason::Watchdog);
        }
        if let Some(m) = self.mouse.as_mut() {
            if now.saturating_sub(m.last_snap) >= SNAP_EVERY {
                m.last_snap = now;
                let snap = m.poll.snapshot(now);
                let moving = snap.hz.is_some();
                if snap.reports != m.last_reports || (!moving && !m.idle_sent) {
                    m.idle_sent = !moving;
                    m.last_reports = snap.reports;
                    self.buf.push(Ev::Poll(snap));
                }
                if m.dpi.is_counting() {
                    self.buf.push(Ev::Dpi(m.dpi.snapshot()));
                }
                if m.sensor.is_active() {
                    self.buf.push(Ev::Sensor(m.sensor.snapshot()));
                }
            }
        }
        if let Some(h) = self.hid.as_mut() {
            h.tick(now, &mut self.buf);
        }
        if let Some(a) = self.analog.as_mut() {
            a.raw.tick(now, &mut self.buf);
        }
        if let Some(p) = self.pad.as_mut() {
            let finished = p.rest.tick(now);
            if now.saturating_sub(p.last_snap) >= SNAP_EVERY {
                p.last_snap = now;
                let snap = p.poll().snapshot(now);
                let moving = snap.hz.is_some();
                if snap.reports != p.last_reports || (!moving && !p.idle_sent) {
                    p.idle_sent = !moving;
                    p.last_reports = snap.reports;
                    self.buf.push(Ev::Poll(snap));
                }
                if p.left.is_active() || p.right.is_active() || p.rest.is_running() || finished {
                    self.buf.push(Ev::Sticks { left: p.left.snapshot(), right: p.right.snapshot(), rest: p.rest.snapshot(now) });
                }
            }
        }
        None
    }

    pub fn push(&mut self, ev: Ev) {
        self.buf.push(ev);
    }

    pub fn has_pending(&self) -> bool {
        !self.buf.is_empty() || self.mouse.as_ref().is_some_and(|m| m.n > 0) || self.pad.as_ref().is_some_and(|p| !p.pending.is_empty()) || self.pad.as_ref().is_some_and(|p| p.raw.fresh) || self.hid.as_ref().is_some_and(|h| h.fresh) || self.analog.as_ref().is_some_and(|a| a.raw.fresh)
    }

    pub fn take_frame(&mut self) -> Option<Frame> {
        if let Some(m) = self.mouse.as_mut() {
            if m.n > 0 {
                let ev = Ev::Mouse { dx: m.dx, dy: m.dy, n: m.n };
                m.dx = 0;
                m.dy = 0;
                m.n = 0;
                self.buf.push(ev);
            }
        }
        if let Some(h) = self.hid.as_mut() {
            h.flush(&mut self.buf);
        }
        if let Some(a) = self.analog.as_mut() {
            a.raw.flush(&mut self.buf);
        }
        if let Some(p) = self.pad.as_mut() {
            if p.hid_kind.is_some() {
                p.raw.flush(&mut self.buf);
            }
            if !p.pending.is_empty() {
                self.buf.push(Ev::Pad { items: std::mem::take(&mut p.pending) });
            }
        }
        if self.buf.is_empty() {
            return None;
        }
        self.seq += 1;
        Some(Frame { seq: self.seq, events: std::mem::take(&mut self.buf) })
    }
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SessionCommand {
    ResetKeys,
    DpiStart { distance_in: f64 },
    DpiStop,
    DpiReset,
    SensorStart,
    SensorStop,
    SensorReset,
    MouseReset,
    PadCircleStart,
    PadCircleStop,
    PadCircleReset,
    PadRestStart { secs: f32 },
    PadRumble { strong: f32, weak: f32, ms: u32 },
    /// adaptive trigger resistance on a dualsense; force 0 switches it off
    PadTrigger { right: bool, start: f32, force: f32 },
    /// the controller's own speaker and microphone levels (usb dualsense)
    PadAudio { speaker: f32, mic: f32 },
}

pub struct StartReq {
    pub device: DeviceInfo,
    pub settings: SessionSettings,
    pub window: Option<isize>,
    pub sink: Box<dyn FrameSink>,
    pub reply: Sender<Result<SessionInfo, String>>,
}

pub enum EngineMsg {
    Raw(RawEvent),
    Pad(PadMsg),
    Hid(HidReport),
    Midi(u64, Vec<u8>),
    Start(Box<StartReq>),
    /// the user asked to leave (exit button)
    End(EndReason),
    FocusLost,
    Heartbeat,
    Command(SessionCommand),
    Shutdown,
}

#[derive(Clone)]
pub struct EngineHandle {
    pub tx: Sender<EngineMsg>,
    /// true while a session holds capture. used to veto window close.
    pub active: Arc<AtomicBool>,
}

struct Live {
    _reader: Option<HidReader>,
    _midi: Option<Box<dyn std::any::Any + Send>>,
    session: Session,
    sink: Box<dyn FrameSink>,
    capture_disabled: bool,
    last_flush: u64,
    last_focus_check: u64,
    last_refresh: u64,
    last_stats: String,
    last_stats_at: u64,
    /// a rumble command to send once its time is up (stops the motors)
    rumble_off: Option<(u64, Vec<u8>)>,
    /// reports that put the adaptive triggers back to normal, sent when the session ends
    trigger_off: Vec<Vec<u8>>,
}

pub fn channel() -> (Sender<EngineMsg>, Receiver<EngineMsg>) {
    mpsc::channel()
}

pub fn spawn(rx: Receiver<EngineMsg>, tx: Sender<EngineMsg>, platform: Arc<dyn Platform>, nudge: Arc<Nudge>, no_capture: bool, pads: Sender<PadCmd>) -> EngineHandle {
    let active = Arc::new(AtomicBool::new(false));

    let raw_tx = tx.clone();
    let sink: RawSink = Arc::new(move |ev| {
        let _ = raw_tx.send(EngineMsg::Raw(ev));
    });
    if let Err(e) = platform.start_input(sink) {
        eprintln!("[inputlab] raw input unavailable: {e}");
    }

    let handle = EngineHandle { tx: tx.clone(), active: active.clone() };
    thread::Builder::new()
        .name("inputlab-engine".into())
        .spawn(move || run(rx, tx, platform, nudge, active, no_capture, pads))
        .expect("spawn engine thread");
    handle
}

fn finish(live: &mut Option<Live>, platform: &Arc<dyn Platform>, active: &AtomicBool, reason: EndReason, pads: &Sender<PadCmd>) {
    // release first: the os must be free before anything else can go wrong
    platform.end_capture();
    let _ = pads.send(PadCmd::Watch(None));
    active.store(false, Ordering::SeqCst);
    if let Some(mut l) = live.take() {
        if let (Some((_, off)), Some(rd)) = (l.rumble_off.take(), l._reader.as_ref()) {
            rd.write(off);
            std::thread::sleep(Duration::from_millis(150)); // let the reader loop send it before it stops
        }
        if let Some(rd) = l._reader.as_ref() {
            if !l.trigger_off.is_empty() {
                for off in l.trigger_off.drain(..) {
                    rd.write(off);
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        }
        l.session.push(Ev::Ended { reason });
        if let Some(f) = l.session.take_frame() {
            l.sink.send(f);
        }
    }
}

fn run(rx: Receiver<EngineMsg>, tx: Sender<EngineMsg>, platform: Arc<dyn Platform>, nudge: Arc<Nudge>, active: Arc<AtomicBool>, no_capture: bool, pads: Sender<PadCmd>) {
    let mut live: Option<Live> = None;
    let mut last_tick = 0u64;

    loop {
        let msg = rx.recv_timeout(Duration::from_millis(4));
        let now = now_ns();
        match msg {
            Ok(EngineMsg::Raw(ev)) => {
                if matches!(ev, RawEvent::DevicesChanged) {
                    nudge.ping();
                }
                let mut end = None;
                if let Some(l) = live.as_mut() {
                    if let RawEvent::FocusLost { .. } = ev {
                        end = l.session.on_focus_lost(now);
                    }
                    l.session.on_raw(&ev);
                }
                if let Some(r) = end {
                    finish(&mut live, &platform, &active, r, &pads);
                }
            }
            Ok(EngineMsg::Start(req)) => {
                // a new session replaces any old one; release before arming again
                if live.is_some() {
                    finish(&mut live, &platform, &active, EndReason::UiGone, &pads);
                }
                let req = *req;
                let settings = req.settings.sanitized();
                let mode = mode_for(req.device.viewer);
                // safe mode (settings) and --no-capture are the same thing
                let no_capture = no_capture || settings.disable_capture;
                let spec = CaptureSpec { mode, window: req.window, disabled: no_capture };
                match platform.begin_capture(&spec) {
                    Ok(report) => {
                        let mut session = Session::new(req.device.clone(), &settings, now);
                        let mut hid_info = None;
                        let mut analog_info = None;
                        let mut midi_error = None;
                        let mut midi_conn = None;
                        let mut generic_reader = None;
                        match req.device.viewer {
                            Viewer::Hid => {
                                let t = tx.clone();
                                let opened = req.device.hid_paths.first().ok_or("no hid path for this device".to_string()).and_then(|p| HidReader::open(p, move |r| { let _ = t.send(EngineMsg::Hid(r)); }));
                                match opened {
                                    Ok(rd) => {
                                        let layout = (!rd.descriptor.is_empty()).then(|| crate::hid::descriptor::parse(&rd.descriptor));
                                        let li = layout.as_ref().map(|l| l.info(rd.descriptor.len()));
                                        session.set_hid(layout.filter(|l| !l.fields.is_empty()));
                                        hid_info = Some(HidSessionInfo { layout: li, error: None });
                                        generic_reader = Some(rd);
                                    }
                                    Err(e) => {
                                        session.set_hid(None);
                                        hid_info = Some(HidSessionInfo { layout: None, error: Some(e) });
                                    }
                                }
                            }
                            Viewer::Keyboard => {
                                if let (Some(pid), Some(path)) = (req.device.analog_profile.as_deref(), req.device.analog_path.as_deref()) {
                                    if let Some(profile) = crate::hid::analog::profile(pid) {
                                        let verified = profile.verified();
                                        let t = tx.clone();
                                        match HidReader::open(path, move |r| { let _ = t.send(EngineMsg::Hid(r)); }) {
                                            Ok(rd) => {
                                                session.set_analog(profile);
                                                analog_info = Some(AnalogInfo { profile: pid.to_string(), verified, error: None });
                                                generic_reader = Some(rd);
                                            }
                                            Err(e) => analog_info = Some(AnalogInfo { profile: pid.to_string(), verified, error: Some(e) }),
                                        }
                                    }
                                }
                            }
                            Viewer::Midi => {
                                let t = tx.clone();
                                match crate::devices::midi::open(&req.device.name, move |ts, bytes| { let _ = t.send(EngineMsg::Midi(ts, bytes)); }) {
                                    Ok(c) => midi_conn = Some(c),
                                    Err(e) => midi_error = Some(e),
                                }
                            }
                            _ => {}
                        }
                        let info = SessionInfo {
                            hid: hid_info,
                            analog: analog_info,
                            midi_error,
                            device: req.device,
                            mode,
                            exit: exit_info(mode, &settings, no_capture),
                            capture: report,
                            reserved: platform.reserved_combos(),
                            combo_warning: platform.exit_combo_warning(),
                            no_capture,
                        };
                        active.store(true, Ordering::SeqCst);
                        let mut reader = generic_reader;
                        if let Some(pad) = session.pad_id() {
                            let _ = pads.send(PadCmd::Watch(Some(pad)));
                            // the real polling rate comes from hid report timestamps, when the device lets us open it
                            if let Some(path) = info.device.hid_paths.first() {
                                let t = tx.clone();
                                // a bluetooth dualsense starts in a short report with no battery; asking for its
                                // calibration feature report switches it to the full one
                                let wake = info.device.bluetooth && matches!(info.device.vid.zip(info.device.pid).and_then(|(v, p)| crate::hid::pads::kind_for(v, p)), Some(crate::hid::pads::HidPad::DualSense));
                                reader = HidReader::open_with(
                                    path,
                                    move |dev| {
                                        if wake {
                                            let mut b = [0u8; 41];
                                            b[0] = 0x05;
                                            let _ = dev.get_feature_report(&mut b);
                                        }
                                    },
                                    move |r| {
                                        let _ = t.send(EngineMsg::Hid(r));
                                    },
                                )
                                .ok();
                            }
                        }
                        live = Some(Live { _reader: reader, _midi: midi_conn, session, sink: req.sink, capture_disabled: no_capture, last_flush: now, last_focus_check: now, last_refresh: now, last_stats: String::new(), last_stats_at: 0, rumble_off: None, trigger_off: vec![] });
                        let _ = req.reply.send(Ok(info));
                    }
                    Err(e) => {
                        platform.end_capture();
                        let _ = req.reply.send(Err(e));
                    }
                }
            }
            Ok(EngineMsg::End(reason)) => {
                let allowed = live.as_ref().is_some_and(|l| match reason {
                    EndReason::ExitButton => l.session.allows_button(l.capture_disabled),
                    _ => true,
                });
                if allowed {
                    finish(&mut live, &platform, &active, reason, &pads);
                }
            }
            Ok(EngineMsg::FocusLost) => {
                let end = live.as_mut().and_then(|l| l.session.on_focus_lost(now));
                if let Some(r) = end {
                    finish(&mut live, &platform, &active, r, &pads);
                }
            }
            Ok(EngineMsg::Pad(m)) => {
                if let Some(l) = live.as_mut() {
                    l.session.on_pad(&m, now);
                }
            }
            Ok(EngineMsg::Midi(ts, bytes)) => {
                if let Some(l) = live.as_mut() {
                    l.session.on_midi(ts, &bytes);
                }
            }
            Ok(EngineMsg::Hid(r)) => {
                if let Some(l) = live.as_mut() {
                    l.session.on_hid(&r);
                }
            }
            Ok(EngineMsg::Command(cmd)) => {
                if let Some(l) = live.as_mut() {
                    if let SessionCommand::PadRumble { strong, weak, ms } = cmd {
                        if let Some((on, off)) = l.session.hid_rumble(strong, weak) {
                            // a pad read over hid: write the output report ourselves
                            if let Some(rd) = l._reader.as_ref() {
                                rd.write(on);
                                l.rumble_off = Some((now + ms.clamp(50, 5000) as u64 * MS, off));
                            }
                        } else if let Some(pad) = l.session.pad_id() {
                            let _ = pads.send(PadCmd::Rumble { pad, strong, weak, ms });
                        }
                    }
                    if let SessionCommand::PadTrigger { right, start, force } = cmd {
                        if let (Some((on, off)), Some(rd)) = (l.session.hid_trigger(right, start, force), l._reader.as_ref()) {
                            rd.write(on);
                            if !l.trigger_off.contains(&off) {
                                l.trigger_off.push(off);
                            }
                        }
                    }
                    if let SessionCommand::PadAudio { speaker, mic } = cmd {
                        if let (Some(r), Some(rd)) = (l.session.hid_audio(speaker, mic), l._reader.as_ref()) {
                            rd.write(r);
                        }
                    }
                    l.session.command(cmd, now);
                }
            }
            Ok(EngineMsg::Heartbeat) => {
                if let Some(l) = live.as_mut() {
                    l.session.heartbeat(now);
                }
            }
            Ok(EngineMsg::Shutdown) => {
                finish(&mut live, &platform, &active, EndReason::Shutdown, &pads);
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                finish(&mut live, &platform, &active, EndReason::Shutdown, &pads);
                break;
            }
        }

        if now.saturating_sub(last_tick) < TICK_EVERY {
            continue;
        }
        last_tick = now;

        let mut end = None;
        if let Some(l) = live.as_mut() {
            if let Some((at, _)) = l.rumble_off.as_ref() {
                if now >= *at {
                    if let (Some((_, off)), Some(rd)) = (l.rumble_off.take(), l._reader.as_ref()) {
                        rd.write(off);
                    }
                }
            }
            end = l.session.tick(now);
            if end.is_none() && now.saturating_sub(l.last_focus_check) >= FOCUS_CHECK_EVERY {
                l.last_focus_check = now;
                if !platform.is_app_foreground() {
                    end = l.session.on_focus_lost(now);
                }
            }
            if now.saturating_sub(l.last_refresh) >= REFRESH_CAPTURE_EVERY {
                l.last_refresh = now;
                platform.refresh_capture();
                if let Some(t) = platform.capture_stats() {
                    // resent every second even when unchanged: the first copy can land before the
                    // session screen exists, and the line must never stay blank
                    if t != l.last_stats || now.saturating_sub(l.last_stats_at) >= 1000 * MS {
                        l.last_stats = t.clone();
                        l.last_stats_at = now;
                        l.session.push(Ev::Capture { text: t });
                    }
                }
            }
            if end.is_none() && now.saturating_sub(l.last_flush) >= FLUSH_EVERY && l.session.has_pending() {
                l.last_flush = now;
                if let Some(f) = l.session.take_frame() {
                    if !l.sink.send(f) {
                        end = Some(EndReason::UiGone);
                    }
                }
            }
        }
        if let Some(r) = end {
            finish(&mut live, &platform, &active, r, &pads);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{DeviceKind, Viewer};

    const T0: u64 = 10_000 * MS; // arbitrary session start

    fn dev(kind: DeviceKind, viewer: Viewer, srcs: Vec<u64>) -> DeviceInfo {
        let mut d = DeviceInfo::base("d".into(), "d".into(), kind, viewer, "test");
        d.srcs = srcs;
        d
    }

    fn key(src: u64, code: &'static str, down: bool, ts: u64) -> RawEvent {
        RawEvent::Key { src, code, down, ts }
    }

    fn mouse_session() -> Session {
        Session::new(dev(DeviceKind::Mouse, Viewer::Mouse, vec![5]), &SessionSettings::default(), T0)
    }
    fn kbd_session() -> Session {
        Session::new(dev(DeviceKind::Keyboard, Viewer::Keyboard, vec![1]), &SessionSettings::default(), T0)
    }
    fn pad_session() -> Session {
        Session::new(dev(DeviceKind::Gamepad, Viewer::Gamepad, vec![]), &SessionSettings::default(), T0)
    }

    #[test]
    fn mouse_session_exits_on_esc_hold_only() {
        let mut s = mouse_session();
        s.on_raw(&key(1, "Escape", true, T0));
        assert_eq!(s.tick(T0 + 500 * MS), None);
        assert_eq!(s.tick(T0 + 1000 * MS), Some(EndReason::EscHold));
    }

    #[test]
    fn mouse_session_esc_released_early_stays_open() {
        let mut s = mouse_session();
        s.on_raw(&key(1, "Escape", true, T0));
        assert_eq!(s.tick(T0 + 900 * MS), None);
        s.on_raw(&key(1, "Escape", false, T0 + 900 * MS));
        assert_eq!(s.tick(T0 + 5000 * MS), None);
    }

    #[test]
    fn mouse_session_ignores_clicks_wheel_and_motion() {
        let mut s = mouse_session();
        for i in 0..2000u64 {
            let ts = T0 + i * MS;
            s.on_raw(&RawEvent::MouseButton { src: 5, button: (i % 5) as u8 + 1, down: i % 2 == 0, ts });
            s.on_raw(&RawEvent::Wheel { src: 5, dx: 120, dy: -120, ts });
            s.on_raw(&RawEvent::MouseMove { src: 5, dx: 40, dy: 40, ts, absolute: false });
            assert_eq!(s.tick(ts), None);
            s.heartbeat(ts);
        }
    }

    #[test]
    fn mouse_session_has_no_exit_button() {
        let s = mouse_session();
        assert!(!s.allows_button(false));
        assert!(s.allows_button(true), "--no-capture frees the mouse");
    }

    #[test]
    fn keyboard_session_esc_and_shortcuts_do_not_exit() {
        let mut s = kbd_session();
        for (i, k) in ["Escape", "F4", "Tab", "MetaLeft", "F5", "F11", "KeyW", "AltLeft", "ControlLeft"].iter().enumerate() {
            s.on_raw(&key(1, k, true, T0 + i as u64 * MS));
        }
        s.heartbeat(T0 + 59_999 * MS);
        assert_eq!(s.tick(T0 + 60_000 * MS), None);
    }

    #[test]
    fn keyboard_session_exits_on_combo_after_hold() {
        let mut s = kbd_session();
        for k in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            s.on_raw(&key(1, k, true, T0));
        }
        assert_eq!(s.tick(T0 + 1400 * MS), None);
        assert_eq!(s.tick(T0 + 1500 * MS), Some(EndReason::Combo));
    }

    #[test]
    fn combo_from_a_second_keyboard_exits_the_session() {
        let mut s = kbd_session(); // under test: src 1
        for k in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            s.on_raw(&key(99, k, true, T0));
        }
        assert_eq!(s.tick(T0 + 1500 * MS), Some(EndReason::Combo));
    }

    #[test]
    fn combo_keys_still_show_up_as_test_inputs() {
        let mut s = kbd_session();
        for k in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            s.on_raw(&key(1, k, true, T0 + MS));
        }
        let f = s.take_frame().unwrap();
        let codes: Vec<_> = f.events.iter().filter_map(|e| if let Ev::Key { code, .. } = e { Some(*code) } else { None }).collect();
        assert_eq!(codes, vec!["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"]);
    }

    #[test]
    fn keyboard_events_from_other_keyboards_are_not_shown_but_still_count_for_exit() {
        let mut s = kbd_session();
        s.on_raw(&key(42, "KeyA", true, T0));
        assert!(s.take_frame().is_none());
    }

    #[test]
    fn controller_session_never_exits_from_keys_or_buttons() {
        let mut s = pad_session();
        for k in ["Escape", "ControlLeft", "AltLeft", "ShiftLeft", "Backspace", "Enter"] {
            s.on_raw(&key(1, k, true, T0));
        }
        s.heartbeat(T0 + 119_999 * MS);
        assert_eq!(s.tick(T0 + 120_000 * MS), None);
    }

    #[test]
    fn controller_session_only_ends_through_the_button() {
        let s = pad_session();
        assert!(s.allows_button(false));
    }

    #[test]
    fn watchdog_ends_a_session_whose_ui_stopped_answering() {
        let mut s = kbd_session();
        assert_eq!(s.tick(T0 + 9_000 * MS), None);
        s.heartbeat(T0 + 9_000 * MS);
        assert_eq!(s.tick(T0 + 18_000 * MS), None);
        assert_eq!(s.tick(T0 + 19_001 * MS), Some(EndReason::Watchdog));
    }

    #[test]
    fn focus_loss_ends_the_session_after_the_grace_period() {
        let mut s = kbd_session();
        assert_eq!(s.on_focus_lost(T0 + 100 * MS), None, "grace while capture arms");
        assert_eq!(s.on_focus_lost(T0 + 700 * MS), Some(EndReason::FocusLost));
    }

    #[test]
    fn os_reserved_combo_is_reported_when_focus_is_stolen() {
        let mut s = kbd_session();
        s.on_raw(&key(1, "ControlLeft", true, T0 + MS));
        s.on_raw(&key(1, "AltLeft", true, T0 + MS));
        s.take_frame();
        assert_eq!(s.on_focus_lost(T0 + 2000 * MS), Some(EndReason::FocusLost));
        let f = s.take_frame().unwrap();
        assert!(f.events.iter().any(|e| matches!(e, Ev::Reserved { label } if label.contains("del"))));
    }

    #[test]
    fn exit_progress_events_are_sent_and_reset_to_zero() {
        let mut s = mouse_session();
        s.on_raw(&key(1, "Escape", true, T0));
        s.tick(T0 + 400 * MS);
        s.on_raw(&key(1, "Escape", false, T0 + 400 * MS));
        s.tick(T0 + 410 * MS);
        let f = s.take_frame().unwrap();
        let ps: Vec<f32> = f.events.iter().filter_map(|e| if let Ev::Exit { progress, .. } = e { Some(*progress) } else { None }).collect();
        assert!(ps.iter().any(|p| *p > 0.3));
        assert_eq!(*ps.last().unwrap(), 0.0);
    }

    #[test]
    fn key_events_carry_hold_time_chatter_and_rollover() {
        let mut s = kbd_session();
        s.on_raw(&key(1, "KeyA", true, T0 + MS));
        s.on_raw(&key(1, "KeyS", true, T0 + 2 * MS));
        s.on_raw(&key(1, "KeyA", false, T0 + 80 * MS)); // held 79ms
        s.on_raw(&key(1, "KeyA", true, T0 + 95 * MS)); // 15ms bounce
        s.on_raw(&key(1, "KeyA", true, T0 + 120 * MS)); // os auto-repeat: dropped
        let evs = s.take_frame().unwrap().events;
        let keys: Vec<_> = evs.iter().filter_map(|e| if let Ev::Key { code, down, hold, chatter, held, max, .. } = e { Some((*code, *down, *hold, *chatter, *held, *max)) } else { None }).collect();
        assert_eq!(keys.len(), 4, "auto-repeat must not produce an event");
        assert_eq!(keys[1].5, 2, "max rollover");
        assert!((keys[2].2.unwrap() - 79.0).abs() < 0.01);
        assert!(keys[3].3, "15ms release->press is chatter");
    }

    fn mv(src: u64, dx: i32, dy: i32, ts: u64) -> RawEvent {
        RawEvent::MouseMove { src, dx, dy, ts, absolute: false }
    }

    fn snaps(f: &Frame) -> (Option<f64>, Option<crate::timing::dpi::DpiSnap>) {
        let mut hz = None;
        let mut dpi = None;
        for e in &f.events {
            match e {
                Ev::Poll(p) => hz = p.avg_hz,
                Ev::Dpi(d) => dpi = Some(d.clone()),
                _ => {}
            }
        }
        (hz, dpi)
    }

    #[test]
    fn mouse_movement_is_summed_per_frame_and_polling_is_measured() {
        let mut s = mouse_session();
        for i in 0..2000u64 {
            s.on_raw(&mv(5, 2, -1, T0 + i * MS)); // 1000hz
        }
        s.tick(T0 + 2000 * MS);
        let f = s.take_frame().unwrap();
        let m = f.events.iter().find_map(|e| if let Ev::Mouse { dx, dy, n } = e { Some((*dx, *dy, *n)) } else { None }).unwrap();
        assert_eq!(m, (4000, -2000, 2000));
        let (hz, _) = snaps(&f);
        assert!((hz.unwrap() - 1000.0).abs() < 5.0);
    }

    #[test]
    fn movement_from_other_mice_and_absolute_devices_is_ignored() {
        let mut s = mouse_session(); // device srcs = [5]
        s.on_raw(&mv(99, 50, 50, T0));
        s.on_raw(&RawEvent::MouseMove { src: 5, dx: 70, dy: 70, ts: T0, absolute: true });
        s.tick(T0 + 100 * MS);
        assert!(!s.take_frame().is_some_and(|f| f.events.iter().any(|e| matches!(e, Ev::Mouse { .. }))));
    }

    #[test]
    fn dpi_test_runs_through_session_commands() {
        let mut s = mouse_session();
        s.command(SessionCommand::DpiStart { distance_in: 2.0 }, T0);
        for i in 0..400u64 {
            s.on_raw(&mv(5, 8, 0, T0 + i * MS)); // 3200 counts
        }
        s.command(SessionCommand::DpiStop, T0 + 400 * MS);
        let f = s.take_frame().unwrap();
        let (_, dpi) = snaps(&f);
        let d = dpi.unwrap();
        assert!((d.dpi.unwrap() - 1600.0).abs() < 0.5);
    }

    #[test]
    fn mouse_clicks_report_hold_and_chatter() {
        let mut s = mouse_session();
        s.on_raw(&RawEvent::MouseButton { src: 5, button: 1, down: true, ts: T0 });
        s.on_raw(&RawEvent::MouseButton { src: 5, button: 1, down: false, ts: T0 + 70 * MS });
        s.on_raw(&RawEvent::MouseButton { src: 5, button: 1, down: true, ts: T0 + 85 * MS });
        let f = s.take_frame().unwrap();
        let b: Vec<_> = f.events.iter().filter_map(|e| if let Ev::Button { down, hold, chatter, .. } = e { Some((*down, *hold, *chatter)) } else { None }).collect();
        assert!((b[1].1.unwrap() - 70.0).abs() < 0.01);
        assert!(b[2].2, "15ms release->press is a double-click chatter");
    }

    #[test]
    fn wheel_events_reach_the_ui() {
        let mut s = mouse_session();
        s.on_raw(&RawEvent::Wheel { src: 5, dx: 0, dy: 120, ts: T0 });
        s.on_raw(&RawEvent::Wheel { src: 5, dx: -120, dy: 0, ts: T0 });
        let f = s.take_frame().unwrap();
        assert_eq!(f.events.iter().filter(|e| matches!(e, Ev::Wheel { .. })).count(), 2);
    }

    #[test]
    fn keyboard_sessions_have_no_mouse_state_to_confuse() {
        let mut s = kbd_session();
        s.on_raw(&mv(1, 5, 5, T0));
        s.tick(T0 + 100 * MS);
        assert!(s.take_frame().is_none());
    }

    fn pad_dev() -> Session {
        let mut d = dev(DeviceKind::Gamepad, Viewer::Gamepad, vec![]);
        d.pad_id = Some(3);
        Session::new(d, &SessionSettings::default(), T0)
    }

    fn pe(pad: usize, name: &str, v: f32, axis: bool, ts: u64) -> PadMsg {
        PadMsg::Event(crate::devices::gamepad::PadEvent { pad, name: name.into(), code: format!("c-{name}"), value: v, axis, ts })
    }

    #[test]
    fn pad_events_are_coalesced_per_control_and_filtered_by_pad() {
        let mut s = pad_dev();
        s.on_pad(&pe(3, "LeftStickX", 0.1, true, T0 + MS), T0);
        s.on_pad(&pe(3, "LeftStickX", 0.7, true, T0 + 2 * MS), T0);
        s.on_pad(&pe(3, "South", 1.0, false, T0 + 3 * MS), T0);
        s.on_pad(&pe(9, "North", 1.0, false, T0 + 3 * MS), T0); // another pad
        let f = s.take_frame().unwrap();
        let items = f.events.iter().find_map(|e| if let Ev::Pad { items } = e { Some(items.clone()) } else { None }).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items.iter().find(|i| i.name == "LeftStickX").unwrap().value, 0.7, "latest value wins");
        assert!(items.iter().all(|i| i.name != "North"));
    }

    #[test]
    fn controller_polling_prefers_hid_report_timestamps_over_events() {
        let mut s = pad_dev();
        for i in 0..400u64 {
            s.on_pad(&pe(3, "LeftStickX", (i % 7) as f32 / 10.0, true, T0 + i * 4 * MS), T0); // 250hz of events
        }
        s.tick(T0 + 2000 * MS);
        let hz = |f: &Frame| f.events.iter().rev().find_map(|e| if let Ev::Poll(p) = e { p.avg_hz } else { None });
        assert!((hz(&s.take_frame().unwrap()).unwrap() - 250.0).abs() < 3.0);
        for i in 0..2000u64 {
            s.on_hid(&HidReport { ts: T0 + 2000 * MS + i * MS, data: vec![0] }); // real 1000hz reports
        }
        s.tick(T0 + 4100 * MS);
        assert!((hz(&s.take_frame().unwrap()).unwrap() - 1000.0).abs() < 5.0);
    }

    #[test]
    fn circle_test_through_commands_reports_a_circular_gate() {
        let mut s = pad_dev();
        s.command(SessionCommand::PadCircleStart, T0);
        for i in 0..720 {
            let a = (i as f32 / 720.0) * std::f32::consts::TAU;
            s.on_pad(&pe(3, "LeftStickX", a.cos(), true, T0 + i as u64 * MS), T0);
            s.on_pad(&pe(3, "LeftStickY", a.sin(), true, T0 + i as u64 * MS), T0);
        }
        s.command(SessionCommand::PadCircleStop, T0 + 800 * MS);
        let f = s.take_frame().unwrap();
        let left = f.events.iter().rev().find_map(|e| if let Ev::Sticks { left, .. } = e { Some(left.clone()) } else { None }).unwrap();
        assert!(left.circularity.unwrap() > 0.98);
        assert!(!left.active);
    }

    #[test]
    fn rest_test_flags_a_drifting_stick() {
        let mut s = pad_dev();
        s.command(SessionCommand::PadRestStart { secs: 2.0 }, T0);
        for i in 0..200u64 {
            s.on_pad(&pe(3, "LeftStickX", 0.11, true, T0 + i * 5 * MS), T0);
            s.on_pad(&pe(3, "LeftStickY", 0.0, true, T0 + i * 5 * MS), T0);
            s.on_pad(&pe(3, "RightStickX", 0.0, true, T0 + i * 5 * MS), T0);
        }
        s.tick(T0 + 2100 * MS);
        let f = s.take_frame().unwrap();
        let rest = f.events.iter().rev().find_map(|e| if let Ev::Sticks { rest, .. } = e { Some(rest.clone()) } else { None }).unwrap();
        assert!(!rest.running);
        assert!(rest.entries.iter().find(|e| e.name == "left stick").unwrap().drift);
        assert!(!rest.entries.iter().find(|e| e.name == "right stick").unwrap().drift);
    }

    #[test]
    fn battery_and_rumble_support_reach_the_ui() {
        let mut s = pad_dev();
        s.on_pad(&PadMsg::Status(crate::devices::gamepad::PadStatus { pad: 3, force_feedback: true, power: "discharging".into(), battery: Some(55) }), T0);
        let f = s.take_frame().unwrap();
        assert!(f.events.iter().any(|e| matches!(e, Ev::PadInfo { force_feedback: true, battery: Some(55), .. })));
    }

    fn hid_session() -> Session {
        let mut s = Session::new(dev(DeviceKind::Hid, Viewer::Hid, vec![]), &SessionSettings::default(), T0);
        // 8 buttons + one 8-bit x axis, no report ids
        let desc = [0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, 0x05, 0x09, 0x19, 0x01, 0x29, 0x08, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x05, 0x01, 0x09, 0x30, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02, 0xC0];
        s.set_hid(Some(crate::hid::descriptor::parse(&desc)));
        s
    }

    fn report(ts: u64, data: &[u8]) -> HidReport {
        HidReport { ts, data: data.to_vec() }
    }

    #[test]
    fn hid_reports_are_coalesced_decoded_and_changed_bytes_reported() {
        let mut s = hid_session();
        s.on_hid(&report(T0, &[0b0000_0001, 10]));
        s.take_frame();
        s.on_hid(&report(T0 + MS, &[0b0000_0001, 200]));
        s.on_hid(&report(T0 + 2 * MS, &[0b0000_0011, 200]));
        let f = s.take_frame().unwrap();
        let (changed, n, buttons, axes, bytes) = f.events.iter().find_map(|e| if let Ev::Hid { changed, n, buttons, axes, report, .. } = e { Some((changed.clone(), *n, buttons.clone(), axes.clone(), report.clone())) } else { None }).unwrap();
        assert_eq!(n, 2, "two reports folded into one event");
        assert_eq!(changed, vec![0, 1], "both bytes changed within the frame, even though byte 1 settled");
        assert_eq!(bytes, vec![0b11, 200]);
        assert!(buttons[0] && buttons[1] && !buttons[2]);
        assert!((axes[0] - 200.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn identical_reports_flag_no_changed_bytes() {
        let mut s = hid_session();
        s.on_hid(&report(T0, &[5, 5]));
        s.take_frame();
        s.on_hid(&report(T0 + MS, &[5, 5]));
        let f = s.take_frame().unwrap();
        assert!(f.events.iter().any(|e| matches!(e, Ev::Hid { changed, .. } if changed.is_empty())));
    }

    #[test]
    fn hid_without_a_parsable_descriptor_still_dumps_raw_bytes() {
        let mut s = Session::new(dev(DeviceKind::Hid, Viewer::Hid, vec![]), &SessionSettings::default(), T0);
        s.set_hid(None);
        s.on_hid(&report(T0, &[1, 2, 3]));
        let f = s.take_frame().unwrap();
        assert!(f.events.iter().any(|e| matches!(e, Ev::Hid { report, buttons, axes, .. } if report == &vec![1, 2, 3] && buttons.is_empty() && axes.is_empty())));
    }

    #[test]
    fn hid_polling_rate_comes_from_report_timestamps() {
        let mut s = hid_session();
        for i in 0..1500u64 {
            s.on_hid(&report(T0 + i * MS, &[0, (i % 200) as u8]));
        }
        s.tick(T0 + 1500 * MS);
        let hz = s.take_frame().unwrap().events.iter().rev().find_map(|e| if let Ev::Poll(p) = e { p.avg_hz } else { None }).unwrap();
        assert!((hz - 1000.0).abs() < 5.0);
    }

    #[test]
    fn analog_keyboard_depth_changes_are_sent_and_releases_zeroed() {
        let mut s = Session::new(dev(DeviceKind::Keyboard, Viewer::Keyboard, vec![1]), &SessionSettings::default(), T0);
        s.set_analog(crate::hid::analog::profile("wooting").unwrap());
        s.on_hid(&report(T0, &[0x00, 0x1A, 100, 0, 0, 0]));
        s.on_hid(&report(T0 + MS, &[0x00, 0x1A, 200, 0x00, 0x04, 50, 0, 0, 0]));
        s.on_hid(&report(T0 + 2 * MS, &[0, 0, 0]));
        let keys: Vec<(String, f32)> = s.take_frame().unwrap().events.iter().filter_map(|e| if let Ev::Analog { keys } = e { Some(keys.clone()) } else { None }).flatten().collect();
        assert!(keys.iter().any(|(k, v)| k == "KeyW" && (*v - 100.0 / 255.0).abs() < 0.01));
        assert!(keys.iter().any(|(k, v)| k == "KeyA" && *v > 0.1));
        assert!(keys.iter().filter(|(k, v)| (k == "KeyW" || k == "KeyA") && *v == 0.0).count() == 2, "both keys released");
    }

    #[test]
    fn analog_sends_the_raw_dump_too() {
        let mut s = Session::new(dev(DeviceKind::Keyboard, Viewer::Keyboard, vec![1]), &SessionSettings::default(), T0);
        s.set_analog(crate::hid::analog::profile("wooting").unwrap());
        s.on_hid(&report(T0, &[9, 9, 9]));
        assert!(s.take_frame().unwrap().events.iter().any(|e| matches!(e, Ev::Hid { .. })));
    }

    #[test]
    fn midi_messages_are_decoded_for_midi_sessions_only() {
        let mut s = Session::new(dev(DeviceKind::Midi, Viewer::Midi, vec![]), &SessionSettings::default(), T0);
        s.on_midi(T0 + 5 * MS, &[0x90, 64, 90]);
        s.on_midi(T0 + 6 * MS, &[0xB0, 1, 33]);
        let evs = s.take_frame().unwrap().events;
        assert!(evs.iter().any(|e| matches!(e, Ev::Midi { kind, a: 64, value: 90, .. } if kind == "noteOn")));
        assert!(evs.iter().any(|e| matches!(e, Ev::Midi { kind, a: 1, value: 33, .. } if kind == "cc")));
        let mut k = kbd_session();
        k.on_midi(T0, &[0x90, 1, 1]);
        assert!(k.take_frame().is_none());
    }

    #[test]
    fn esc_hold_still_works_when_keys_only_come_from_the_hook_fallback() {
        // raw input is blind: the hook reports keys with no device identity (src 0)
        let mut s = mouse_session();
        s.on_raw(&RawEvent::RawBlind);
        s.on_raw(&key(0, "Escape", true, T0));
        assert_eq!(s.tick(T0 + 1000 * MS), Some(EndReason::EscHold));
        let f = s.take_frame().unwrap();
        assert!(f.events.iter().any(|e| matches!(e, Ev::Notice { text } if text.contains("raw input"))));
    }

    #[test]
    fn the_combo_works_from_the_hook_fallback_too() {
        let mut s = kbd_session();
        for k in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            s.on_raw(&key(0, k, true, T0));
        }
        assert_eq!(s.tick(T0 + 1500 * MS), Some(EndReason::Combo));
    }

    #[test]
    fn hook_sourced_clicks_reach_a_mouse_session() {
        let mut s = mouse_session();
        s.on_raw(&RawEvent::MouseButton { src: 0, button: 1, down: true, ts: T0 });
        s.on_raw(&RawEvent::Wheel { src: 0, dx: 0, dy: 120, ts: T0 });
        let f = s.take_frame().unwrap();
        assert!(f.events.iter().any(|e| matches!(e, Ev::Button { n: 1, down: true, .. })));
        assert!(f.events.iter().any(|e| matches!(e, Ev::Wheel { dy: 120, .. })));
    }

    #[test]
    fn a_dualsense_reports_motion_and_which_bus_it_is_on() {
        let mut d = dev(DeviceKind::Gamepad, Viewer::Gamepad, vec![]);
        d.vid = Some(0x054c);
        d.pid = Some(0x0ce6);
        let mut s = Session::new(d, &SessionSettings::default(), T0);
        let mut r = vec![0u8; 64];
        r[0] = 1;
        r[1..11].copy_from_slice(&[128, 128, 128, 128, 0, 0, 0, 0x08, 0, 0]);
        r[16..18].copy_from_slice(&1640i16.to_le_bytes()); // 100 deg/s of pitch
        r[26..28].copy_from_slice(&8192i16.to_le_bytes()); // 1 g on the third axis
        s.on_hid(&HidReport { ts: T0, data: r.clone() });
        let f = s.take_frame().unwrap();
        let m = f.events.iter().find_map(|e| if let Ev::Motion { gyro, accel } = e { Some((*gyro, *accel)) } else { None }).expect("a motion event");
        assert!((m.0[0] - 100.0).abs() < 0.01 && (m.1[2] - 1.0).abs() < 0.001);
        assert!(f.events.iter().any(|e| matches!(e, Ev::PadInfo { bus, hid_pad, .. } if bus == "usb" && hid_pad == "dualsense")));
        // throttled: another report right after adds no second reading
        s.on_hid(&HidReport { ts: T0 + MS, data: r });
        assert!(s.take_frame().is_none_or(|f| !f.events.iter().any(|e| matches!(e, Ev::Motion { .. }))));
        // trigger and audio reports exist for it
        assert!(s.hid_trigger(true, 0.5, 1.0).is_some());
        assert!(s.hid_audio(0.5, 0.5).is_some());
    }

    #[test]
    fn a_dualsense_read_over_hid_drives_the_controller_tests() {
        let mut d = dev(DeviceKind::Gamepad, Viewer::Gamepad, vec![]);
        d.vid = Some(0x054c);
        d.pid = Some(0x0ce6);
        let mut s = Session::new(d, &SessionSettings::default(), T0);
        let rep = |lx: u8, ly: u8, b0: u8, ts: u64| {
            let mut r = vec![0u8; 64];
            r[0] = 1;
            r[1..11].copy_from_slice(&[lx, ly, 128, 128, 0, 0, 0, b0, 0, 0]);
            HidReport { ts, data: r }
        };
        s.command(SessionCommand::PadCircleStart, T0);
        for i in 0..720u64 {
            let a = (i as f32 / 720.0) * std::f32::consts::TAU;
            s.on_hid(&rep((127.5 + 127.5 * a.cos()) as u8, (127.5 - 127.5 * a.sin()) as u8, 0x28, T0 + i * MS));
        }
        let f = s.take_frame().unwrap();
        let items: Vec<_> = f.events.iter().filter_map(|e| if let Ev::Pad { items } = e { Some(items.clone()) } else { None }).flatten().collect();
        assert!(items.iter().any(|i| i.name == "South" && i.value == 1.0), "cross pressed");
        assert!(items.iter().any(|i| i.name == "LeftStickX"));
        assert!(f.events.iter().any(|e| matches!(e, Ev::PadInfo { poll_source, .. } if poll_source == "hid reports")));
        s.command(SessionCommand::PadCircleStop, T0 + 800 * MS);
        let left = s.take_frame().unwrap().events.iter().rev().find_map(|e| if let Ev::Sticks { left, .. } = e { Some(left.clone()) } else { None }).unwrap();
        assert!(left.circularity.unwrap() > 0.95, "{:?}", left.circularity);
        // polling comes from the report timestamps
        s.tick(T0 + 900 * MS);
        let hz = s.take_frame().unwrap().events.iter().find_map(|e| if let Ev::Poll(p) = e { p.avg_hz } else { None }).unwrap();
        assert!((hz - 1000.0).abs() < 10.0);
    }

    #[test]
    fn frames_are_sequenced() {
        let mut s = kbd_session();
        s.on_raw(&key(1, "KeyA", true, T0));
        assert_eq!(s.take_frame().unwrap().seq, 1);
        s.on_raw(&key(1, "KeyA", false, T0));
        assert_eq!(s.take_frame().unwrap().seq, 2);
        assert!(s.take_frame().is_none());
    }
}

/// the real engine thread, driven by a fake platform that records every
/// capture / release call. this is the path that must never trap the user.
#[cfg(test)]
mod engine_thread_tests {
    use super::*;
    use crate::devices::{DeviceKind, Viewer};
    use crate::platform::{KeyLabels, PlatformCaps, RawDevice};
    use std::sync::atomic::AtomicU32;
    use std::sync::Mutex;
    use std::time::Instant;

    #[derive(Default)]
    struct Fake {
        begins: Mutex<Vec<CaptureMode>>,
        ends: AtomicU32,
        fg: AtomicBool,
        fail_begin: AtomicBool,
        sink: Mutex<Option<RawSink>>,
    }

    impl Platform for Fake {
        fn os(&self) -> &'static str { "fake" }
        fn raw_backend(&self) -> &'static str { "fake" }
        fn capabilities(&self) -> PlatformCaps { PlatformCaps::default() }
        fn enumerate_raw(&self) -> Vec<RawDevice> { vec![] }
        fn start_input(&self, sink: RawSink) -> Result<(), String> {
            *self.sink.lock().unwrap() = Some(sink);
            Ok(())
        }
        fn begin_capture(&self, spec: &CaptureSpec) -> Result<CaptureReport, String> {
            if self.fail_begin.load(Ordering::SeqCst) {
                return Err("boom".into());
            }
            self.begins.lock().unwrap().push(spec.mode);
            Ok(CaptureReport { active: true, ..Default::default() })
        }
        fn end_capture(&self) { self.ends.fetch_add(1, Ordering::SeqCst); }
        fn is_app_foreground(&self) -> bool { self.fg.load(Ordering::SeqCst) }
        fn reserved_combos(&self) -> Vec<ReservedCombo> { vec![] }
        fn key_labels(&self) -> Option<KeyLabels> { None }
    }

    struct Collect {
        frames: Arc<Mutex<Vec<Frame>>>,
        alive: Arc<AtomicBool>,
    }
    impl FrameSink for Collect {
        fn send(&self, f: Frame) -> bool {
            if !self.alive.load(Ordering::SeqCst) {
                return false;
            }
            self.frames.lock().unwrap().push(f);
            true
        }
    }

    struct Rig {
        fake: Arc<Fake>,
        eng: EngineHandle,
        frames: Arc<Mutex<Vec<Frame>>>,
        alive: Arc<AtomicBool>,
    }

    fn rig() -> Rig {
        let fake = Arc::new(Fake::default());
        fake.fg.store(true, Ordering::SeqCst);
        let (tx, rx) = channel();
        let (pad_tx, _pad_rx) = mpsc::channel();
        std::mem::forget(_pad_rx);
        let eng = spawn(rx, tx, fake.clone(), Arc::new(Nudge::default()), false, pad_tx);
        Rig { fake, eng, frames: Arc::new(Mutex::new(vec![])), alive: Arc::new(AtomicBool::new(true)) }
    }

    impl Rig {
        fn start(&self, kind: DeviceKind, viewer: Viewer) -> Result<SessionInfo, String> {
            let mut d = DeviceInfo::base("d".into(), "d".into(), kind, viewer, "fake");
            d.srcs = vec![1];
            let (tx, rx) = mpsc::channel();
            let req = StartReq {
                device: d,
                settings: SessionSettings { esc_hold_ms: 400, combo_hold_ms: 600, chatter_ms: 30.0, disable_capture: false }.sanitized(),
                window: None,
                sink: Box::new(Collect { frames: self.frames.clone(), alive: self.alive.clone() }),
                reply: tx,
            };
            self.eng.tx.send(EngineMsg::Start(Box::new(req))).unwrap();
            rx.recv_timeout(Duration::from_secs(2)).unwrap()
        }
        fn raw(&self, ev: RawEvent) {
            (self.fake.sink.lock().unwrap().as_ref().unwrap())(ev);
        }
        fn key(&self, src: u64, code: &'static str, down: bool) {
            self.raw(RawEvent::Key { src, code, down, ts: now_ns() });
        }
        fn active(&self) -> bool { self.eng.active.load(Ordering::SeqCst) }
        fn ended_with(&self) -> Option<EndReason> {
            self.frames.lock().unwrap().iter().flat_map(|f| f.events.iter()).find_map(|e| if let Ev::Ended { reason } = e { Some(*reason) } else { None })
        }
        fn wait_inactive(&self, ms: u64) -> bool {
            let t = Instant::now();
            while t.elapsed() < Duration::from_millis(ms) {
                if !self.active() { return true; }
                std::thread::sleep(Duration::from_millis(10));
            }
            false
        }
        fn ends(&self) -> u32 { self.fake.ends.load(Ordering::SeqCst) }
    }

    #[test]
    fn mouse_session_arms_mouse_capture_and_ends_on_esc_hold() {
        let r = rig();
        let info = r.start(DeviceKind::Mouse, Viewer::Mouse).unwrap();
        assert!(!info.exit.button, "no exit control in a mouse session");
        assert_eq!(*r.fake.begins.lock().unwrap(), vec![CaptureMode::Mouse]);
        assert!(r.active());
        r.key(9, "Escape", true);
        assert!(r.wait_inactive(1500), "esc held past the hold time must end the session");
        assert_eq!(r.ended_with(), Some(EndReason::EscHold));
        assert_eq!(r.ends(), 1, "capture must be released exactly when the session ends");
    }

    #[test]
    fn mouse_session_survives_a_short_esc_tap_and_hostile_mouse_input() {
        let r = rig();
        r.start(DeviceKind::Mouse, Viewer::Mouse).unwrap();
        r.key(9, "Escape", true);
        std::thread::sleep(Duration::from_millis(150));
        r.key(9, "Escape", false);
        for i in 0..500 {
            r.raw(RawEvent::MouseButton { src: 5, button: (i % 5 + 1) as u8, down: i % 2 == 0, ts: now_ns() });
            r.raw(RawEvent::Wheel { src: 5, dx: 0, dy: 120, ts: now_ns() });
        }
        std::thread::sleep(Duration::from_millis(700));
        assert!(r.active());
        assert_eq!(r.ends(), 0);
    }

    #[test]
    fn exit_button_is_refused_in_a_mouse_session_but_works_elsewhere() {
        let r = rig();
        r.start(DeviceKind::Mouse, Viewer::Mouse).unwrap();
        r.eng.tx.send(EngineMsg::End(EndReason::ExitButton)).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        assert!(r.active(), "mouse sessions cannot be left with a button");
        r.eng.tx.send(EngineMsg::End(EndReason::EscHold)).unwrap();
        assert!(r.wait_inactive(500));

        let r = rig();
        r.start(DeviceKind::Gamepad, Viewer::Gamepad).unwrap();
        assert_eq!(*r.fake.begins.lock().unwrap(), vec![CaptureMode::Other]);
        r.eng.tx.send(EngineMsg::End(EndReason::ExitButton)).unwrap();
        assert!(r.wait_inactive(500));
        assert_eq!(r.ended_with(), Some(EndReason::ExitButton));
        assert_eq!(r.ends(), 1);
    }

    #[test]
    fn keyboard_session_ends_on_combo_from_a_different_keyboard() {
        let r = rig();
        r.start(DeviceKind::Keyboard, Viewer::Keyboard).unwrap();
        for k in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            r.key(77, k, true); // not the keyboard under test
        }
        assert!(r.wait_inactive(1500));
        assert_eq!(r.ended_with(), Some(EndReason::Combo));
        assert_eq!(r.ends(), 1);
    }

    #[test]
    fn controller_session_ignores_every_key() {
        let r = rig();
        r.start(DeviceKind::Gamepad, Viewer::Gamepad).unwrap();
        for k in ["Escape", "ControlLeft", "AltLeft", "ShiftLeft", "Backspace"] {
            r.key(1, k, true);
        }
        std::thread::sleep(Duration::from_millis(1200));
        assert!(r.active());
    }

    #[test]
    fn losing_the_foreground_releases_capture_and_ends_the_session() {
        let r = rig();
        r.start(DeviceKind::Keyboard, Viewer::Keyboard).unwrap();
        r.fake.fg.store(false, Ordering::SeqCst);
        assert!(r.wait_inactive(2000), "unexpected focus loss must free the user");
        assert_eq!(r.ended_with(), Some(EndReason::FocusLost));
        assert_eq!(r.ends(), 1);
    }

    #[test]
    fn a_vanished_ui_releases_capture() {
        let r = rig();
        r.start(DeviceKind::Keyboard, Viewer::Keyboard).unwrap();
        r.alive.store(false, Ordering::SeqCst); // webview reloaded / closed
        r.key(1, "KeyA", true); // any frame that cannot be delivered
        assert!(r.wait_inactive(1000));
        assert_eq!(r.ends(), 1);
    }

    #[test]
    fn starting_a_new_session_releases_the_old_one_first() {
        let r = rig();
        r.start(DeviceKind::Mouse, Viewer::Mouse).unwrap();
        r.start(DeviceKind::Keyboard, Viewer::Keyboard).unwrap();
        assert_eq!(r.ends(), 1);
        assert_eq!(*r.fake.begins.lock().unwrap(), vec![CaptureMode::Mouse, CaptureMode::Keyboard]);
        assert!(r.active());
    }

    #[test]
    fn failed_arming_reports_the_error_and_leaves_nothing_active() {
        let r = rig();
        r.fake.fail_begin.store(true, Ordering::SeqCst);
        assert!(r.start(DeviceKind::Keyboard, Viewer::Keyboard).is_err());
        assert!(!r.active());
        assert!(r.ends() >= 1, "a half-armed capture is cleaned up");
    }

    #[test]
    fn shutdown_releases_capture() {
        let r = rig();
        r.start(DeviceKind::Mouse, Viewer::Mouse).unwrap();
        r.eng.tx.send(EngineMsg::Shutdown).unwrap();
        assert!(r.wait_inactive(500));
        assert_eq!(r.ends(), 1);
    }
}
