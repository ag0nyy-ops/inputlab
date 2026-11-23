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
use crate::devices::hub::Nudge;
use crate::devices::{DeviceInfo, Viewer};
use crate::events::{EndReason, Ev, Frame};
use crate::keys;
use crate::platform::{CaptureMode, CaptureReport, CaptureSpec, Platform, RawEvent, RawSink, ReservedCombo, SrcId};
use crate::timing::dpi::DpiTest;
use crate::timing::keystats::KeyTracker;
use crate::timing::poll::PollEstimator;
use crate::timing::sensor::{ButtonTracker, SensorTest};
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
    buf: Vec<Ev>,
    seq: u64,
}

impl Session {
    pub fn new(device: DeviceInfo, settings: &SessionSettings, now: u64) -> Self {
        let mode = mode_for(device.viewer);
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
                if let Some(m) = self.mouse.as_mut().filter(|_| self.srcs.contains(src)) {
                    if let Some(o) = m.buttons.event(*button, *down, *ts) {
                        self.buf.push(Ev::Button { n: *button, down: *down, ts: rel, hold: o.hold_ms, gap: o.gap_ms, chatter: o.chatter });
                        if !*down {
                            self.buf.push(Ev::Clicks { buttons: m.buttons.stats() });
                        }
                    }
                }
            }
            RawEvent::Wheel { src, dx, dy, .. } => {
                if self.mouse.is_some() && self.srcs.contains(src) {
                    self.buf.push(Ev::Wheel { dx: *dx, dy: *dy });
                }
            }
            RawEvent::FocusLost { .. } | RawEvent::DevicesChanged | RawEvent::RawBlind => {}
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
            SessionCommand::ResetKeys => {}
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
        None
    }

    pub fn push(&mut self, ev: Ev) {
        self.buf.push(ev);
    }

    pub fn has_pending(&self) -> bool {
        !self.buf.is_empty() || self.mouse.as_ref().is_some_and(|m| m.n > 0)
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
    session: Session,
    sink: Box<dyn FrameSink>,
    capture_disabled: bool,
    last_flush: u64,
    last_focus_check: u64,
    last_refresh: u64,
}

pub fn spawn(platform: Arc<dyn Platform>, nudge: Arc<Nudge>, no_capture: bool) -> EngineHandle {
    let (tx, rx) = mpsc::channel::<EngineMsg>();
    let active = Arc::new(AtomicBool::new(false));

    let raw_tx = tx.clone();
    let sink: RawSink = Arc::new(move |ev| {
        let _ = raw_tx.send(EngineMsg::Raw(ev));
    });
    if let Err(e) = platform.start_input(sink) {
        eprintln!("[inputlab] raw input unavailable: {e}");
    }

    let handle = EngineHandle { tx, active: active.clone() };
    thread::Builder::new()
        .name("inputlab-engine".into())
        .spawn(move || run(rx, platform, nudge, active, no_capture))
        .expect("spawn engine thread");
    handle
}

fn finish(live: &mut Option<Live>, platform: &Arc<dyn Platform>, active: &AtomicBool, reason: EndReason) {
    // release first: the os must be free before anything else can go wrong
    platform.end_capture();
    active.store(false, Ordering::SeqCst);
    if let Some(mut l) = live.take() {
        l.session.push(Ev::Ended { reason });
        if let Some(f) = l.session.take_frame() {
            l.sink.send(f);
        }
    }
}

fn run(rx: Receiver<EngineMsg>, platform: Arc<dyn Platform>, nudge: Arc<Nudge>, active: Arc<AtomicBool>, no_capture: bool) {
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
                    finish(&mut live, &platform, &active, r);
                }
            }
            Ok(EngineMsg::Start(req)) => {
                // a new session replaces any old one; release before arming again
                if live.is_some() {
                    finish(&mut live, &platform, &active, EndReason::UiGone);
                }
                let req = *req;
                let settings = req.settings.sanitized();
                let mode = mode_for(req.device.viewer);
                let spec = CaptureSpec { mode, window: req.window, disabled: no_capture };
                match platform.begin_capture(&spec) {
                    Ok(report) => {
                        let session = Session::new(req.device.clone(), &settings, now);
                        let info = SessionInfo {
                            device: req.device,
                            mode,
                            exit: exit_info(mode, &settings, no_capture),
                            capture: report,
                            reserved: platform.reserved_combos(),
                            combo_warning: platform.exit_combo_warning(),
                            no_capture,
                        };
                        active.store(true, Ordering::SeqCst);
                        live = Some(Live { session, sink: req.sink, capture_disabled: no_capture, last_flush: now, last_focus_check: now, last_refresh: now });
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
                    finish(&mut live, &platform, &active, reason);
                }
            }
            Ok(EngineMsg::FocusLost) => {
                let end = live.as_mut().and_then(|l| l.session.on_focus_lost(now));
                if let Some(r) = end {
                    finish(&mut live, &platform, &active, r);
                }
            }
            Ok(EngineMsg::Command(cmd)) => {
                if let Some(l) = live.as_mut() {
                    l.session.command(cmd, now);
                }
            }
            Ok(EngineMsg::Heartbeat) => {
                if let Some(l) = live.as_mut() {
                    l.session.heartbeat(now);
                }
            }
            Ok(EngineMsg::Shutdown) => {
                finish(&mut live, &platform, &active, EndReason::Shutdown);
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                finish(&mut live, &platform, &active, EndReason::Shutdown);
                break;
            }
        }

        if now.saturating_sub(last_tick) < TICK_EVERY {
            continue;
        }
        last_tick = now;

        let mut end = None;
        if let Some(l) = live.as_mut() {
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
            finish(&mut live, &platform, &active, r);
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
        let eng = spawn(fake.clone(), Arc::new(Nudge::default()), false);
        Rig { fake, eng, frames: Arc::new(Mutex::new(vec![])), alive: Arc::new(AtomicBool::new(true)) }
    }

    impl Rig {
        fn start(&self, kind: DeviceKind, viewer: Viewer) -> Result<SessionInfo, String> {
            let mut d = DeviceInfo::base("d".into(), "d".into(), kind, viewer, "fake");
            d.srcs = vec![1];
            let (tx, rx) = mpsc::channel();
            let req = StartReq {
                device: d,
                settings: SessionSettings { esc_hold_ms: 400, combo_hold_ms: 600, chatter_ms: 30.0 }.sanitized(),
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
