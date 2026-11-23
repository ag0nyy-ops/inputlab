// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! the os boundary. everything above this module (engine, commands, frontend)
//! only knows the `Platform` trait and the platform-neutral `RawEvent`.

use std::sync::Arc;

use serde::Serialize;

use crate::devices::DeviceKind;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod win;

/// opaque id of one os-level input source (a raw-input handle, an evdev fd...).
pub type SrcId = u64;

/// one keyboard or mouse interface as the os raw layer sees it.
#[derive(Clone, Debug)]
pub struct RawDevice {
    pub src: SrcId,
    pub kind: DeviceKind,
    pub path: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
}

/// what the raw layer reports. timestamps are `timing::now_ns()` nanoseconds.
#[derive(Clone, Debug)]
pub enum RawEvent {
    /// `code` is a dom-style physical key name (KeyA, ShiftLeft, Numpad5...)
    Key { src: SrcId, code: &'static str, down: bool, ts: u64 },
    MouseMove { src: SrcId, dx: i32, dy: i32, ts: u64, absolute: bool },
    MouseButton { src: SrcId, button: u8, down: bool, ts: u64 },
    /// wheel in 120ths of a notch, as the os reports it
    Wheel { src: SrcId, dx: i32, dy: i32, ts: u64 },
    /// an input device was added or removed
    DevicesChanged,
    /// our window lost the foreground while capture was armed
    FocusLost { ts: u64 },
    /// key presses seen by the capture hook but never by raw input
    RawBlind,
}

pub type RawSink = Arc<dyn Fn(RawEvent) + Send + Sync>;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    /// device under test is a mouse: clip + hide cursor, swallow clicks and wheel
    Mouse,
    /// device under test is a keyboard: swallow every shortcut at the lowest level
    Keyboard,
    /// controller / hid / midi: same keyboard lockdown, mouse stays free
    Other,
}

#[derive(Clone, Debug)]
pub struct CaptureSpec {
    pub mode: CaptureMode,
    /// native window handle (hwnd on windows)
    pub window: Option<isize>,
    /// `--no-capture`: arm nothing
    pub disabled: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CaptureReport {
    pub active: bool,
    pub cursor_clipped: bool,
    pub keyboard_hooked: bool,
    pub mouse_swallowed: bool,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ReservedCombo {
    pub label: String,
    /// physical keys involved, dom codes
    pub keys: Vec<&'static str>,
    pub why: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCaps {
    pub raw_input: bool,
    pub capture_mouse: bool,
    pub capture_keyboard: bool,
    pub native_key_labels: bool,
    pub notes: Vec<String>,
}

/// physical key -> printable label in the current os layout
#[derive(Serialize, Clone, Debug, Default)]
pub struct KeyLabels {
    pub layout_name: String,
    pub labels: Vec<(String, String)>,
}

pub trait Platform: Send + Sync {
    /// "windows" | "macos" | "linux"
    fn os(&self) -> &'static str;
    /// the backend reading keyboards and mice: rawinput | iokit | evdev
    fn raw_backend(&self) -> &'static str;
    fn capabilities(&self) -> PlatformCaps;
    fn enumerate_raw(&self) -> Vec<RawDevice>;
    /// start the raw input thread. events flow into `sink` until the process ends.
    fn start_input(&self, sink: RawSink) -> Result<(), String>;
    fn begin_capture(&self, spec: &CaptureSpec) -> Result<CaptureReport, String>;
    /// must be idempotent and must never fail: it is the safety net.
    fn end_capture(&self);
    fn is_app_foreground(&self) -> bool;
    /// a one-line account of what capture is doing right now, for the session screen
    fn capture_stats(&self) -> Option<String> {
        None
    }
    /// re-assert capture state (cursor clip can be reset by the os). cheap, called ~4x/s.
    fn refresh_capture(&self) {}
    fn reserved_combos(&self) -> Vec<ReservedCombo>;
    fn key_labels(&self) -> Option<KeyLabels> {
        None
    }
    /// set when the exit combo (ctrl+alt+shift+backspace) is eaten by the os
    fn exit_combo_warning(&self) -> Option<String> {
        None
    }
}

/// the platform for this build.
pub fn current() -> Arc<dyn Platform> {
    #[cfg(windows)]
    {
        Arc::new(win::WindowsPlatform::new())
    }
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::MacPlatform::new())
    }
    #[cfg(target_os = "linux")]
    {
        Arc::new(linux::LinuxPlatform::new())
    }
}

/// called from the panic hook and exit paths. never panics, never blocks long.
pub fn emergency_release() {
    #[cfg(windows)]
    win::emergency_release();
    #[cfg(target_os = "linux")]
    linux::emergency_release();
}

/// pull vid/pid out of a windows raw-input device path. usb looks like
/// `\\?\HID#VID_046D&PID_C33C&MI_00#...`, bluetooth like
/// `\\?\HID#{00001124-...}_VID&0002046d_PID&b00d#...` (vendor source 0002 + the usb vid).
pub fn parse_vid_pid(path: &str) -> (Option<u16>, Option<u16>) {
    let up = path.to_ascii_uppercase();
    let hex4 = |i: usize| up.get(i..i + 4).and_then(|h| u16::from_str_radix(h, 16).ok());
    if let (Some(v), Some(p)) = (up.find("VID_"), up.find("PID_")) {
        return (hex4(v + 4), hex4(p + 4));
    }
    // bluetooth: VID&00020000 style, eight digits, the low four are the vendor id
    if let (Some(v), Some(p)) = (up.find("VID&"), up.find("PID&")) {
        return (hex4(v + 8), hex4(p + 4));
    }
    (None, None)
}

/// input "devices" windows creates for its own reasons: remote desktop, hyper-v, the
/// converted-portable and virtual hid layers. never a thing you can hold.
pub fn is_virtual_path(path: &str) -> bool {
    let u = path.to_ascii_uppercase();
    ["RDP_", "ROOT#", "CONVERTEDDEVICE", "VMBUS", "VIRTUAL", "TERMINAL"].iter().any(|n| u.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_paths() {
        let p = r"\\?\HID#VID_046D&PID_C33C&MI_00#8&2a1b&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}";
        assert_eq!(parse_vid_pid(p), (Some(0x046d), Some(0xc33c)));
    }

    #[test]
    fn parses_bluetooth_paths() {
        let p = r"\\?\HID#{00001124-0000-1000-8000-00805f9b34fb}_VID&0002046d_PID&b00d#9&1c5e&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}";
        assert_eq!(parse_vid_pid(p), (Some(0x046d), Some(0xb00d)));
    }

    #[test]
    fn recognises_windows_internal_devices() {
        assert!(is_virtual_path(r"\\?\Root#RDP_KBD#0000#{a}"));
        assert!(is_virtual_path(r"\\?\HID#ConvertedDevice&Col01#1&2&0&0000#{a}"));
        assert!(!is_virtual_path(r"\\?\HID#VID_046D&PID_C33C&MI_00#8&2a#{a}"));
        assert!(!is_virtual_path(r"\\?\ACPI#PNP0303#4&1#{a}"));
    }

    #[test]
    fn virtual_devices_have_no_ids() {
        assert_eq!(parse_vid_pid(r"\\?\ACPI#PNP0303#4&1"), (None, None));
        assert_eq!(parse_vid_pid(""), (None, None));
    }
}
