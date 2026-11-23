//! linux: evdev for raw keyboards and mice (kernel timestamps).

use std::path::PathBuf;

use evdev::{Device, EventType, KeyCode, RelativeAxisCode};

use super::*;

pub fn emergency_release() {}

pub struct LinuxPlatform;

impl LinuxPlatform {
    pub fn new() -> Self {
        Self
    }
}

fn classify(d: &Device) -> Option<DeviceKind> {
    let keys = d.supported_keys();
    let rel = d.supported_relative_axes();
    if rel.is_some_and(|r| r.contains(RelativeAxisCode::REL_X)) && d.supported_events().contains(EventType::RELATIVE) {
        return Some(DeviceKind::Mouse);
    }
    if keys.is_some_and(|k| k.contains(KeyCode::KEY_A) && k.contains(KeyCode::KEY_SPACE)) {
        return Some(DeviceKind::Keyboard);
    }
    None
}

/// stable per-node id: the event node number
fn src_of(path: &PathBuf) -> u64 {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.trim_start_matches("event").parse::<u64>().ok())
        .unwrap_or(0)
        + 1
}

impl Platform for LinuxPlatform {
    fn os(&self) -> &'static str {
        "linux"
    }
    fn raw_backend(&self) -> &'static str {
        "evdev"
    }
    fn capabilities(&self) -> PlatformCaps {
        let readable = evdev::enumerate().next().is_some();
        PlatformCaps {
            raw_input: readable,
            capture_mouse: readable,
            capture_keyboard: readable,
            native_key_labels: false,
            notes: if readable {
                vec![]
            } else {
                vec!["no readable /dev/input/event* nodes. add your user to the `input` group (then log out and in) so evdev can read keyboards and mice.".into()]
            },
        }
    }

    fn enumerate_raw(&self) -> Vec<RawDevice> {
        evdev::enumerate()
            .filter_map(|(path, dev)| {
                let kind = classify(&dev)?;
                let id = dev.input_id();
                let (vid, pid) = (id.vendor(), id.product());
                Some(RawDevice {
                    src: src_of(&path),
                    kind,
                    path: path.to_string_lossy().into_owned(),
                    vid: if vid == 0 { None } else { Some(vid) },
                    pid: if vid == 0 { None } else { Some(pid) },
                })
            })
            .collect()
    }

    fn start_input(&self, _sink: RawSink) -> Result<(), String> {
        Err("not wired yet".into())
    }
    fn begin_capture(&self, _spec: &CaptureSpec) -> Result<CaptureReport, String> {
        Ok(CaptureReport::default())
    }
    fn end_capture(&self) {}
    fn is_app_foreground(&self) -> bool {
        true
    }
    fn reserved_combos(&self) -> Vec<ReservedCombo> {
        vec![]
    }
}
