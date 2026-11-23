// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! macos: enumeration through hidapi (IOKit HID manager underneath). the raw
//! input + capture layer is not written yet, so sessions here are view-only.

use super::*;
use crate::hid;

pub struct MacPlatform;

impl MacPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for MacPlatform {
    fn os(&self) -> &'static str {
        "macos"
    }
    fn raw_backend(&self) -> &'static str {
        "iokit"
    }
    fn capabilities(&self) -> PlatformCaps {
        PlatformCaps {
            raw_input: false,
            capture_mouse: false,
            capture_keyboard: false,
            native_key_labels: false,
            notes: vec![
                "macos support is a stub: devices are listed through the iokit hid manager, but live keyboard and mouse input, polling and capture are not implemented yet.".into(),
            ],
        }
    }

    fn enumerate_raw(&self) -> Vec<RawDevice> {
        hid::enumerate()
            .into_iter()
            .filter(|h| h.usage_page == 0x01 && matches!(h.usage, 0x02 | 0x06))
            .enumerate()
            .map(|(i, h)| RawDevice {
                src: i as u64 + 1,
                kind: if h.usage == 0x06 { DeviceKind::Keyboard } else { DeviceKind::Mouse },
                path: h.path,
                vid: Some(h.vid),
                pid: Some(h.pid),
            })
            .collect()
    }

    fn start_input(&self, _sink: RawSink) -> Result<(), String> {
        Err("macos raw input is not implemented yet".into())
    }
    fn begin_capture(&self, _spec: &CaptureSpec) -> Result<CaptureReport, String> {
        Ok(CaptureReport {
            warnings: vec!["capture is not implemented on macos yet".into()],
            ..Default::default()
        })
    }
    fn end_capture(&self) {}
    fn is_app_foreground(&self) -> bool {
        true
    }
    fn reserved_combos(&self) -> Vec<ReservedCombo> {
        vec![]
    }
}
