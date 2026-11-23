// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! one table maps physical keys across every os. the frontend only ever sees
//! the dom-style `code` string (KeyA, ShiftLeft, Numpad5), so layouts and the
//! tested-key tracker never depend on windows scancodes or evdev numbers.

/// (dom code, windows set-1 scancode with 0xE000 for extended keys, linux evdev KEY_* number)
pub const KEYS: &[(&str, u16, u16)] = &[
    ("Escape", 0x01, 1),
    ("Digit1", 0x02, 2), ("Digit2", 0x03, 3), ("Digit3", 0x04, 4), ("Digit4", 0x05, 5),
    ("Digit5", 0x06, 6), ("Digit6", 0x07, 7), ("Digit7", 0x08, 8), ("Digit8", 0x09, 9),
    ("Digit9", 0x0A, 10), ("Digit0", 0x0B, 11),
    ("Minus", 0x0C, 12), ("Equal", 0x0D, 13), ("Backspace", 0x0E, 14), ("Tab", 0x0F, 15),
    ("KeyQ", 0x10, 16), ("KeyW", 0x11, 17), ("KeyE", 0x12, 18), ("KeyR", 0x13, 19), ("KeyT", 0x14, 20),
    ("KeyY", 0x15, 21), ("KeyU", 0x16, 22), ("KeyI", 0x17, 23), ("KeyO", 0x18, 24), ("KeyP", 0x19, 25),
    ("BracketLeft", 0x1A, 26), ("BracketRight", 0x1B, 27), ("Enter", 0x1C, 28),
    ("ControlLeft", 0x1D, 29),
    ("KeyA", 0x1E, 30), ("KeyS", 0x1F, 31), ("KeyD", 0x20, 32), ("KeyF", 0x21, 33), ("KeyG", 0x22, 34),
    ("KeyH", 0x23, 35), ("KeyJ", 0x24, 36), ("KeyK", 0x25, 37), ("KeyL", 0x26, 38),
    ("Semicolon", 0x27, 39), ("Quote", 0x28, 40), ("Backquote", 0x29, 41),
    ("ShiftLeft", 0x2A, 42), ("Backslash", 0x2B, 43),
    ("KeyZ", 0x2C, 44), ("KeyX", 0x2D, 45), ("KeyC", 0x2E, 46), ("KeyV", 0x2F, 47), ("KeyB", 0x30, 48),
    ("KeyN", 0x31, 49), ("KeyM", 0x32, 50), ("Comma", 0x33, 51), ("Period", 0x34, 52), ("Slash", 0x35, 53),
    ("ShiftRight", 0x36, 54), ("NumpadMultiply", 0x37, 55), ("AltLeft", 0x38, 56), ("Space", 0x39, 57),
    ("CapsLock", 0x3A, 58),
    ("F1", 0x3B, 59), ("F2", 0x3C, 60), ("F3", 0x3D, 61), ("F4", 0x3E, 62), ("F5", 0x3F, 63),
    ("F6", 0x40, 64), ("F7", 0x41, 65), ("F8", 0x42, 66), ("F9", 0x43, 67), ("F10", 0x44, 68),
    ("NumLock", 0x45, 69), ("ScrollLock", 0x46, 70),
    ("Numpad7", 0x47, 71), ("Numpad8", 0x48, 72), ("Numpad9", 0x49, 73), ("NumpadSubtract", 0x4A, 74),
    ("Numpad4", 0x4B, 75), ("Numpad5", 0x4C, 76), ("Numpad6", 0x4D, 77), ("NumpadAdd", 0x4E, 78),
    ("Numpad1", 0x4F, 79), ("Numpad2", 0x50, 80), ("Numpad3", 0x51, 81), ("Numpad0", 0x52, 82),
    ("NumpadDecimal", 0x53, 83),
    ("IntlBackslash", 0x56, 86), ("F11", 0x57, 87), ("F12", 0x58, 88),
    // extended (E0) keys
    ("NumpadEnter", 0xE01C, 96), ("ControlRight", 0xE01D, 97), ("NumpadDivide", 0xE035, 98),
    ("PrintScreen", 0xE037, 99), ("AltRight", 0xE038, 100),
    ("Home", 0xE047, 102), ("ArrowUp", 0xE048, 103), ("PageUp", 0xE049, 104),
    ("ArrowLeft", 0xE04B, 105), ("ArrowRight", 0xE04D, 106),
    ("End", 0xE04F, 107), ("ArrowDown", 0xE050, 108), ("PageDown", 0xE051, 109),
    ("Insert", 0xE052, 110), ("Delete", 0xE053, 111),
    ("MetaLeft", 0xE05B, 125), ("MetaRight", 0xE05C, 126), ("ContextMenu", 0xE05D, 127),
    // pause has a multi-byte make code on windows (E1 1D 45); handled by virtual key there
    ("Pause", 0xE11D, 119),
];

pub fn from_set1(scan: u16) -> Option<&'static str> {
    KEYS.iter().find(|k| k.1 == scan).map(|k| k.0)
}

pub fn from_evdev(code: u16) -> Option<&'static str> {
    KEYS.iter().find(|k| k.2 == code).map(|k| k.0)
}

pub fn to_set1(code: &str) -> Option<u16> {
    KEYS.iter().find(|k| k.0 == code).map(|k| k.1)
}

pub fn is_ctrl(code: &str) -> bool {
    matches!(code, "ControlLeft" | "ControlRight")
}
pub fn is_alt(code: &str) -> bool {
    matches!(code, "AltLeft" | "AltRight")
}
pub fn is_shift(code: &str) -> bool {
    matches!(code, "ShiftLeft" | "ShiftRight")
}
pub fn is_meta(code: &str) -> bool {
    matches!(code, "MetaLeft" | "MetaRight")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn codes_are_unique() {
        let mut names = HashSet::new();
        let mut scans = HashSet::new();
        let mut evdev = HashSet::new();
        for (n, s, e) in KEYS {
            assert!(names.insert(*n), "dup name {n}");
            assert!(scans.insert(*s), "dup scancode {s:#x} ({n})");
            assert!(evdev.insert(*e), "dup evdev {e} ({n})");
        }
    }

    #[test]
    fn lookups_round_trip() {
        assert_eq!(from_set1(0x1E), Some("KeyA"));
        assert_eq!(from_set1(0xE053), Some("Delete"));
        assert_eq!(from_evdev(30), Some("KeyA"));
        assert_eq!(from_evdev(14), Some("Backspace"));
        assert_eq!(to_set1("Backspace"), Some(0x0E));
        assert_eq!(from_set1(0xE02A), None);
    }

    #[test]
    fn exit_combo_keys_exist() {
        for k in ["ControlLeft", "AltLeft", "ShiftLeft", "Backspace", "Escape"] {
            assert!(to_set1(k).is_some());
        }
    }
}
