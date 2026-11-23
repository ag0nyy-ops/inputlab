//! everything rust streams to the webview. batched into frames (~120hz) so a
//! busy device never means a busy ui; timestamps stay full resolution in rust.

use serde::Serialize;

use crate::capture::ExitKind;

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
    Key { code: &'static str, down: bool, ts: f64 },
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
