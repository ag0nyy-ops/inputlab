// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! linux: evdev for raw keyboards and mice (kernel timestamps). capture uses
//! an exclusive EVIOCGRAB: while grabbed nothing else receives the device, and
//! the kernel drops the grab by itself if this process dies.
//!
//! needs read access to /dev/input/event* (the `input` group). x11 and wayland
//! behave the same because evdev sits below both.

use std::collections::HashSet;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::*};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use evdev::{Device, EventType, KeyCode, RelativeAxisCode};

use super::*;
use crate::keys;

static SINK: OnceLock<RawSink> = OnceLock::new();
static TRACKED: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);
static GRAB_MOUSE: AtomicBool = AtomicBool::new(false);
static GRAB_KEYBOARD: AtomicBool = AtomicBool::new(false);

pub fn emergency_release() {
    GRAB_MOUSE.store(false, SeqCst);
    GRAB_KEYBOARD.store(false, SeqCst);
}

pub struct LinuxPlatform;

impl LinuxPlatform {
    pub fn new() -> Self {
        Self
    }
}

fn classify(d: &Device) -> Option<DeviceKind> {
    let rel = d.supported_relative_axes();
    if rel.is_some_and(|r| r.contains(RelativeAxisCode::REL_X)) {
        return Some(DeviceKind::Mouse);
    }
    let keys = d.supported_keys();
    if keys.is_some_and(|k| k.contains(KeyCode::KEY_A) && k.contains(KeyCode::KEY_SPACE)) {
        return Some(DeviceKind::Keyboard);
    }
    None
}

/// stable per-node id: the event node number
fn src_of(path: &std::path::Path) -> u64 {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.trim_start_matches("event").parse::<u64>().ok())
        .unwrap_or(0)
        + 1
}

fn unix_ns(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as i64).unwrap_or(0)
}

fn emit(ev: RawEvent) {
    if let Some(s) = SINK.get() {
        s(ev);
    }
}

fn read_loop(path: PathBuf, kind: DeviceKind) {
    let Ok(mut dev) = Device::open(&path) else { return };
    let _ = dev.set_nonblocking(true);
    let src = src_of(&path);
    // kernel timestamps are wall-clock; rebase onto our monotonic clock once
    let offset = crate::timing::now_ns() as i64 - unix_ns(SystemTime::now());
    let mut grabbed = false;
    let (mut dx, mut dy, mut last_ts) = (0i32, 0i32, 0u64);

    loop {
        let want = match kind {
            DeviceKind::Mouse => GRAB_MOUSE.load(SeqCst),
            _ => GRAB_KEYBOARD.load(SeqCst),
        };
        if want != grabbed {
            let ok = if want { dev.grab().is_ok() } else { dev.ungrab().is_ok() };
            if ok {
                grabbed = want;
            }
        }

        let mut pfd = libc::pollfd { fd: dev.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let r = unsafe { libc::poll(&mut pfd, 1, 50) };
        if r < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            break;
        }
        if pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            break; // unplugged
        }
        if r <= 0 {
            continue;
        }
        let events = match dev.fetch_events() {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => break,
        };
        for ev in events {
            let ts = (unix_ns(ev.timestamp()) + offset).max(0) as u64;
            let (ty, code, value) = (ev.event_type(), ev.code(), ev.value());
            if ty == EventType::KEY {
                if value == 2 {
                    continue; // autorepeat
                }
                let down = value == 1;
                if kind == DeviceKind::Mouse {
                    let n = match code {
                        272 => 1,
                        273 => 2,
                        274 => 3,
                        275 => 4,
                        276 => 5,
                        _ => continue,
                    };
                    emit(RawEvent::MouseButton { src, button: n, down, ts });
                } else if let Some(c) = keys::from_evdev(code) {
                    emit(RawEvent::Key { src, code: c, down, ts });
                }
            } else if ty == EventType::RELATIVE {
                match code {
                    0 => dx += value,
                    1 => dy += value,
                    8 => emit(RawEvent::Wheel { src, dx: 0, dy: value * 120, ts }),
                    6 => emit(RawEvent::Wheel { src, dx: value * 120, dy: 0, ts }),
                    _ => {}
                }
                last_ts = ts;
            } else if ty == EventType::SYNCHRONIZATION && code == 0 && (dx != 0 || dy != 0) {
                emit(RawEvent::MouseMove { src, dx, dy, ts: last_ts.max(ts), absolute: false });
                dx = 0;
                dy = 0;
            }
        }
    }
    if let Ok(mut t) = TRACKED.lock() {
        if let Some(s) = t.as_mut() {
            s.remove(&path);
        }
    }
    emit(RawEvent::DevicesChanged);
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
                vec!["linux support is experimental: capture grabs the device exclusively instead of clipping the cursor.".into()]
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
                let real = id.vendor() != 0;
                Some(RawDevice {
                    src: src_of(&path),
                    kind,
                    path: path.to_string_lossy().into_owned(),
                    vid: real.then(|| id.vendor()),
                    pid: real.then(|| id.product()),
                })
            })
            .collect()
    }

    fn start_input(&self, sink: RawSink) -> Result<(), String> {
        SINK.set(sink).map_err(|_| "raw input already started".to_string())?;
        if let Ok(mut t) = TRACKED.lock() {
            *t = Some(HashSet::new());
        }
        // a small manager picks up devices as they appear
        std::thread::Builder::new()
            .name("inputlab-evdev".into())
            .spawn(|| loop {
                for (path, dev) in evdev::enumerate() {
                    let Some(kind) = classify(&dev) else { continue };
                    let fresh = TRACKED.lock().ok().and_then(|mut t| t.as_mut().map(|s| s.insert(path.clone()))).unwrap_or(false);
                    if fresh {
                        let p = path.clone();
                        std::thread::Builder::new().name("inputlab-evdev-read".into()).spawn(move || read_loop(p, kind)).ok();
                    }
                }
                std::thread::sleep(Duration::from_millis(1000));
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn begin_capture(&self, spec: &CaptureSpec) -> Result<CaptureReport, String> {
        emergency_release();
        let mut rep = CaptureReport::default();
        if spec.disabled {
            rep.warnings.push("--no-capture is on: no device is grabbed.".into());
            return Ok(rep);
        }
        match spec.mode {
            CaptureMode::Mouse => {
                GRAB_MOUSE.store(true, SeqCst);
                rep.mouse_swallowed = true;
                rep.cursor_clipped = true;
            }
            _ => {
                GRAB_KEYBOARD.store(true, SeqCst);
                rep.keyboard_hooked = true;
            }
        }
        rep.active = true;
        Ok(rep)
    }

    fn end_capture(&self) {
        emergency_release();
    }

    fn is_app_foreground(&self) -> bool {
        // no portable check below the toolkit: the window-focus event from tauri covers it
        true
    }

    fn reserved_combos(&self) -> Vec<ReservedCombo> {
        vec![ReservedCombo {
            label: "ctrl + alt + f1..f12".into(),
            keys: vec!["ControlLeft", "AltLeft", "F1"],
            why: "switches virtual terminal in the kernel or display server".into(),
        }]
    }

    fn exit_combo_warning(&self) -> Option<String> {
        let opts = std::process::Command::new("setxkbmap").arg("-query").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        let env = std::env::var("XKB_DEFAULT_OPTIONS").unwrap_or_default();
        if opts.contains("terminate:ctrl_alt_bksp") || env.contains("terminate:ctrl_alt_bksp") {
            Some("your desktop treats ctrl+alt+backspace as 'kill the session'. the exit combo may never reach inputlab; only the on-screen exit button is guaranteed.".into())
        } else {
            None
        }
    }
}
