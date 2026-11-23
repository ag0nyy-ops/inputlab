// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
//! analog keyboard profiles. a profile recognises a board's analog hid
//! interface and turns its reports into (key, depth 0..1). profiles are
//! pluggable: add a struct, implement `AnalogProfile`, list it in `profile`.
//! anything without a profile still gets the raw-report dump.

use super::HidInfo;

pub const WOOTING_ANALOG_PAGE: u16 = 0xFF54;

/// returns (profile id, hid path of the analog interface) when the keyboard
/// with this vid/pid is a known analog board.
pub fn detect(hid: &[HidInfo], vid: u16, pid: u16) -> Option<(&'static str, String)> {
    let wooting = vid == 0x31E3 || (vid == 0x03EB && matches!(pid, 0xFF01 | 0xFF02));
    if !wooting {
        return None;
    }
    hid.iter()
        .find(|h| h.vid == vid && h.pid == pid && h.usage_page == WOOTING_ANALOG_PAGE)
        .map(|h| ("wooting", h.path.clone()))
}

pub trait AnalogProfile: Send {
    fn id(&self) -> &'static str;
    /// true once somebody has checked this parser against real hardware
    fn verified(&self) -> bool;
    /// keys currently pressed and how far. keys not listed are up.
    fn parse(&self, report: &[u8]) -> Vec<(&'static str, f32)>;
}

pub fn profile(id: &str) -> Option<Box<dyn AnalogProfile>> {
    match id {
        "wooting" => Some(Box::new(Wooting)),
        _ => None,
    }
}

/// usb hid keyboard-page usage id -> physical key name
pub fn usage_to_code(u: u16) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL", "KeyM", "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV", "KeyW",
        "KeyX", "KeyY", "KeyZ",
    ];
    const DIGITS: [&str; 10] = ["Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0"];
    const FKEYS: [&str; 12] = ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"];
    const NUMPAD: [&str; 10] = ["Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6", "Numpad7", "Numpad8", "Numpad9", "Numpad0"];
    const MODS: [&str; 8] = ["ControlLeft", "ShiftLeft", "AltLeft", "MetaLeft", "ControlRight", "ShiftRight", "AltRight", "MetaRight"];
    Some(match u {
        0x04..=0x1D => LETTERS[(u - 0x04) as usize],
        0x1E..=0x27 => DIGITS[(u - 0x1E) as usize],
        0x28 => "Enter",
        0x29 => "Escape",
        0x2A => "Backspace",
        0x2B => "Tab",
        0x2C => "Space",
        0x2D => "Minus",
        0x2E => "Equal",
        0x2F => "BracketLeft",
        0x30 => "BracketRight",
        0x31 => "Backslash",
        0x33 => "Semicolon",
        0x34 => "Quote",
        0x35 => "Backquote",
        0x36 => "Comma",
        0x37 => "Period",
        0x38 => "Slash",
        0x39 => "CapsLock",
        0x3A..=0x45 => FKEYS[(u - 0x3A) as usize],
        0x46 => "PrintScreen",
        0x47 => "ScrollLock",
        0x48 => "Pause",
        0x49 => "Insert",
        0x4A => "Home",
        0x4B => "PageUp",
        0x4C => "Delete",
        0x4D => "End",
        0x4E => "PageDown",
        0x4F => "ArrowRight",
        0x50 => "ArrowLeft",
        0x51 => "ArrowDown",
        0x52 => "ArrowUp",
        0x53 => "NumLock",
        0x54 => "NumpadDivide",
        0x55 => "NumpadMultiply",
        0x56 => "NumpadSubtract",
        0x57 => "NumpadAdd",
        0x58 => "NumpadEnter",
        0x59..=0x62 => NUMPAD[(u - 0x59) as usize],
        0x63 => "NumpadDecimal",
        0x64 => "IntlBackslash",
        0x65 => "ContextMenu",
        0xE0..=0xE7 => MODS[(u - 0xE0) as usize],
        _ => return None,
    })
}

/// wooting analog boards: the analog interface (usage page 0xFF54) sends a list
/// of pressed keys as repeating 3-byte entries: u16 big-endian hid usage, u8 depth
/// (0-255), ended by an all-zero entry.
///
/// UNVERIFIED: written from the format wooting's open analog sdk uses as best
/// remembered, never run against a real board. if depths look wrong, the raw
/// report panel in the keyboard view shows exactly what the board sends.
pub struct Wooting;

impl AnalogProfile for Wooting {
    fn id(&self) -> &'static str {
        "wooting"
    }
    fn verified(&self) -> bool {
        false
    }
    fn parse(&self, report: &[u8]) -> Vec<(&'static str, f32)> {
        let mut out = Vec::new();
        for e in report.chunks_exact(3) {
            let usage = u16::from_be_bytes([e[0], e[1]]);
            if usage == 0 && e[2] == 0 {
                break;
            }
            if let Some(code) = usage_to_code(usage) {
                if e[2] > 0 {
                    out.push((code, e[2] as f32 / 255.0));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(vid: u16, pid: u16, page: u16) -> HidInfo {
        HidInfo { path: format!("p{page:x}"), vid, pid, product: None, manufacturer: None, usage_page: page, usage: 1, interface: 0 }
    }

    #[test]
    fn finds_wooting_analog_interface() {
        let list = vec![h(0x31E3, 0x1100, 1), h(0x31E3, 0x1100, WOOTING_ANALOG_PAGE)];
        let (id, path) = detect(&list, 0x31E3, 0x1100).unwrap();
        assert_eq!(id, "wooting");
        assert_eq!(path, "pff54");
    }

    #[test]
    fn ignores_other_keyboards() {
        assert!(detect(&[h(0x046d, 0xc33c, 1)], 0x046d, 0xc33c).is_none());
    }

    #[test]
    fn wooting_entries_become_key_depths() {
        let r = [0x00, 0x1A, 128, 0x00, 0x04, 255, 0x00, 0x2C, 1, 0, 0, 0, 0x00, 0x05, 99];
        let k = Wooting.parse(&r);
        assert_eq!(k.len(), 3, "the zero entry ends the list");
        assert_eq!(k[0].0, "KeyW");
        assert!((k[0].1 - 128.0 / 255.0).abs() < 0.001);
        assert_eq!(k[1], ("KeyA", 1.0));
        assert_eq!(k[2].0, "Space");
    }

    #[test]
    fn unknown_usages_and_zero_depth_are_skipped() {
        let r = [0xFF, 0xFF, 200, 0x00, 0x04, 0, 0x00, 0x05, 10];
        let k = Wooting.parse(&r);
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].0, "KeyB");
    }

    #[test]
    fn short_or_empty_reports_are_safe() {
        assert!(Wooting.parse(&[]).is_empty());
        assert!(Wooting.parse(&[0x00, 0x04]).is_empty());
    }

    #[test]
    fn the_profile_is_marked_unverified() {
        let p = profile("wooting").unwrap();
        assert!(!p.verified());
        assert!(profile("nope").is_none());
    }

    #[test]
    fn usage_table_covers_the_main_block() {
        assert_eq!(usage_to_code(0x04), Some("KeyA"));
        assert_eq!(usage_to_code(0x1D), Some("KeyZ"));
        assert_eq!(usage_to_code(0x27), Some("Digit0"));
        assert_eq!(usage_to_code(0x45), Some("F12"));
        assert_eq!(usage_to_code(0xE1), Some("ShiftLeft"));
        assert_eq!(usage_to_code(0x32), None);
    }

    #[test]
    fn every_mapped_usage_is_a_key_the_layouts_know() {
        for u in 0u16..=0xE7 {
            if let Some(c) = usage_to_code(u) {
                if c == "Pause" {
                    continue;
                }
                assert!(crate::keys::to_set1(c).is_some(), "{c} (usage {u:#x}) has no key-table entry");
            }
        }
    }
}
