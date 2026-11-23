//! windows: raw input (WM_INPUT), low-level hooks, ClipCursor, QPC.

use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Input::{
    GetRawInputDeviceInfoW, GetRawInputDeviceList, RAWINPUTDEVICELIST, RIDI_DEVICENAME,
    RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
};

use super::*;

static EMERGENCY_NEEDED: AtomicBool = AtomicBool::new(false);

pub fn emergency_release() {
    EMERGENCY_NEEDED.store(true, Ordering::SeqCst);
}

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

fn device_name(h: HANDLE) -> String {
    unsafe {
        let mut len = 0u32;
        GetRawInputDeviceInfoW(Some(h), RIDI_DEVICENAME, None, &mut len);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize];
        let n = GetRawInputDeviceInfoW(
            Some(h),
            RIDI_DEVICENAME,
            Some(buf.as_mut_ptr() as *mut _),
            &mut len,
        );
        if n == u32::MAX {
            return String::new();
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }
}

impl Platform for WindowsPlatform {
    fn os(&self) -> &'static str {
        "windows"
    }
    fn raw_backend(&self) -> &'static str {
        "rawinput"
    }
    fn capabilities(&self) -> PlatformCaps {
        PlatformCaps {
            raw_input: true,
            capture_mouse: true,
            capture_keyboard: true,
            native_key_labels: true,
            notes: vec![],
        }
    }

    fn enumerate_raw(&self) -> Vec<RawDevice> {
        unsafe {
            let stride = size_of::<RAWINPUTDEVICELIST>() as u32;
            let mut count = 0u32;
            if GetRawInputDeviceList(None, &mut count, stride) == u32::MAX || count == 0 {
                return vec![];
            }
            let mut list = vec![RAWINPUTDEVICELIST::default(); count as usize];
            let got = GetRawInputDeviceList(Some(list.as_mut_ptr()), &mut count, stride);
            if got == u32::MAX {
                return vec![];
            }
            list.truncate(got as usize);
            list.iter()
                .filter_map(|d| {
                    let kind = if d.dwType == RIM_TYPEKEYBOARD {
                        DeviceKind::Keyboard
                    } else if d.dwType == RIM_TYPEMOUSE {
                        DeviceKind::Mouse
                    } else {
                        return None;
                    };
                    let path = device_name(d.hDevice);
                    let (vid, pid) = parse_vid_pid(&path);
                    Some(RawDevice {
                        src: d.hDevice.0 as usize as u64,
                        kind,
                        path,
                        vid,
                        pid,
                    })
                })
                .collect()
        }
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
