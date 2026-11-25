// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! macos: raw keyboards and mice through the iokit hid manager (per-device, with
//! hardware timestamps), capture through a quartz event tap plus a frozen cursor.
//!
//! two privacy switches gate this, both in system settings > privacy & security:
//!   input monitoring -> the hid manager may read keyboards. without it keyboard and
//!                       mouse tests stay locked (the mouse test exits with esc, so it
//!                       must never arm while keys are unreadable).
//!   accessibility    -> the event tap may swallow keys and clicks. without it a test
//!                       still runs, it just cannot stop shortcuts reaching macos.
//!
//! source ids are the iokit registry entry id, the same number hidapi puts in its
//! `DevSrvsID:<id>` paths, so enumeration and live events agree on which device is which.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicU8, AtomicUsize, Ordering::*};
use std::sync::OnceLock;
use std::time::Duration;

use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::array::CFArrayRef;
use core_foundation_sys::base::{kCFAllocatorDefault, CFAllocatorRef, CFIndex, CFRelease};
use core_foundation_sys::data::{CFDataGetBytePtr, CFDataRef};
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::mach_port::{CFMachPortCreateRunLoopSource, CFMachPortRef};
use core_foundation_sys::runloop::{
    kCFRunLoopBeforeWaiting, kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoopActivity, CFRunLoopAddObserver, CFRunLoopAddSource, CFRunLoopGetCurrent,
    CFRunLoopObserverCreate, CFRunLoopObserverRef, CFRunLoopRef, CFRunLoopRun, CFRunLoopRunInMode, CFRunLoopWakeUp,
};
use core_foundation_sys::string::CFStringRef;

use super::*;
use crate::{hid, keys};

// ---------------------------------------------------------------- ffi

type IOHIDManagerRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type IOHIDValueRef = *mut c_void;
type IOHIDElementRef = *mut c_void;
type IOReturn = i32;
type IOHIDDeviceCallback = extern "C" fn(ctx: *mut c_void, result: IOReturn, sender: *mut c_void, device: IOHIDDeviceRef);
type IOHIDValueCallback = extern "C" fn(ctx: *mut c_void, result: IOReturn, sender: *mut c_void, value: IOHIDValueRef);

const IO_RETURN_SUCCESS: IOReturn = 0;
const IO_RETURN_NOT_PERMITTED: IOReturn = 0xE000_02E2_u32 as i32;
/// IOHIDRequestType: listen = read keyboards
const HID_REQUEST_LISTEN: u32 = 1;
/// IOHIDAccessType
const HID_ACCESS_GRANTED: u32 = 0;
const HID_ACCESS_DENIED: u32 = 1;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: u32) -> IOHIDManagerRef;
    fn IOHIDManagerSetDeviceMatchingMultiple(manager: IOHIDManagerRef, multiple: CFArrayRef);
    fn IOHIDManagerRegisterDeviceMatchingCallback(manager: IOHIDManagerRef, callback: IOHIDDeviceCallback, ctx: *mut c_void);
    fn IOHIDManagerRegisterDeviceRemovalCallback(manager: IOHIDManagerRef, callback: IOHIDDeviceCallback, ctx: *mut c_void);
    fn IOHIDManagerRegisterInputValueCallback(manager: IOHIDManagerRef, callback: IOHIDValueCallback, ctx: *mut c_void);
    fn IOHIDManagerScheduleWithRunLoop(manager: IOHIDManagerRef, run_loop: CFRunLoopRef, mode: CFStringRef);
    fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: u32) -> IOReturn;
    fn IOHIDValueGetElement(value: IOHIDValueRef) -> IOHIDElementRef;
    fn IOHIDValueGetIntegerValue(value: IOHIDValueRef) -> CFIndex;
    fn IOHIDValueGetTimeStamp(value: IOHIDValueRef) -> u64;
    fn IOHIDElementGetUsagePage(element: IOHIDElementRef) -> u32;
    fn IOHIDElementGetUsage(element: IOHIDElementRef) -> u32;
    fn IOHIDElementGetDevice(element: IOHIDElementRef) -> IOHIDDeviceRef;
    fn IOHIDElementIsRelative(element: IOHIDElementRef) -> u8;
    fn IOHIDDeviceGetService(device: IOHIDDeviceRef) -> u32;
    fn IORegistryEntryGetRegistryEntryID(entry: u32, id: *mut u64) -> i32;
    fn IOHIDCheckAccess(request: u32) -> u32;
    fn IOHIDRequestAccess(request: u32) -> bool;
}

type CGEventTapCallBack = extern "C" fn(proxy: *mut c_void, ty: u32, event: *mut c_void, info: *mut c_void) -> *mut c_void;

const TAP_HID: u32 = 0; // kCGHIDEventTap
const TAP_SESSION: u32 = 1; // kCGSessionEventTap
const TAP_HEAD: u32 = 0; // kCGHeadInsertEventTap
const TAP_ACTIVE: u32 = 0; // kCGEventTapOptionDefault (may swallow)
const TAP_DISABLED_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_USER: u32 = 0xFFFF_FFFF;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(tap: u32, place: u32, options: u32, mask: u64, callback: CGEventTapCallBack, info: *mut c_void) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventTapIsEnabled(tap: CFMachPortRef) -> bool;
    fn CGAssociateMouseAndMouseCursorPosition(connected: u32) -> i32;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayHideCursor(display: u32) -> i32;
    fn CGDisplayShowCursor(display: u32) -> i32;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardLayoutInputSource() -> *mut c_void;
    fn TISGetInputSourceProperty(source: *mut c_void, key: CFStringRef) -> *mut c_void;
    static kTISPropertyUnicodeKeyLayoutData: CFStringRef;
    static kTISPropertyLocalizedName: CFStringRef;
    fn LMGetKbdType() -> u8;
    #[allow(clippy::too_many_arguments)]
    fn UCKeyTranslate(
        layout: *const c_void,
        virtual_key: u16,
        action: u16,
        modifiers: u32,
        keyboard_type: u32,
        options: u32,
        dead_key_state: *mut u32,
        max_len: usize,
        actual_len: *mut usize,
        out: *mut u16,
    ) -> i32;
}

#[repr(C)]
struct MachTimebase {
    numer: u32,
    denom: u32,
}

extern "C" {
    fn mach_timebase_info(info: *mut MachTimebase) -> i32;
    fn mach_absolute_time() -> u64;
    fn pthread_main_np() -> i32;
}

// ---------------------------------------------------------------- clock

fn timebase() -> (u64, u64) {
    static TB: OnceLock<(u64, u64)> = OnceLock::new();
    *TB.get_or_init(|| {
        let mut tb = MachTimebase { numer: 0, denom: 0 };
        unsafe { mach_timebase_info(&mut tb) };
        if tb.numer == 0 || tb.denom == 0 {
            (1, 1)
        } else {
            (tb.numer as u64, tb.denom as u64)
        }
    })
}

fn mach_to_ns(t: u64) -> u64 {
    let (n, d) = timebase();
    (t as u128 * n as u128 / d as u128) as u64
}

/// iokit timestamps are mach ticks; rebase them once onto the shared monotonic clock.
fn hid_ts(raw: u64) -> u64 {
    static OFFSET: OnceLock<i64> = OnceLock::new();
    let off = *OFFSET.get_or_init(|| crate::timing::now_ns() as i64 - mach_to_ns(unsafe { mach_absolute_time() }) as i64);
    if raw == 0 {
        return crate::timing::now_ns();
    }
    (mach_to_ns(raw) as i64 + off).max(0) as u64
}

// ---------------------------------------------------------------- raw input (hid manager)

static SINK: OnceLock<RawSink> = OnceLock::new();
/// IOHIDManagerOpen's answer; i32::MIN until the thread got that far
static OPEN_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
static VALUES_SEEN: AtomicU64 = AtomicU64::new(0);
static KEYS_SEEN: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct HidState {
    /// IOHIDDeviceRef -> registry entry id
    ids: HashMap<usize, u64>,
    /// (src, page << 16 | usage) for every key and button currently held
    down: HashSet<(u64, u32)>,
    /// motion from the report being assembled: src -> (dx, dy, ts)
    pending: HashMap<u64, (i32, i32, u64)>,
}

thread_local! {
    // only ever touched from the hid run loop thread
    static STATE: RefCell<HidState> = RefCell::new(HidState::default());
}

fn emit(ev: RawEvent) {
    if let Some(s) = SINK.get() {
        s(ev);
    }
}

fn registry_id(dev: IOHIDDeviceRef) -> u64 {
    if dev.is_null() {
        return 0;
    }
    let mut id = 0u64;
    unsafe {
        let svc = IOHIDDeviceGetService(dev);
        if svc != 0 {
            IORegistryEntryGetRegistryEntryID(svc, &mut id);
        }
    }
    if id == 0 {
        dev as usize as u64
    } else {
        id
    }
}

/// the src for a hidapi path. hidapi on macos names devices `DevSrvsID:<registry id>`.
pub(crate) fn src_from_path(path: &str) -> u64 {
    if let Some(id) = path.strip_prefix("DevSrvsID:").and_then(|n| n.trim().parse::<u64>().ok()) {
        return id;
    }
    // older hidapi builds use an iokit path: keep it stable within a run at least
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    h.finish() | 1 << 63
}

extern "C" fn on_device_added(_ctx: *mut c_void, _res: IOReturn, _sender: *mut c_void, dev: IOHIDDeviceRef) {
    let id = registry_id(dev);
    STATE.with(|s| {
        if let Ok(mut s) = s.try_borrow_mut() {
            s.ids.insert(dev as usize, id);
        }
    });
    emit(RawEvent::DevicesChanged);
}

extern "C" fn on_device_removed(_ctx: *mut c_void, _res: IOReturn, _sender: *mut c_void, dev: IOHIDDeviceRef) {
    STATE.with(|s| {
        if let Ok(mut s) = s.try_borrow_mut() {
            if let Some(id) = s.ids.remove(&(dev as usize)) {
                s.down.retain(|(src, _)| *src != id);
                s.pending.remove(&id);
            }
        }
    });
    emit(RawEvent::DevicesChanged);
}

/// edge-trigger keys and buttons: some boards repeat the same state across reports
fn edge(s: &mut HidState, held: (u64, u32), down: bool) -> bool {
    if down {
        s.down.insert(held)
    } else {
        s.down.remove(&held)
    }
}

fn flush_one(src: u64, p: &mut (i32, i32, u64)) {
    if p.0 != 0 || p.1 != 0 {
        emit(RawEvent::MouseMove { src, dx: p.0, dy: p.1, ts: p.2, absolute: false });
    }
    *p = (0, 0, 0);
}

/// every value of one hid report arrives in the same run loop pass, so once the loop is
/// about to sleep the report is complete and its x and y go out together.
extern "C" fn on_before_wait(_obs: CFRunLoopObserverRef, _act: CFRunLoopActivity, _info: *mut c_void) {
    STATE.with(|s| {
        if let Ok(mut s) = s.try_borrow_mut() {
            for (src, p) in s.pending.iter_mut() {
                flush_one(*src, p);
            }
        }
    });
}

extern "C" fn on_value(_ctx: *mut c_void, _res: IOReturn, _sender: *mut c_void, value: IOHIDValueRef) {
    if value.is_null() {
        return;
    }
    VALUES_SEEN.fetch_add(1, Relaxed);
    let (page, usage, v, raw_ts, el) = unsafe {
        let el = IOHIDValueGetElement(value);
        if el.is_null() {
            return;
        }
        (IOHIDElementGetUsagePage(el), IOHIDElementGetUsage(el), IOHIDValueGetIntegerValue(value), IOHIDValueGetTimeStamp(value), el)
    };
    let ts = hid_ts(raw_ts);
    let dev = unsafe { IOHIDElementGetDevice(el) };

    STATE.with(|s| {
        let Ok(mut s) = s.try_borrow_mut() else { return };
        let src = match s.ids.get(&(dev as usize)) {
            Some(id) => *id,
            None => {
                let id = registry_id(dev);
                s.ids.insert(dev as usize, id);
                id
            }
        };
        let held = (src, page << 16 | (usage & 0xFFFF));

        match (page, usage) {
            // keyboard page. 0x01..0x03 are rollover/error codes, not keys
            (0x07, 0x04..=0xE7) => {
                let Some(code) = keys::from_hid_usage(usage) else { return };
                let down = v != 0;
                if edge(&mut s, held, down) {
                    KEYS_SEEN.fetch_add(1, Relaxed);
                    emit(RawEvent::Key { src, code, down, ts });
                }
            }
            // buttons
            (0x09, 1..=255) => {
                let down = v != 0;
                if edge(&mut s, held, down) {
                    emit(RawEvent::MouseButton { src, button: usage as u8, down, ts });
                }
            }
            // x / y: relative counts only (tablets report positions)
            (0x01, 0x30 | 0x31) => {
                if unsafe { IOHIDElementIsRelative(el) } == 0 || v == 0 {
                    return;
                }
                let p = s.pending.entry(src).or_insert((0, 0, 0));
                // a value with a new timestamp starts the next report
                if p.2 != 0 && p.2 != ts {
                    flush_one(src, p);
                }
                p.2 = ts;
                let d = v.clamp(i32::MIN as isize, i32::MAX as isize) as i32;
                if usage == 0x30 {
                    p.0 = p.0.saturating_add(d);
                } else {
                    p.1 = p.1.saturating_add(d);
                }
            }
            // wheel, in 120ths of a notch like windows
            (0x01, 0x38) if v != 0 => emit(RawEvent::Wheel { src, dx: 0, dy: (v as i32).saturating_mul(120), ts }),
            // consumer page ac pan: the horizontal wheel
            (0x0C, 0x238) if v != 0 => emit(RawEvent::Wheel { src, dx: (v as i32).saturating_mul(120), dy: 0, ts }),
            _ => {}
        }
    });
}

fn matching(page: i32, usage: i32) -> CFDictionary<CFString, CFNumber> {
    CFDictionary::from_CFType_pairs(&[
        (CFString::from_static_string("DeviceUsagePage"), CFNumber::from(page)),
        (CFString::from_static_string("DeviceUsage"), CFNumber::from(usage)),
    ])
}

fn hid_thread() {
    unsafe {
        // shows the input monitoring prompt the first time; a no-op once answered
        if IOHIDCheckAccess(HID_REQUEST_LISTEN) != HID_ACCESS_GRANTED {
            IOHIDRequestAccess(HID_REQUEST_LISTEN);
        }
        let mgr = IOHIDManagerCreate(kCFAllocatorDefault, 0);
        if mgr.is_null() {
            OPEN_RESULT.store(-1, SeqCst);
            return;
        }
        let wanted = CFArray::from_CFTypes(&[matching(0x01, 0x06), matching(0x01, 0x02)]);
        IOHIDManagerSetDeviceMatchingMultiple(mgr, wanted.as_concrete_TypeRef());
        IOHIDManagerRegisterDeviceMatchingCallback(mgr, on_device_added, ptr::null_mut());
        IOHIDManagerRegisterDeviceRemovalCallback(mgr, on_device_removed, ptr::null_mut());
        IOHIDManagerRegisterInputValueCallback(mgr, on_value, ptr::null_mut());
        let rl = CFRunLoopGetCurrent();
        IOHIDManagerScheduleWithRunLoop(mgr, rl, kCFRunLoopDefaultMode);
        let obs = CFRunLoopObserverCreate(kCFAllocatorDefault, kCFRunLoopBeforeWaiting, 1, 0, on_before_wait, ptr::null_mut());
        if !obs.is_null() {
            CFRunLoopAddObserver(rl, obs, kCFRunLoopDefaultMode);
        }
        // kIOHIDOptionsTypeNone: shared, never seize. denied input monitoring shows up here
        // as not-permitted, but mice still open.
        OPEN_RESULT.store(IOHIDManagerOpen(mgr, 0), SeqCst);
        // the manager and its callbacks live as long as the process
        CFRunLoopRun();
    }
}

fn listen_access() -> u32 {
    unsafe { IOHIDCheckAccess(HID_REQUEST_LISTEN) }
}

// ---------------------------------------------------------------- capture (event tap + cursor)

const MODE_OFF: u8 = 0;
const MODE_KEYS: u8 = 1;
const MODE_MOUSE: u8 = 2;

static MODE: AtomicU8 = AtomicU8::new(MODE_OFF);
/// CFMachPortRef of the one event tap, 0 until created
static TAP: AtomicUsize = AtomicUsize::new(0);
static TAP_LOOP: AtomicUsize = AtomicUsize::new(0);
static TAP_LOOP_STARTED: AtomicBool = AtomicBool::new(false);
static CURSOR_FROZEN: AtomicBool = AtomicBool::new(false);
static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);
static AX_PROMPTED: AtomicBool = AtomicBool::new(false);
static TAP_CALLS: AtomicU64 = AtomicU64::new(0);
static SWALLOWED: AtomicU64 = AtomicU64::new(0);
static REVIVED: AtomicU64 = AtomicU64::new(0);

const fn bit(t: u32) -> u64 {
    1u64 << t
}
// CGEventType numbers
const KEY_EVENTS: u64 = bit(10) | bit(11) | bit(12) | bit(14); // key down/up, flags changed, system defined (media, brightness)
const MOUSE_EVENTS: u64 = bit(1) | bit(2) | bit(3) | bit(4) | bit(5) | bit(6) | bit(7) | bit(22) | bit(25) | bit(26) | bit(27)
    // trackpad gestures: rotate, begin/end, gesture, magnify, swipe, smart magnify, pressure
    | bit(18) | bit(19) | bit(20) | bit(29) | bit(30) | bit(31) | bit(32) | bit(34);

extern "C" fn on_tap(_proxy: *mut c_void, ty: u32, event: *mut c_void, _info: *mut c_void) -> *mut c_void {
    if ty == TAP_DISABLED_TIMEOUT || ty == TAP_DISABLED_USER {
        // macos turns a slow tap off; turn it back on only while a test is armed
        let t = TAP.load(SeqCst);
        if t != 0 && MODE.load(SeqCst) != MODE_OFF {
            unsafe { CGEventTapEnable(t as CFMachPortRef, true) };
            REVIVED.fetch_add(1, Relaxed);
        }
        return event;
    }
    TAP_CALLS.fetch_add(1, Relaxed);
    let swallow = match MODE.load(Relaxed) {
        MODE_KEYS => ty < 64 && KEY_EVENTS & bit(ty) != 0,
        MODE_MOUSE => ty < 64 && MOUSE_EVENTS & bit(ty) != 0,
        _ => false,
    };
    if swallow {
        SWALLOWED.fetch_add(1, Relaxed);
        ptr::null_mut()
    } else {
        event
    }
}

/// a thread whose run loop hosts the tap, so a busy engine never makes macos time it out
fn tap_loop() -> Option<CFRunLoopRef> {
    if !TAP_LOOP_STARTED.swap(true, SeqCst) {
        let spawned = std::thread::Builder::new().name("inputlab-tap".into()).spawn(|| unsafe {
            TAP_LOOP.store(CFRunLoopGetCurrent() as usize, SeqCst);
            loop {
                // returns at once while the loop has no source yet
                if CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.5, 0) == 1 {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        });
        if spawned.is_err() {
            TAP_LOOP_STARTED.store(false, SeqCst);
            return None;
        }
    }
    for _ in 0..100 {
        let rl = TAP_LOOP.load(SeqCst);
        if rl != 0 {
            return Some(rl as CFRunLoopRef);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

fn accessibility_trusted(prompt: bool) -> bool {
    unsafe {
        if AXIsProcessTrusted() {
            return true;
        }
        if prompt && !AX_PROMPTED.swap(true, SeqCst) {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let opts = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
            return AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef());
        }
        false
    }
}

/// the tap, created once and kept: switching tests only flips MODE.
fn ensure_tap() -> Result<CFMachPortRef, String> {
    let t = TAP.load(SeqCst);
    if t != 0 {
        return Ok(t as CFMachPortRef);
    }
    if !accessibility_trusted(true) {
        return Err("turn on inputlab in system settings > privacy & security > accessibility so it can block shortcuts during a test.".into());
    }
    let rl = tap_loop().ok_or("could not start the event tap thread")?;
    unsafe {
        let mask = KEY_EVENTS | MOUSE_EVENTS;
        let mut tap = CGEventTapCreate(TAP_HID, TAP_HEAD, TAP_ACTIVE, mask, on_tap, ptr::null_mut());
        if tap.is_null() {
            tap = CGEventTapCreate(TAP_SESSION, TAP_HEAD, TAP_ACTIVE, mask, on_tap, ptr::null_mut());
        }
        if tap.is_null() {
            return Err("macos refused the event tap. check accessibility for inputlab, then reopen it.".into());
        }
        let src = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0);
        if src.is_null() {
            CFRelease(tap as *const c_void);
            return Err("could not attach the event tap".into());
        }
        CFRunLoopAddSource(rl, src, kCFRunLoopCommonModes);
        CFRunLoopWakeUp(rl);
        CGEventTapEnable(tap, false);
        TAP.store(tap as usize, SeqCst);
        Ok(tap)
    }
}

fn freeze_cursor() -> bool {
    let ok = unsafe { CGAssociateMouseAndMouseCursorPosition(0) } == 0;
    if !CURSOR_HIDDEN.swap(true, SeqCst) {
        unsafe { CGDisplayHideCursor(CGMainDisplayID()) };
    }
    CURSOR_FROZEN.store(ok, SeqCst);
    ok
}

pub fn emergency_release() {
    MODE.store(MODE_OFF, SeqCst);
    let t = TAP.load(SeqCst);
    unsafe {
        if t != 0 {
            CGEventTapEnable(t as CFMachPortRef, false);
        }
        if CURSOR_FROZEN.swap(false, SeqCst) {
            CGAssociateMouseAndMouseCursorPosition(1);
        }
        if CURSOR_HIDDEN.swap(false, SeqCst) {
            CGDisplayShowCursor(CGMainDisplayID());
        }
    }
}

// ---------------------------------------------------------------- platform

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
        let access = listen_access();
        let open = OPEN_RESULT.load(SeqCst);
        let raw = access == HID_ACCESS_GRANTED && open != -1;
        let ax = accessibility_trusted(false);
        let mut notes = vec![];
        if access == HID_ACCESS_DENIED || (open == IO_RETURN_NOT_PERMITTED && access != HID_ACCESS_GRANTED) {
            notes.push("macos is blocking keyboard input. turn on inputlab in system settings > privacy & security > input monitoring, then quit and reopen it.".into());
        } else if access != HID_ACCESS_GRANTED {
            notes.push("inputlab needs input monitoring to read keyboards and mice. allow it in the prompt, or in system settings > privacy & security > input monitoring, then reopen inputlab.".into());
        } else if open != IO_RETURN_SUCCESS && open != i32::MIN {
            notes.push(format!("the hid manager opened with an error ({open:#x}); some devices may stay silent."));
        }
        if !ax {
            notes.push("without accessibility, tests still run but shortcuts (cmd+tab, cmd+q, media keys) are not blocked. turn it on in system settings > privacy & security > accessibility.".into());
        }
        notes.push("macos support is experimental.".into());
        PlatformCaps { raw_input: raw, capture_mouse: raw, capture_keyboard: raw && ax, native_key_labels: true, notes }
    }

    fn enumerate_raw(&self) -> Vec<RawDevice> {
        let mut seen = HashSet::new();
        hid::enumerate()
            .into_iter()
            .filter(|h| h.usage_page == 0x01 && matches!(h.usage, 0x02 | 0x06))
            .filter_map(|h| {
                let src = src_from_path(&h.path);
                let kind = if h.usage == 0x06 { DeviceKind::Keyboard } else { DeviceKind::Mouse };
                // hidapi lists one iokit device once per top-level collection
                seen.insert((src, h.usage)).then(|| RawDevice { src, kind, path: h.path, vid: Some(h.vid), pid: Some(h.pid) })
            })
            .collect()
    }

    fn start_input(&self, sink: RawSink) -> Result<(), String> {
        SINK.set(sink).map_err(|_| "raw input already started".to_string())?;
        std::thread::Builder::new().name("inputlab-iokit".into()).spawn(hid_thread).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn begin_capture(&self, spec: &CaptureSpec) -> Result<CaptureReport, String> {
        emergency_release();
        let mut rep = CaptureReport::default();
        if spec.disabled {
            rep.warnings.push("--no-capture is on: nothing is blocked.".into());
            return Ok(rep);
        }
        let keys_readable = listen_access() == HID_ACCESS_GRANTED;
        if spec.mode == CaptureMode::Mouse && !keys_readable {
            // esc-hold is the only way out of a mouse test: never arm it blind
            rep.warnings.push("keyboards are unreadable (input monitoring is off), so the mouse is left free: holding esc could not end the test.".into());
            return Ok(rep);
        }
        if spec.mode == CaptureMode::Mouse {
            rep.cursor_clipped = freeze_cursor();
        }
        match ensure_tap() {
            Ok(tap) => {
                MODE.store(if spec.mode == CaptureMode::Mouse { MODE_MOUSE } else { MODE_KEYS }, SeqCst);
                unsafe { CGEventTapEnable(tap, true) };
                if spec.mode == CaptureMode::Mouse {
                    rep.mouse_swallowed = true;
                } else {
                    rep.keyboard_hooked = true;
                }
            }
            Err(e) => rep.warnings.push(e),
        }
        if spec.mode == CaptureMode::Keyboard && !keys_readable {
            rep.warnings.push("keyboards are unreadable (input monitoring is off): only the exit button can end this test.".into());
        }
        rep.active = rep.cursor_clipped || rep.mouse_swallowed || rep.keyboard_hooked;
        Ok(rep)
    }

    fn end_capture(&self) {
        emergency_release();
    }

    fn is_app_foreground(&self) -> bool {
        use objc2_app_kit::NSWorkspace;
        let me = std::process::id() as i32;
        objc2::rc::autoreleasepool(|_| NSWorkspace::sharedWorkspace().frontmostApplication().map(|a| a.processIdentifier() == me).unwrap_or(true))
    }

    fn capture_stats(&self) -> Option<String> {
        let mode = MODE.load(Relaxed);
        if mode == MODE_OFF {
            return None;
        }
        let t = TAP.load(Relaxed);
        let on = t != 0 && unsafe { CGEventTapIsEnabled(t as CFMachPortRef) };
        Some(format!(
            "event tap {} ({}) · tap saw {} · {} swallowed · revived {}x · iokit saw {} values, {} keys{}",
            if on { "on" } else { "OFF" },
            if mode == MODE_MOUSE { "mouse" } else { "keys" },
            TAP_CALLS.load(Relaxed),
            SWALLOWED.load(Relaxed),
            REVIVED.load(Relaxed),
            VALUES_SEEN.load(Relaxed),
            KEYS_SEEN.load(Relaxed),
            if CURSOR_FROZEN.load(Relaxed) { " · cursor frozen" } else { "" },
        ))
    }

    fn refresh_capture(&self) {
        let mode = MODE.load(SeqCst);
        if mode == MODE_OFF {
            return;
        }
        let t = TAP.load(SeqCst);
        unsafe {
            if t != 0 && !CGEventTapIsEnabled(t as CFMachPortRef) {
                CGEventTapEnable(t as CFMachPortRef, true);
                REVIVED.fetch_add(1, Relaxed);
            }
            // app switches can re-attach the cursor
            if mode == MODE_MOUSE && CURSOR_FROZEN.load(SeqCst) {
                CGAssociateMouseAndMouseCursorPosition(0);
            }
        }
    }

    fn reserved_combos(&self) -> Vec<ReservedCombo> {
        vec![
            ReservedCombo { label: "cmd + option + esc".into(), keys: vec!["MetaLeft", "AltLeft", "Escape"], why: "force quit belongs to the window server and may still open".into() },
            ReservedCombo { label: "ctrl + cmd + q".into(), keys: vec!["ControlLeft", "MetaLeft", "KeyQ"], why: "lock screen; secure input and the lock screen bypass every event tap".into() },
        ]
    }

    fn key_labels(&self) -> Option<KeyLabels> {
        // the text input source api asserts on the main thread (tauri runs sync commands there)
        if unsafe { pthread_main_np() } == 0 {
            return None;
        }
        unsafe {
            let src = TISCopyCurrentKeyboardLayoutInputSource();
            if src.is_null() {
                return None;
            }
            let name = TISGetInputSourceProperty(src, kTISPropertyLocalizedName);
            let layout_name = if name.is_null() { String::new() } else { CFString::wrap_under_get_rule(name as CFStringRef).to_string() };
            let data = TISGetInputSourceProperty(src, kTISPropertyUnicodeKeyLayoutData) as CFDataRef;
            let mut labels = Vec::new();
            if !data.is_null() {
                let layout = CFDataGetBytePtr(data) as *const c_void;
                let kbd = LMGetKbdType() as u32;
                for (code, _, vk) in keys::MAC_KEYS {
                    if *vk == keys::NO_VK {
                        continue;
                    }
                    let mut dead = 0u32;
                    let mut buf = [0u16; 4];
                    let mut n = 0usize;
                    // action 3 = display, option 1 = no dead keys
                    let st = UCKeyTranslate(layout, *vk, 3, 0, kbd, 1, &mut dead, buf.len(), &mut n, buf.as_mut_ptr());
                    if st != 0 || n != 1 {
                        continue;
                    }
                    if let Some(c) = char::from_u32(buf[0] as u32) {
                        if !c.is_control() && c != ' ' {
                            labels.push((code.to_string(), c.to_string()));
                        }
                    }
                }
            }
            CFRelease(src as *const c_void);
            Some(KeyLabels { layout_name, labels })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidapi_paths_give_the_registry_id() {
        assert_eq!(src_from_path("DevSrvsID:4294968335"), 4294968335);
        // unknown formats still get a stable, non-colliding id
        let a = src_from_path("IOService:/AppleARMPE/foo");
        assert_eq!(a, src_from_path("IOService:/AppleARMPE/foo"));
        assert!(a >= 1 << 63);
    }

    #[test]
    fn tap_masks_split_keys_from_mouse() {
        assert_eq!(KEY_EVENTS & MOUSE_EVENTS, 0);
        assert!(KEY_EVENTS & bit(10) != 0, "key down");
        assert!(MOUSE_EVENTS & bit(22) != 0, "scroll wheel");
    }

    #[test]
    fn mach_ticks_convert_forward() {
        let now = unsafe { mach_absolute_time() };
        assert!(hid_ts(now) <= crate::timing::now_ns() + 1_000_000);
    }
}
