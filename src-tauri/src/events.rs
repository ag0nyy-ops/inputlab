// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! everything rust streams to the webview. batched into frames (~120hz) so a
//! busy device never means a busy ui; timestamps stay full resolution in rust.

use serde::Serialize;

use crate::capture::ExitKind;
use crate::timing::dpi::DpiSnap;
use crate::timing::poll::PollSnap;
use crate::timing::sensor::{ButtonStat, SensorSnap};
use crate::timing::stick::{RestSnap, StickSnap};

/// one changed control on a pad. latest value wins within a frame.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PadItem {
    pub name: String,
    pub code: String,
    pub value: f32,
    pub axis: bool,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EndReason {
    /// the on-screen exit button
    ExitButton,
    /// held esc in a mouse session
    EscHold,
    /// held ctrl+alt+shift+backspace in a keyboard session
    Combo,
    /// the window lost the foreground: capture was released
    FocusLost,
    /// the ui stopped answering heartbeats
    Watchdog,
    /// the ui went away (webview closed or reloaded)
    UiGone,
    /// the app is quitting
    Shutdown,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Ev {
    /// a physical key changed state on the device under test. `ts` is ms since session start.
    /// hold: ms held (on release). gap: ms since last release of this key (on press).
    /// chatter: the press came back faster than the threshold. held/max: keys down now / most ever.
    Key { code: &'static str, down: bool, ts: f64, hold: Option<f64>, gap: Option<f64>, chatter: bool, held: u32, max: u32 },
    /// summed raw mouse movement since the last frame (counts). `n` = reports folded in.
    Mouse { dx: i32, dy: i32, n: u32 },
    /// a mouse button changed state. hold on release, gap + chatter on press.
    Button { n: u8, down: bool, ts: f64, hold: Option<f64>, gap: Option<f64>, chatter: bool },
    /// wheel in 120ths of a notch: dy > 0 is up, dx > 0 is tilt right
    Wheel { dx: i32, dy: i32 },
    Poll(PollSnap),
    Dpi(DpiSnap),
    Sensor(SensorSnap),
    Clicks { buttons: Vec<ButtonStat> },
    Pad { items: Vec<PadItem> },
    /// static-ish facts about the pad: rumble support, battery, where the polling rate comes from
    PadInfo { force_feedback: bool, power: String, battery: Option<u8>, poll_source: String, bus: String, hid_pad: String },
    /// gyro in degrees per second and accelerometer in g, as the pad reports them (uncalibrated)
    Motion { gyro: [f32; 3], accel: [f32; 3] },
    Sticks { left: StickSnap, right: StickSnap, rest: RestSnap },
    /// the latest raw input report of a generic hid device (coalesced per frame).
    /// `changed` = byte indices that changed since the last frame, `n` = reports folded in.
    Hid { id: Option<u8>, report: Vec<u8>, changed: Vec<u32>, n: u32, buttons: Vec<bool>, axes: Vec<f32> },
    /// analog depth (0..1) of keys that changed. keys not listed did not change.
    Analog { keys: Vec<(String, f32)> },
    Midi { kind: String, channel: u8, a: u8, b: u8, value: i32, ts: f64, text: String },
    /// live account of what the capture hooks are doing (diagnostics)
    Capture { text: String },
    /// something the user should know about the session
    Notice { text: String },
    /// progress of an exit hold (0..1). sent whenever it changes.
    Exit { kind: ExitKind, progress: f32 },
    /// a key combo the os ate before we could see it, noticed after the fact
    Reserved { label: String },
    /// the session is over. capture is already released when this arrives.
    Ended { reason: EndReason },
}

#[derive(Serialize, Clone, Debug)]
pub struct Frame {
    pub seq: u64,
    pub events: Vec<Ev>,
}
