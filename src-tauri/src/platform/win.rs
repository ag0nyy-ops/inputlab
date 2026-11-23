// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! windows: raw input (WM_INPUT) on a dedicated time-critical thread, low-level
//! hooks for capture, ClipCursor for the mouse, QueryPerformanceCounter for time.
//!
//! safety rules for everything below:
//!  * hooks only swallow input while OUR window is the foreground window. if it
//!    is not, they pass everything through and report FocusLost.
//!  * `release_all` unhooks and unclips, can be called from any thread, any
//!    number of times, and is what end_capture, the panic hook and exit all use.

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, AtomicU32, AtomicU64, Ordering::*};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_HIGHEST, THREAD_PRIORITY_TIME_CRITICAL,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardLayoutNameW, MapVirtualKeyExW, SendInput, ToUnicodeEx, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    MAPVK_VSC_TO_VK_EX, VIRTUAL_KEY,
};
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList, RegisterRawInputDevices, HRAWINPUT,
    RAWINPUT, RAWINPUTDEVICE, RAWINPUTDEVICELIST, RAWINPUTHEADER, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDI_DEVICENAME,
    RID_INPUT, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, ClipCursor, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetForegroundWindow,
    GetMessageW, GetWindowThreadProcessId, PeekMessageW, PostThreadMessageW, RegisterClassW, SetTimer, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HHOOK, HWND_MESSAGE, MSG, PM_NOREMOVE, WH_KEYBOARD_LL, WH_MOUSE_LL,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_INPUT, WM_INPUT_DEVICE_CHANGE, WM_MOUSEMOVE, WM_TIMER, WNDCLASSW, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT,
};

use super::*;
use crate::keys;

const WM_ARM: u32 = WM_APP + 1;
/// ask the hook thread to install the keyboard hook again (windows can drop a hook silently)
const WM_REHOOK: u32 = WM_APP + 2;

static SINK: OnceLock<RawSink> = OnceLock::new();
static THREAD_ID: AtomicU32 = AtomicU32::new(0);
static KB_HOOK: AtomicIsize = AtomicIsize::new(0);
static MS_HOOK: AtomicIsize = AtomicIsize::new(0);
/// what the keyboard hook did, shown live on the session screen so a leak can be diagnosed
/// the dedicated thread that owns the low-level hooks (its own message loop, normal priority)
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
/// 0 = not run, 1 = the hook saw an injected test key, 2 = it did not
static HOOK_SELFTEST: AtomicU32 = AtomicU32::new(0);
static HOOK_THREAD: std::sync::Once = std::sync::Once::new();
static HOOK_SWALLOWED: AtomicU64 = AtomicU64::new(0);
static HOOK_INJECTED: AtomicU64 = AtomicU64::new(0);
static HOOK_UNFOCUSED: AtomicU64 = AtomicU64::new(0);
static HOOK_CALLS: AtomicU64 = AtomicU64::new(0);
static HOOK_REINSTALLS: AtomicU32 = AtomicU32::new(0);
/// the last keys the keyboard hook saw: vk | up << 8 | outcome << 9 (1 swallowed, 2 passed on, 3 unfocused)
const KEYLOG_LEN: usize = 14;
static KEYLOG: [AtomicU32; KEYLOG_LEN] = [const { AtomicU32::new(0) }; KEYLOG_LEN];
static KEYLOG_N: AtomicU64 = AtomicU64::new(0);
static WATCH_RAW: AtomicU64 = AtomicU64::new(0);
static WATCH_CALLS: AtomicU64 = AtomicU64::new(0);
static WATCH_AT: AtomicU64 = AtomicU64::new(0);
static KB_ON: AtomicBool = AtomicBool::new(false);
/// keyboard hook installed but not swallowing: a second key source that keeps the exit rules alive
static KB_MONITOR: AtomicBool = AtomicBool::new(false);
static RAW_HWND: AtomicIsize = AtomicIsize::new(0);
static RAW_MOUSE_SEEN: AtomicU64 = AtomicU64::new(0);
static HOOK_MOUSE_SEEN: AtomicU64 = AtomicU64::new(0);
static MOUSE_HOOK_SOURCE: AtomicBool = AtomicBool::new(false);
static MS_ON: AtomicBool = AtomicBool::new(false);
static APP_HWND: AtomicIsize = AtomicIsize::new(0);
static OUR_PID: AtomicU32 = AtomicU32::new(0);
static HOOK_KEYS_SEEN: AtomicU64 = AtomicU64::new(0);
static RAW_KEYS_SEEN: AtomicU64 = AtomicU64::new(0);
static HOOK_SOURCE: AtomicBool = AtomicBool::new(false);
static FOCUS_SENT: AtomicBool = AtomicBool::new(false);
static LAST_MOUSE_TS: AtomicU64 = AtomicU64::new(0);
static QPC_FREQ: AtomicI64 = AtomicI64::new(0);
static QPC_OFFSET: AtomicI64 = AtomicI64::new(0);
static PENDING: Mutex<Option<(CaptureSpec, mpsc::Sender<CaptureReport>)>> = Mutex::new(None);

/// QueryPerformanceCounter, rebased onto the shared `timing::now_ns` origin.
fn ts() -> u64 {
    unsafe {
        let mut freq = QPC_FREQ.load(Relaxed);
        if freq == 0 {
            let mut f = 0i64;
            let _ = QueryPerformanceFrequency(&mut f);
            let mut c = 0i64;
            let _ = QueryPerformanceCounter(&mut c);
            let qpc_ns = (c as i128 * 1_000_000_000 / f.max(1) as i128) as i64;
            QPC_OFFSET.store(crate::timing::now_ns() as i64 - qpc_ns, Relaxed);
            QPC_FREQ.store(f.max(1), Relaxed);
            freq = f.max(1);
        }
        let mut c = 0i64;
        let _ = QueryPerformanceCounter(&mut c);
        ((c as i128 * 1_000_000_000 / freq as i128) as i64 + QPC_OFFSET.load(Relaxed)) as u64
    }
}

fn emit(ev: RawEvent) {
    if let Some(s) = SINK.get() {
        s(ev);
    }
}

/// unhook everything and free the cursor. safe from any thread, idempotent.
pub fn emergency_release() {
    unsafe { release_all() }
}

unsafe fn release_all() {
    KB_ON.store(false, SeqCst);
    KB_MONITOR.store(false, SeqCst);
    MS_ON.store(false, SeqCst);
    let k = KB_HOOK.swap(0, SeqCst);
    if k != 0 {
        let _ = UnhookWindowsHookEx(HHOOK(k as *mut c_void));
    }
    let m = MS_HOOK.swap(0, SeqCst);
    if m != 0 {
        let _ = UnhookWindowsHookEx(HHOOK(m as *mut c_void));
    }
    let _ = ClipCursor(None);
}

unsafe fn app_foreground() -> bool {
    let fg = GetForegroundWindow();
    if fg.0.is_null() {
        return false;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(fg, Some(&mut pid));
    pid == OUR_PID.load(Relaxed)
}

/// tell the engine we are not foreground (once per loss). the engine ends the session after its grace.
unsafe fn emit_focus_lost() {
    if !FOCUS_SENT.swap(true, SeqCst) {
        emit(RawEvent::FocusLost { ts: ts() });
    }
}

/// mouse hook found us unfocused: free the cursor right now and tell the engine.
/// the keyboard hook never switches itself off: it decides per key, so a focus flicker while
/// the capture is arming cannot leave shortcuts unblocked for the rest of the session.
unsafe fn notify_focus_lost() {
    MS_ON.store(false, SeqCst);
    let _ = ClipCursor(None);
    emit_focus_lost();
}

// ------------------------------------------------------------------ raw input

#[derive(Default)]
struct MoveGroup {
    moves: Vec<(u64, i32, i32, bool)>,
}

unsafe fn process_input(ri: &RAWINPUT, g: &mut MoveGroup) {
    let src = ri.header.hDevice.0 as usize as u64;
    if ri.header.dwType == RIM_TYPEMOUSE.0 {
        RAW_MOUSE_SEEN.fetch_add(1, Relaxed);
        MOUSE_HOOK_SOURCE.store(false, Relaxed);
        let m = ri.data.mouse;
        let absolute = m.usFlags.0 & 0x01 != 0;
        if m.lLastX != 0 || m.lLastY != 0 {
            g.moves.push((src, m.lLastX, m.lLastY, absolute));
        }
        let bf = m.Anonymous.Anonymous.usButtonFlags as u32;
        let bd = m.Anonymous.Anonymous.usButtonData as i16 as i32;
        if bf != 0 {
            let t = ts();
            const BTNS: [(u32, u32, u8); 5] = [(0x1, 0x2, 1), (0x4, 0x8, 2), (0x10, 0x20, 3), (0x40, 0x80, 4), (0x100, 0x200, 5)];
            for (down, up, n) in BTNS {
                if bf & down != 0 {
                    emit(RawEvent::MouseButton { src, button: n, down: true, ts: t });
                }
                if bf & up != 0 {
                    emit(RawEvent::MouseButton { src, button: n, down: false, ts: t });
                }
            }
            if bf & 0x400 != 0 {
                emit(RawEvent::Wheel { src, dx: 0, dy: bd, ts: t });
            }
            if bf & 0x800 != 0 {
                emit(RawEvent::Wheel { src, dx: bd, dy: 0, ts: t });
            }
        }
    } else if ri.header.dwType == RIM_TYPEKEYBOARD.0 {
        let k = ri.data.keyboard;
        let flags = k.Flags as u32;
        let down = flags & 0x01 == 0;
        // 0xFF = fake keys windows injects around fn/numlock/printscreen
        if k.VKey == 0xFF {
            return;
        }
        let code = if k.VKey == 0x13 {
            "Pause"
        } else {
            if flags & 0x04 != 0 || k.MakeCode == 0 {
                return;
            }
            let scan = if flags & 0x02 != 0 { 0xE000 | k.MakeCode } else { k.MakeCode };
            match keys::from_set1(scan) {
                Some(c) => c,
                None => return,
            }
        };
        RAW_KEYS_SEEN.fetch_add(1, Relaxed);
        HOOK_SOURCE.store(false, Relaxed); // raw input works: the hook goes back to just swallowing
        emit(RawEvent::Key { src, code, down, ts: ts() });
    }
}

/// emit the mouse moves of one WM_INPUT drain. when several reports were
/// queued behind each other they get evenly spaced timestamps between the last
/// report and now, so a batched drain does not read as one 0us gap + one long gap.
fn flush_moves(g: &mut MoveGroup) {
    if g.moves.is_empty() {
        return;
    }
    let now = ts();
    let last = LAST_MOUSE_TS.load(Relaxed);
    let n = g.moves.len() as u64;
    let spread = last != 0 && now.saturating_sub(last) < 5_000_000;
    for (i, (src, dx, dy, abs)) in g.moves.drain(..).enumerate() {
        let t = if spread { last + (now - last) * (i as u64 + 1) / n } else { now };
        emit(RawEvent::MouseMove { src, dx, dy, ts: t, absolute: abs });
    }
    LAST_MOUSE_TS.store(now, Relaxed);
}

unsafe fn on_wm_input(lparam: LPARAM) {
    let hdr = size_of::<RAWINPUTHEADER>() as u32;
    let mut g = MoveGroup::default();
    let mut buf = [0u64; 32];
    let mut size = (buf.len() * 8) as u32;
    let got = GetRawInputData(HRAWINPUT(lparam.0 as *mut c_void), RID_INPUT, Some(buf.as_mut_ptr() as *mut c_void), &mut size, hdr);
    if got != u32::MAX {
        process_input(&*(buf.as_ptr() as *const RAWINPUT), &mut g);
    }
    flush_moves(&mut g);
}

/// windows keeps ONE raw input registration per process per device type, and the
/// window toolkit registers its own when its event loop starts, which silently
/// replaces ours. so we register again whenever it matters, and once a second.
unsafe fn register_raw(hwnd: HWND) -> bool {
    let flags = RIDEV_INPUTSINK | RIDEV_DEVNOTIFY;
    let rids = [
        RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x02, dwFlags: flags, hwndTarget: hwnd },
        RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x06, dwFlags: flags, hwndTarget: hwnd },
    ];
    RegisterRawInputDevices(&rids, size_of::<RAWINPUTDEVICE>() as u32).is_ok()
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_INPUT => {
            on_wm_input(lparam);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_TIMER => {
            register_raw(hwnd);
            LRESULT(0)
        }
        WM_INPUT_DEVICE_CHANGE => {
            emit(RawEvent::DevicesChanged);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------------- hooks

fn log_key(vk: u32, up: bool, outcome: u32) {
    let n = KEYLOG_N.fetch_add(1, Relaxed) as usize;
    KEYLOG[n % KEYLOG_LEN].store((vk & 0xFF) | ((up as u32) << 8) | (outcome << 9) | (1 << 16), Relaxed);
}

/// a readable name for a virtual key, for the diagnostics line
fn vk_name(vk: u32) -> String {
    match vk {
        0x08 => "backspace".into(),
        0x09 => "tab".into(),
        0x0D => "enter".into(),
        0x10 | 0xA0 | 0xA1 => "shift".into(),
        0x11 | 0xA2 | 0xA3 => "ctrl".into(),
        0x12 | 0xA4 | 0xA5 => "alt".into(),
        0x1B => "esc".into(),
        0x20 => "space".into(),
        0x2C => "prtsc".into(),
        0x5B => "lwin".into(),
        0x5C => "rwin".into(),
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_ascii_lowercase().to_string(),
        _ => format!("{vk:#04x}"),
    }
}

/// "lwin+s tab-s": key, + down / - up, then s swallowed, p passed on, u passed because we were not foreground
fn keylog_text() -> String {
    let total = KEYLOG_N.load(Relaxed) as usize;
    let count = total.min(KEYLOG_LEN);
    let mut out = vec![];
    for i in 0..count {
        let v = KEYLOG[(total - count + i) % KEYLOG_LEN].load(Relaxed);
        let o = match (v >> 9) & 3 { 1 => 's', 2 => 'p', _ => 'u' };
        out.push(format!("{}{}{}", vk_name(v & 0xFF), if v & 0x100 != 0 { '-' } else { '+' }, o));
    }
    out.join(" ")
}

unsafe extern "system" fn ll_keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let swallow = KB_ON.load(Relaxed);
    if code >= 0 && (swallow || KB_MONITOR.load(Relaxed)) {
        HOOK_CALLS.fetch_add(1, Relaxed);
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        // keystrokes injected by software (macro tools, remote desktop, some keyboard drivers)
        // are swallowed like any other: letting them through is a leak in a lockdown tester
        if info.flags.0 & 0x10 != 0 {
            HOOK_INJECTED.fetch_add(1, Relaxed);
        }
        // our own self-test key is not a real key press: it must never count as "unfocused"
        let selftest_key = info.flags.0 & 0x10 != 0 && info.vkCode == 0x87;
        if app_foreground() {
            FOCUS_SENT.store(false, Relaxed);
            let seen = HOOK_KEYS_SEEN.fetch_add(1, Relaxed) + 1;
            // raw input should have reported these keys too. if it saw none, it is blind:
            // report keys from the hook instead, so the exit rules can never go deaf.
            if RAW_KEYS_SEEN.load(Relaxed) == 0 && seen >= 4 && !HOOK_SOURCE.swap(true, Relaxed) {
                emit(RawEvent::RawBlind);
            }
            if HOOK_SOURCE.load(Relaxed) {
                let ext = info.flags.0 & 0x01 != 0;
                let up = info.flags.0 & 0x80 != 0;
                let scan = if ext { 0xE000 | info.scanCode as u16 } else { info.scanCode as u16 };
                let name = if info.vkCode == 0x13 { Some("Pause") } else { keys::from_set1(scan) };
                if let Some(c) = name {
                    emit(RawEvent::Key { src: 0, code: c, down: !up, ts: ts() });
                }
            }
            if swallow {
                HOOK_SWALLOWED.fetch_add(1, Relaxed);
                if !selftest_key {
                    log_key(info.vkCode, info.flags.0 & 0x80 != 0, 1);
                }
                return LRESULT(1);
            }
            log_key(info.vkCode, info.flags.0 & 0x80 != 0, 2);
        } else if swallow && !selftest_key {
            HOOK_UNFOCUSED.fetch_add(1, Relaxed);
            log_key(info.vkCode, info.flags.0 & 0x80 != 0, 3);
            emit_focus_lost();
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn ll_mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // moves pass untouched (the cursor is already clipped); everything else is swallowed
    if code >= 0 && MS_ON.load(Relaxed) && wparam.0 as u32 != WM_MOUSEMOVE {
        if app_foreground() {
            let m = wparam.0 as u32;
            let seen = HOOK_MOUSE_SEEN.fetch_add(1, Relaxed) + 1;
            if RAW_MOUSE_SEEN.load(Relaxed) == 0 && seen >= 3 {
                MOUSE_HOOK_SOURCE.store(true, Relaxed);
            }
            if MOUSE_HOOK_SOURCE.load(Relaxed) {
                let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
                let t = ts();
                let delta = (info.mouseData >> 16) as i16 as i32;
                match m {
                    0x201 => emit(RawEvent::MouseButton { src: 0, button: 1, down: true, ts: t }),
                    0x202 => emit(RawEvent::MouseButton { src: 0, button: 1, down: false, ts: t }),
                    0x204 => emit(RawEvent::MouseButton { src: 0, button: 2, down: true, ts: t }),
                    0x205 => emit(RawEvent::MouseButton { src: 0, button: 2, down: false, ts: t }),
                    0x207 => emit(RawEvent::MouseButton { src: 0, button: 3, down: true, ts: t }),
                    0x208 => emit(RawEvent::MouseButton { src: 0, button: 3, down: false, ts: t }),
                    0x20B | 0x20C => {
                        let n = if (info.mouseData >> 16) as u16 == 1 { 4 } else { 5 };
                        emit(RawEvent::MouseButton { src: 0, button: n, down: m == 0x20B, ts: t });
                    }
                    0x20A => emit(RawEvent::Wheel { src: 0, dx: 0, dy: delta, ts: t }),
                    0x20E => emit(RawEvent::Wheel { src: 0, dx: delta, dy: 0, ts: t }),
                    _ => {}
                }
            }
            return LRESULT(1);
        }
        notify_focus_lost();
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe fn clip_to_window(hwnd: HWND) -> bool {
    let mut r = RECT::default();
    if GetClientRect(hwnd, &mut r).is_err() {
        return false;
    }
    let mut tl = POINT { x: r.left, y: r.top };
    let mut br = POINT { x: r.right, y: r.bottom };
    let _ = ClientToScreen(hwnd, &mut tl);
    let _ = ClientToScreen(hwnd, &mut br);
    let rr = RECT { left: tl.x, top: tl.y, right: br.x, bottom: br.y };
    ClipCursor(Some(&rr)).is_ok()
}

unsafe fn arm(spec: &CaptureSpec) -> CaptureReport {
    release_all();
    HOOK_KEYS_SEEN.store(0, SeqCst);
    HOOK_SWALLOWED.store(0, SeqCst);
    HOOK_INJECTED.store(0, SeqCst);
    HOOK_UNFOCUSED.store(0, SeqCst);
    HOOK_CALLS.store(0, SeqCst);
    HOOK_REINSTALLS.store(0, SeqCst);
    KEYLOG_N.store(0, SeqCst);
    WATCH_RAW.store(0, SeqCst);
    WATCH_CALLS.store(0, SeqCst);
    WATCH_AT.store(0, SeqCst);
    RAW_KEYS_SEEN.store(0, SeqCst);
    HOOK_SOURCE.store(false, SeqCst);
    RAW_MOUSE_SEEN.store(0, SeqCst);
    HOOK_MOUSE_SEEN.store(0, SeqCst);
    MOUSE_HOOK_SOURCE.store(false, SeqCst);
    FOCUS_SENT.store(false, SeqCst);
    APP_HWND.store(spec.window.unwrap_or(0), SeqCst);
    let rh = RAW_HWND.load(SeqCst);
    if rh != 0 {
        register_raw(HWND(rh as *mut c_void));
    }

    let mut rep = CaptureReport::default();
    if spec.disabled {
        rep.warnings.push("--no-capture is on: nothing is clipped, hooked or swallowed.".into());
        return rep;
    }
    let hinst: HINSTANCE = GetModuleHandleW(None).map(|m| m.into()).unwrap_or_default();
    match spec.mode {
        CaptureMode::Mouse => {
            match spec.window {
                Some(h) => rep.cursor_clipped = clip_to_window(HWND(h as *mut c_void)),
                None => rep.warnings.push("no window handle: the cursor could not be clipped.".into()),
            }
            if !rep.cursor_clipped && spec.window.is_some() {
                rep.warnings.push("ClipCursor failed: the cursor can leave the window.".into());
            }
            match SetWindowsHookExW(WH_MOUSE_LL, Some(ll_mouse), Some(hinst), 0) {
                Ok(h) => {
                    MS_HOOK.store(h.0 as isize, SeqCst);
                    MS_ON.store(true, SeqCst);
                    rep.mouse_swallowed = true;
                }
                Err(e) => rep.warnings.push(format!("mouse hook failed ({e}): clicks can reach other apps.")),
            }
            // a mouse session has no keyboard lockdown, but esc-hold must work even if raw input
            // goes quiet: a pass-through keyboard hook keeps a second key source ready
            if let Ok(h) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard), Some(hinst), 0) {
                KB_HOOK.store(h.0 as isize, SeqCst);
                KB_MONITOR.store(true, SeqCst);
            }
        }
        CaptureMode::Keyboard | CaptureMode::Other => match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard), Some(hinst), 0) {
            Ok(h) => {
                KB_HOOK.store(h.0 as isize, SeqCst);
                KB_ON.store(true, SeqCst);
                rep.keyboard_hooked = true;
            }
            Err(e) => rep.warnings.push(format!("keyboard hook failed ({e}): shortcuts like alt+tab still work.")),
        },
    }
    rep.active = rep.cursor_clipped || rep.keyboard_hooked || rep.mouse_swallowed;
    rep
}

unsafe fn raw_thread() {
    let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
    OUR_PID.store(GetCurrentProcessId(), SeqCst);
    let hinst: HINSTANCE = GetModuleHandleW(None).map(|m| m.into()).unwrap_or_default();

    let class = w!("inputlab_rawinput");
    let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: hinst, lpszClassName: class, ..Default::default() };
    RegisterClassW(&wc);
    let hwnd = match CreateWindowExW(WINDOW_EX_STYLE(0), class, w!("inputlab raw input"), WINDOW_STYLE(0), 0, 0, 0, 0, Some(HWND_MESSAGE), None, Some(hinst), None) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[inputlab] raw input window failed: {e}");
            return;
        }
    };
    RAW_HWND.store(hwnd.0 as isize, SeqCst);
    if !register_raw(hwnd) {
        eprintln!("[inputlab] RegisterRawInputDevices failed, retrying every second");
    }
    // re-assert once a second: the window toolkit's own registration replaces ours
    SetTimer(Some(hwnd), 1, 1000, None);

    // make sure the thread has a message queue before anyone posts to it
    let mut msg = MSG::default();
    let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
    THREAD_ID.store(GetCurrentThreadId(), SeqCst);

    loop {
        let r = GetMessageW(&mut msg, None, 0, 0);
        if r.0 <= 0 {
            break;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

/// owns the low-level hooks. a hook is only ever called while the thread that installed it is
/// pumping messages, so it gets a thread of its own that does nothing else.
unsafe fn hook_thread() {
    let mut msg = MSG::default();
    let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
    HOOK_THREAD_ID.store(GetCurrentThreadId(), SeqCst);
    loop {
        let r = GetMessageW(&mut msg, None, 0, 0);
        if r.0 <= 0 {
            break;
        }
        if msg.hwnd.0.is_null() && msg.message == WM_REHOOK {
            rehook_keyboard();
            continue;
        }
        if msg.hwnd.0.is_null() && msg.message == WM_ARM {
            let pending = PENDING.lock().ok().and_then(|mut p| p.take());
            if let Some((spec, tx)) = pending {
                let _ = tx.send(arm(&spec));
            }
            continue;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

fn ensure_hook_thread() {
    HOOK_THREAD.call_once(|| {
        let _ = std::thread::Builder::new().name("inputlab-hooks".into()).spawn(|| unsafe {
            // windows skips a hook that answers too slowly: never let this thread get starved
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
            hook_thread()
        });
    });
}

/// install the keyboard hook again, same mode as before. only ever runs on the hook thread.
unsafe fn rehook_keyboard() {
    if !(KB_ON.load(SeqCst) || KB_MONITOR.load(SeqCst)) {
        return;
    }
    let hinst: HINSTANCE = GetModuleHandleW(None).map(|m| m.into()).unwrap_or_default();
    let old = KB_HOOK.swap(0, SeqCst);
    if old != 0 {
        let _ = UnhookWindowsHookEx(HHOOK(old as *mut c_void));
    }
    if let Ok(h) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard), Some(hinst), 0) {
        KB_HOOK.store(h.0 as isize, SeqCst);
        HOOK_REINSTALLS.fetch_add(1, SeqCst);
    }
}

/// inject a harmless key (F24) and check the keyboard hook is actually called for it.
unsafe fn keyboard_hook_selftest() -> bool {
    let before = HOOK_CALLS.load(SeqCst);
    let key = |up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0x87), wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, time: 0, dwExtraInfo: 0 } },
    };
    let inputs = [key(false), key(true)];
    SendInput(&inputs, size_of::<INPUT>() as i32);
    for _ in 0..40 {
        if HOOK_CALLS.load(SeqCst) > before {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

// ------------------------------------------------------------------- platform

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
        let n = GetRawInputDeviceInfoW(Some(h), RIDI_DEVICENAME, Some(buf.as_mut_ptr() as *mut _), &mut len);
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
        PlatformCaps { raw_input: true, capture_mouse: true, capture_keyboard: true, native_key_labels: true, notes: vec![] }
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
                    Some(RawDevice { src: d.hDevice.0 as usize as u64, kind, path, vid, pid })
                })
                .collect()
        }
    }

    fn start_input(&self, sink: RawSink) -> Result<(), String> {
        SINK.set(sink).map_err(|_| "raw input already started".to_string())?;
        std::thread::Builder::new()
            .name("inputlab-rawinput".into())
            .spawn(|| unsafe { raw_thread() })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn begin_capture(&self, spec: &CaptureSpec) -> Result<CaptureReport, String> {
        if spec.disabled {
            return Ok(unsafe { arm(spec) });
        }
        // hooks live on their own thread (it pumps messages), so hand the request over
        ensure_hook_thread();
        let mut waited = 0;
        while HOOK_THREAD_ID.load(SeqCst) == 0 {
            if waited > 100 {
                return Err("the hook thread is not running, capture is unavailable".into());
            }
            std::thread::sleep(Duration::from_millis(20));
            waited += 1;
        }
        let (tx, rx) = mpsc::channel();
        if let Ok(mut p) = PENDING.lock() {
            *p = Some((spec.clone(), tx));
        }
        unsafe { PostThreadMessageW(HOOK_THREAD_ID.load(SeqCst), WM_ARM, WPARAM(0), LPARAM(0)) }.map_err(|e| format!("could not reach the hook thread: {e}"))?;
        let mut rep = rx.recv_timeout(Duration::from_secs(2)).map_err(|_| "arming capture timed out".to_string())?;

        // does windows actually call the keyboard hook? say so, loudly, if it does not
        if rep.keyboard_hooked {
            let ok = unsafe { keyboard_hook_selftest() };
            HOOK_SELFTEST.store(if ok { 1 } else { 2 }, SeqCst);
            HOOK_CALLS.store(0, SeqCst);
            HOOK_SWALLOWED.store(0, SeqCst);
            HOOK_INJECTED.store(0, SeqCst);
            HOOK_KEYS_SEEN.store(0, SeqCst);
            HOOK_UNFOCUSED.store(0, SeqCst);
            if !ok {
                rep.keyboard_hooked = false;
                rep.warnings.push("the keyboard hook is installed but windows is not calling it, so alt+tab, the windows key and other shortcuts are NOT blocked. the exit keys still work. something on this system (security software, a game anti-cheat, a driver) may be stopping low-level hooks.".into());
            }
        }
        Ok(rep)
    }

    fn end_capture(&self) {
        unsafe { release_all() }
    }

    fn is_app_foreground(&self) -> bool {
        unsafe { app_foreground() }
    }

    fn capture_stats(&self) -> Option<String> {
        let kb = KB_HOOK.load(Relaxed) != 0;
        let ms = MS_HOOK.load(Relaxed) != 0;
        if !kb && !ms {
            return None;
        }
        let mut parts = vec![];
        if kb {
            parts.push(format!(
                "keyboard hook {} (self-test {}) · hook saw {} keys · raw input saw {} · {} swallowed · {} from software · {} while unfocused · reinstalled {}x",
                if KB_ON.load(Relaxed) { "swallowing" } else { "watching only" },
                match HOOK_SELFTEST.load(Relaxed) { 1 => "passed", 2 => "FAILED", _ => "not run" },
                HOOK_CALLS.load(Relaxed),
                RAW_KEYS_SEEN.load(Relaxed),
                HOOK_SWALLOWED.load(Relaxed),
                HOOK_INJECTED.load(Relaxed),
                HOOK_UNFOCUSED.load(Relaxed),
                HOOK_REINSTALLS.load(Relaxed)
            ));
            let log = keylog_text();
            if !log.is_empty() {
                parts.push(format!("last keys the hook saw (s swallowed, p passed on, u not foreground): {log}"));
            }
        }
        if ms {
            parts.push("mouse hook installed".into());
        }
        Some(parts.join(" · "))
    }

    fn refresh_capture(&self) {
        // the keyboard hook can go deaf (windows drops a hook that ever answers too slowly): raw
        // input still sees the keys, the hook does not. install it again when that happens.
        if KB_ON.load(Relaxed) {
            let raw = RAW_KEYS_SEEN.load(Relaxed);
            let calls = HOOK_CALLS.load(Relaxed);
            let (wr, wc) = (WATCH_RAW.swap(raw, Relaxed), WATCH_CALLS.swap(calls, Relaxed));
            let now = ts();
            if raw >= wr + 2 && calls == wc && now.saturating_sub(WATCH_AT.load(Relaxed)) > 1_000_000_000 {
                WATCH_AT.store(now, Relaxed);
                let t = HOOK_THREAD_ID.load(Relaxed);
                if t != 0 {
                    let _ = unsafe { PostThreadMessageW(t, WM_REHOOK, WPARAM(0), LPARAM(0)) };
                }
            }
        }
        let h = APP_HWND.load(Relaxed);
        if h != 0 && MS_ON.load(Relaxed) {
            unsafe {
                clip_to_window(HWND(h as *mut c_void));
            }
        }
    }

    fn reserved_combos(&self) -> Vec<ReservedCombo> {
        vec![
            ReservedCombo { label: "ctrl + alt + del".into(), keys: vec!["ControlLeft", "AltLeft", "Delete"], why: "handled by windows before any app can see it".into() },
            ReservedCombo { label: "win + l".into(), keys: vec!["MetaLeft", "KeyL"], why: "locks the pc, no hook can stop it".into() },
        ]
    }

    fn key_labels(&self) -> Option<KeyLabels> {
        unsafe {
            let fg = GetForegroundWindow();
            let tid = GetWindowThreadProcessId(fg, None);
            let hkl = GetKeyboardLayout(tid);
            let mut name = [0u16; 9];
            let layout_name = if GetKeyboardLayoutNameW(&mut name).is_ok() {
                String::from_utf16_lossy(&name[..8])
            } else {
                String::new()
            };
            let state = [0u8; 256];
            let mut labels = Vec::new();
            for (code, scan, _) in keys::KEYS {
                let vk = MapVirtualKeyExW(*scan as u32, MAPVK_VSC_TO_VK_EX, Some(hkl));
                if vk == 0 {
                    continue;
                }
                let mut buf = [0u16; 4];
                // flag 0x4: do not touch the keyboard state, so dead keys are not consumed
                let n = ToUnicodeEx(vk, (*scan & 0xFF) as u32, &state, &mut buf, 0x4, Some(hkl));
                if n == 1 {
                    if let Some(c) = char::from_u32(buf[0] as u32) {
                        if !c.is_control() && c != ' ' {
                            labels.push((code.to_string(), c.to_string()));
                        }
                    }
                }
            }
            Some(KeyLabels { layout_name, labels })
        }
    }
}
