// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! controllers that windows' game-controller layer does not expose (playstation pads)
//! but that hid can read. each decoder turns a raw input report into the same logical
//! control names the rest of the app uses (South, LeftStickX, LeftTrigger2...), so
//! the normal controller view, circularity and drift tests work unchanged.
//!
//! report layouts are the publicly documented dualshock 4 and dualsense ones.
//! UNVERIFIED against real hardware in this repo's tests: they are decoded from
//! hand-built reports. if a control is wrong, the raw table still shows the bytes.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HidPad {
    DualSense,
    DualShock4,
}

pub fn kind_for(vid: u16, pid: u16) -> Option<HidPad> {
    match (vid, pid) {
        (0x054c, 0x0ce6 | 0x0df2) => Some(HidPad::DualSense),
        (0x054c, 0x05c4 | 0x09cc) => Some(HidPad::DualShock4),
        _ => None,
    }
}

/// (logical name, value 0..1 or -1..1 for sticks, is_axis)
pub type Item = (&'static str, f32, bool);

fn stick(b: u8) -> f32 {
    (b as f32 - 127.5) / 127.5
}

/// decode the shared face / shoulder / dpad bytes
fn buttons(out: &mut Vec<Item>, b0: u8, b1: u8, b2: u8) {
    let hat = b0 & 0x0F;
    // 0 = up, 1 = up-right ... 7 = up-left, 8 = released
    let on = |dirs: &[u8]| if dirs.contains(&hat) { 1.0 } else { 0.0 };
    out.push(("DPadUp", on(&[7, 0, 1]), false));
    out.push(("DPadRight", on(&[1, 2, 3]), false));
    out.push(("DPadDown", on(&[3, 4, 5]), false));
    out.push(("DPadLeft", on(&[5, 6, 7]), false));
    let bit = |v: u8, m: u8| if v & m != 0 { 1.0 } else { 0.0 };
    out.push(("West", bit(b0, 0x10), false)); // square
    out.push(("South", bit(b0, 0x20), false)); // cross
    out.push(("East", bit(b0, 0x40), false)); // circle
    out.push(("North", bit(b0, 0x80), false)); // triangle
    out.push(("LeftTrigger", bit(b1, 0x01), false)); // L1
    out.push(("RightTrigger", bit(b1, 0x02), false)); // R1
    out.push(("Select", bit(b1, 0x10), false)); // create / share
    out.push(("Start", bit(b1, 0x20), false)); // options
    out.push(("LeftThumb", bit(b1, 0x40), false));
    out.push(("RightThumb", bit(b1, 0x80), false));
    out.push(("Mode", bit(b2, 0x01), false)); // ps button
    out.push(("Touchpad", bit(b2, 0x02), false));
}

fn sticks(out: &mut Vec<Item>, d: &[u8]) {
    out.push(("LeftStickX", stick(d[0]), true));
    out.push(("LeftStickY", -stick(d[1]), true)); // hid is y-down, the app is y-up
    out.push(("RightStickX", stick(d[2]), true));
    out.push(("RightStickY", -stick(d[3]), true));
}

/// `report` is exactly what hidapi read, report id first.
pub fn decode(kind: HidPad, report: &[u8]) -> Option<Vec<Item>> {
    let id = *report.first()?;
    let mut out = Vec::with_capacity(24);
    match kind {
        HidPad::DualSense => match (id, report.len()) {
            // usb (and bluetooth once the pad switched to its full report): sticks, triggers, then buttons
            (0x01, n) if n >= 11 => {
                let d = &report[1..];
                sticks(&mut out, d);
                out.push(("LeftTrigger2", d[4] as f32 / 255.0, false));
                out.push(("RightTrigger2", d[5] as f32 / 255.0, false));
                buttons(&mut out, d[7], d[8], d[9]);
                out.push(("Mute", if d[9] & 0x04 != 0 { 1.0 } else { 0.0 }, false));
            }
            (0x31, n) if n >= 12 => {
                let d = &report[2..];
                sticks(&mut out, d);
                out.push(("LeftTrigger2", d[4] as f32 / 255.0, false));
                out.push(("RightTrigger2", d[5] as f32 / 255.0, false));
                buttons(&mut out, d[7], d[8], d[9]);
                out.push(("Mute", if d[9] & 0x04 != 0 { 1.0 } else { 0.0 }, false));
            }
            // bluetooth "simple" report, before the pad is switched to the full one
            (0x01, n) if n >= 9 => {
                let d = &report[1..];
                sticks(&mut out, d);
                out.push(("LeftTrigger2", d[7] as f32 / 255.0, false));
                out.push(("RightTrigger2", d[8] as f32 / 255.0, false));
                buttons(&mut out, d[4], d[5], d[6]);
            }
            _ => return None,
        },
        HidPad::DualShock4 => {
            let d = match (id, report.len()) {
                (0x01, n) if n >= 10 => &report[1..],
                (0x11, n) if n >= 13 => &report[3..],
                _ => return None,
            };
            sticks(&mut out, d);
            out.push(("LeftTrigger2", d[7] as f32 / 255.0, false));
            out.push(("RightTrigger2", d[8] as f32 / 255.0, false));
            buttons(&mut out, d[4], d[5], d[6]);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(items: &[Item], name: &str) -> f32 {
        items.iter().find(|i| i.0 == name).unwrap_or_else(|| panic!("no {name}")).1
    }

    /// a dualsense usb report: id, 4 sticks, l2, r2, counter, 3 button bytes
    fn ds5(lx: u8, ly: u8, rx: u8, ry: u8, l2: u8, r2: u8, b0: u8, b1: u8, b2: u8) -> Vec<u8> {
        let mut r = vec![0u8; 64];
        r[0] = 0x01;
        r[1..11].copy_from_slice(&[lx, ly, rx, ry, l2, r2, 0, b0, b1, b2]);
        r
    }

    #[test]
    fn identifies_the_playstation_pads_by_usb_id() {
        assert_eq!(kind_for(0x054c, 0x0ce6), Some(HidPad::DualSense));
        assert_eq!(kind_for(0x054c, 0x0df2), Some(HidPad::DualSense));
        assert_eq!(kind_for(0x054c, 0x09cc), Some(HidPad::DualShock4));
        assert_eq!(kind_for(0x045e, 0x028e), None);
    }

    #[test]
    fn dualsense_idle_is_centred_and_released() {
        let items = decode(HidPad::DualSense, &ds5(128, 128, 127, 127, 0, 0, 0x08, 0, 0)).unwrap();
        assert!(get(&items, "LeftStickX").abs() < 0.01);
        assert!(get(&items, "RightStickY").abs() < 0.01);
        for n in ["South", "East", "West", "North", "DPadUp", "DPadDown", "LeftTrigger", "Mode"] {
            assert_eq!(get(&items, n), 0.0, "{n}");
        }
        assert_eq!(get(&items, "LeftTrigger2"), 0.0);
    }

    #[test]
    fn dualsense_buttons_sticks_and_triggers() {
        // hat right (2) + cross (0x20); l1 + options; ps button; stick full right/up; l2 half, r2 full
        let items = decode(HidPad::DualSense, &ds5(255, 0, 0, 255, 128, 255, 0x22, 0x21, 0x01)).unwrap();
        assert_eq!(get(&items, "South"), 1.0);
        assert_eq!(get(&items, "East"), 0.0);
        assert_eq!(get(&items, "DPadRight"), 1.0);
        assert_eq!(get(&items, "DPadUp"), 0.0);
        assert_eq!(get(&items, "LeftTrigger"), 1.0);
        assert_eq!(get(&items, "Start"), 1.0);
        assert_eq!(get(&items, "Mode"), 1.0);
        assert!(get(&items, "LeftStickX") > 0.99);
        assert!(get(&items, "LeftStickY") > 0.99, "hid y is down, ours is up");
        assert!(get(&items, "RightStickX") < -0.99);
        assert!((get(&items, "LeftTrigger2") - 0.5).abs() < 0.01);
        assert_eq!(get(&items, "RightTrigger2"), 1.0);
    }

    #[test]
    fn dpad_diagonals_light_two_directions() {
        let items = decode(HidPad::DualSense, &ds5(128, 128, 128, 128, 0, 0, 0x01, 0, 0)).unwrap(); // up-right
        assert_eq!(get(&items, "DPadUp"), 1.0);
        assert_eq!(get(&items, "DPadRight"), 1.0);
        assert_eq!(get(&items, "DPadDown"), 0.0);
    }

    #[test]
    fn face_buttons_map_to_the_logical_names() {
        for (mask, name) in [(0x10u8, "West"), (0x20, "South"), (0x40, "East"), (0x80, "North")] {
            let items = decode(HidPad::DualSense, &ds5(128, 128, 128, 128, 0, 0, 0x08 | mask, 0, 0)).unwrap();
            assert_eq!(get(&items, name), 1.0, "{name}");
            let lit = items.iter().filter(|i| ["West", "South", "East", "North"].contains(&i.0) && i.1 > 0.0).count();
            assert_eq!(lit, 1);
        }
    }

    #[test]
    fn dualsense_bluetooth_full_and_simple_reports() {
        let mut bt = vec![0u8; 78];
        bt[0] = 0x31;
        bt[2..12].copy_from_slice(&[255, 128, 128, 128, 0, 0, 0, 0x28, 0, 0]); // cross + hat 0 (up)... 0x28 = cross|hat 8
        let items = decode(HidPad::DualSense, &bt).unwrap();
        assert!(get(&items, "LeftStickX") > 0.99);
        assert_eq!(get(&items, "South"), 1.0);
        // simple: id, 4 sticks, b0, b1, b2, l2, r2
        let simple = [0x01u8, 128, 128, 128, 128, 0x48, 0x02, 0, 10, 20];
        let items = decode(HidPad::DualSense, &simple).unwrap();
        assert_eq!(get(&items, "East"), 1.0);
        assert_eq!(get(&items, "RightTrigger"), 1.0);
        assert!((get(&items, "RightTrigger2") - 20.0 / 255.0).abs() < 0.001);
    }

    #[test]
    fn dualshock4_usb_and_bluetooth() {
        let mut usb = vec![0u8; 64];
        usb[0] = 0x01;
        usb[1..10].copy_from_slice(&[0, 128, 255, 128, 0x88, 0x40, 0x01, 200, 0]); // square? 0x88: hat 8 + triangle; r3
        let items = decode(HidPad::DualShock4, &usb).unwrap();
        assert_eq!(get(&items, "North"), 1.0);
        assert_eq!(get(&items, "RightThumb"), 0.0);
        assert_eq!(get(&items, "LeftThumb"), 1.0);
        assert_eq!(get(&items, "Mode"), 1.0);
        assert!(get(&items, "LeftStickX") < -0.99);
        assert!((get(&items, "LeftTrigger2") - 200.0 / 255.0).abs() < 0.001);
        let mut bt = vec![0u8; 78];
        bt[0] = 0x11;
        bt[3..12].copy_from_slice(&[255, 128, 128, 128, 0x28, 0, 0, 0, 0]);
        let items = decode(HidPad::DualShock4, &bt).unwrap();
        assert_eq!(get(&items, "South"), 1.0);
    }

    #[test]
    fn unknown_or_short_reports_decode_to_nothing() {
        assert!(decode(HidPad::DualSense, &[]).is_none());
        assert!(decode(HidPad::DualSense, &[0x01, 1, 2]).is_none());
        assert!(decode(HidPad::DualSense, &[0x77; 64]).is_none());
        assert!(decode(HidPad::DualShock4, &[0x01; 4]).is_none());
    }
}
