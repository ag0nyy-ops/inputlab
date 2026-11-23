// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! dev diagnostic, windows only: does a low-level keyboard hook still fire while its own process
//! has the keyboard focus? opens a plain window; click it and type. runs 40 seconds.
//!   cargo run --example hookfocus

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    use std::time::{Duration, Instant};

    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows::Win32::UI::WindowsAndMessaging::*;

    static FG: AtomicU64 = AtomicU64::new(0);
    static BG: AtomicU64 = AtomicU64::new(0);
    static MINE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

    unsafe extern "system" fn kb(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            if GetForegroundWindow().0 as isize == MINE.load(Relaxed) { FG.fetch_add(1, Relaxed) } else { BG.fetch_add(1, Relaxed) };
        }
        CallNextHookEx(None, code, w, l)
    }
    unsafe extern "system" fn wndproc(h: HWND, m: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        DefWindowProcW(h, m, wp, lp)
    }

    std::thread::spawn(|| unsafe {
        let hinst = GetModuleHandleW(None).unwrap();
        let _ = SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb), Some(hinst.into()), 0);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }
    });

    unsafe {
        let hinst = GetModuleHandleW(None).unwrap();
        let class = w!("inputlab_hookfocus");
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: hinst.into(), lpszClassName: class, ..Default::default() });
        let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), class, w!("CLICK HERE, THEN TYPE (inputlab hook test)"), WS_OVERLAPPEDWINDOW | WS_VISIBLE, 300, 300, 700, 250, None, None, Some(hinst.into()), None).unwrap();
        MINE.store(hwnd.0 as isize, Relaxed);
        // `raw` argument: register for raw keyboard input exactly the way inputlab does
        // `rawfg`: the same, but foreground-only (no RIDEV_INPUTSINK)
        let raw = std::env::args().any(|a| a == "raw");
        let rawfg = std::env::args().any(|a| a == "rawfg");
        if raw || rawfg {
            use windows::Win32::UI::Input::{RegisterRawInputDevices, RAWINPUTDEVICE, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK};
            let flags = if raw { RIDEV_INPUTSINK | RIDEV_DEVNOTIFY } else { RIDEV_DEVNOTIFY };
            let rid = [RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x06, dwFlags: flags, hwndTarget: hwnd }];
            println!("raw keyboard registered: {}", RegisterRawInputDevices(&rid, std::mem::size_of::<RAWINPUTDEVICE>() as u32).is_ok());
        }
        println!("window open: click it, then type...");
        let mut presses_fg = 0u64;
        let mut presses_bg = 0u64;
        let mut down = [false; 256];
        // no clock pressure: runs until enough typing was seen both ways, or 3 minutes
        let end = Instant::now() + Duration::from_secs(180);
        let mut last = Instant::now();
        let mut msg = MSG::default();
        while Instant::now() < end && !(presses_fg >= 15 && presses_bg >= 15) {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let ours = GetForegroundWindow() == hwnd;
            for vk in 8..255 {
                let d = GetAsyncKeyState(vk) as u16 & 0x8000 != 0;
                if d && !down[vk as usize] {
                    if ours { presses_fg += 1 } else { presses_bg += 1 }
                }
                down[vk as usize] = d;
            }
            if last.elapsed() >= Duration::from_secs(5) {
                last = Instant::now();
                println!("ours focused: {presses_fg:3} presses, {:3} hook calls | other app focused: {presses_bg:3} presses, {:3} hook calls", FG.load(Relaxed), BG.load(Relaxed));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        println!("FINAL ours focused: {presses_fg} presses, {} hook calls | other app focused: {presses_bg} presses, {} hook calls", FG.load(Relaxed), BG.load(Relaxed));
        let _ = DestroyWindow(hwnd);
    }
}
