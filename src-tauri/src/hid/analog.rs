//! analog keyboard profiles. a profile knows how to recognise a keyboard's
//! analog hid interface; parsing lands in the hid phase.

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

#[cfg(test)]
mod tests {
    use super::*;

    fn h(vid: u16, pid: u16, page: u16) -> HidInfo {
        HidInfo {
            path: format!("p{page:x}"),
            vid,
            pid,
            product: None,
            manufacturer: None,
            usage_page: page,
            usage: 1,
            interface: 0,
        }
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
}
