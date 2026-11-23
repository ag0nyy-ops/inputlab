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

/// `report` is exactly what hidapi read, report id first. `bluetooth` matters because windows pads
/// the short bluetooth "simple" report out to the full report size, so the length cannot tell the two
/// layouts apart: the bus has to.
pub fn decode(kind: HidPad, bluetooth: bool, report: &[u8]) -> Option<Vec<Item>> {
    let id = *report.first()?;
    let mut out = Vec::with_capacity(24);
    match kind {
        HidPad::DualSense => match id {
            // bluetooth before the pad switches to its full report: the short, dualshock-like layout
            0x01 if bluetooth && report.len() >= 9 => {
                let d = &report[1..];
                sticks(&mut out, d);
                out.push(("LeftTrigger2", d[7] as f32 / 255.0, false));
                out.push(("RightTrigger2", d[8] as f32 / 255.0, false));
                buttons(&mut out, d[4], d[5], d[6]);
            }
            // usb: sticks, triggers, then buttons
            0x01 if report.len() >= 11 => ds5_full(&mut out, &report[1..]),
            // bluetooth full report
            0x31 if report.len() >= 12 => ds5_full(&mut out, &report[2..]),
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

fn ds5_full(out: &mut Vec<Item>, d: &[u8]) {
    sticks(out, d);
    out.push(("LeftTrigger2", d[4] as f32 / 255.0, false));
    out.push(("RightTrigger2", d[5] as f32 / 255.0, false));
    buttons(out, d[7], d[8], d[9]);
    out.push(("Mute", if d[9] & 0x04 != 0 { 1.0 } else { 0.0 }, false));
}

/// ("wired" | "discharging" | "charging" | "charged", percent). None when the report does not carry it.
pub fn battery(kind: HidPad, bluetooth: bool, report: &[u8]) -> Option<(&'static str, Option<u8>)> {
    let id = *report.first()?;
    match kind {
        HidPad::DualSense => {
            let status = match id {
                0x01 if !bluetooth && report.len() > 53 => report[53],
                0x31 if report.len() > 54 => report[54],
                _ => return None,
            };
            let level = status & 0x0F;
            let pct = (level as u32 * 10 + 5).min(100) as u8;
            match (status >> 4) & 0x0F {
                0 => Some(("discharging", Some(pct))),
                1 => Some(("charging", Some(pct))),
                2 => Some(("charged", Some(100))),
                _ => None,
            }
        }
        HidPad::DualShock4 => {
            let status = match (id, report.len()) {
                (0x01, n) if n > 30 => report[30],
                (0x11, n) if n > 32 => report[32],
                _ => return None,
            };
            let level = (status & 0x0F) as u32;
            if status & 0x10 != 0 {
                // cable plugged in: the scale is 0..10 and 11 means full
                if level >= 11 { Some(("charged", Some(100))) } else { Some(("charging", Some((level * 10).min(100) as u8))) }
            } else {
                Some(("discharging", Some((level * 10 + 5).min(100) as u8)))
            }
        }
    }
}

/// standard crc-32 (the one zlib uses)
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// bluetooth output reports end with a crc-32 over a 0xA2 seed byte and everything before the checksum
fn with_bt_crc(mut r: Vec<u8>) -> Vec<u8> {
    let n = r.len() - 4;
    let mut seeded = vec![0xA2u8];
    seeded.extend_from_slice(&r[..n]);
    let c = crc32(&seeded).to_le_bytes();
    r[n..].copy_from_slice(&c);
    r
}

static SEQ: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// an output report that runs the two rumble motors. the dualsense works over usb and bluetooth
/// (bluetooth needs the crc); the dualshock 4 only over usb here.
pub fn rumble_report(kind: HidPad, bluetooth: bool, strong: f32, weak: f32) -> Option<Vec<u8>> {
    let (s, w) = ((strong.clamp(0.0, 1.0) * 255.0) as u8, (weak.clamp(0.0, 1.0) * 255.0) as u8);
    match (kind, bluetooth) {
        (HidPad::DualSense, false) => {
            let mut r = vec![0u8; 48];
            r[0] = 0x02;
            r[1] = 0x03; // compatible vibration + haptics select
            r[3] = w; // right (small) motor
            r[4] = s; // left (big) motor
            r[39] = 0x04; // compatible vibration v2, newer firmware
            Some(r)
        }
        (HidPad::DualSense, true) => {
            let mut r = vec![0u8; 78];
            r[0] = 0x31;
            r[1] = (SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) & 0x0F) << 4;
            r[2] = 0x10;
            r[3] = 0x03;
            r[5] = w;
            r[6] = s;
            r[41] = 0x04;
            Some(with_bt_crc(r))
        }
        (HidPad::DualShock4, false) => {
            let mut r = vec![0u8; 32];
            r[0] = 0x05;
            r[1] = 0x01; // rumble
            r[4] = w;
            r[5] = s;
            Some(r)
        }
        (HidPad::DualShock4, true) => None,
    }
}


/// the motion sensors, raw 16-bit readings. gyro order is pitch, yaw, roll as the pad sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motion {
    pub gyro: [i16; 3],
    pub accel: [i16; 3],
}

fn le16(d: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([d[at], d[at + 1]])
}

/// gyro and accelerometer from an input report. the dualsense and dualshock 4 both carry them in
/// their full reports; the short bluetooth "simple" report has none.
pub fn motion(kind: HidPad, bluetooth: bool, report: &[u8]) -> Option<Motion> {
    let id = *report.first()?;
    let (d, gyro_at, accel_at) = match kind {
        HidPad::DualSense => match id {
            0x01 if !bluetooth && report.len() >= 28 => (&report[1..], 15, 21),
            0x31 if report.len() >= 29 => (&report[2..], 15, 21),
            _ => return None,
        },
        HidPad::DualShock4 => match id {
            0x01 if !bluetooth && report.len() >= 25 => (&report[1..], 12, 18),
            0x11 if report.len() >= 27 => (&report[3..], 12, 18),
            _ => return None,
        },
    };
    Some(Motion {
        gyro: [le16(d, gyro_at), le16(d, gyro_at + 2), le16(d, gyro_at + 4)],
        accel: [le16(d, accel_at), le16(d, accel_at + 2), le16(d, accel_at + 4)],
    })
}

/// a dualsense output report with the given valid-flag bits and a writer for its payload. the usb
/// layout starts the payload at byte 1; bluetooth moves everything two bytes along and adds a crc.
fn ds5_output(bluetooth: bool, flag0: u8, fill: impl Fn(&mut dyn FnMut(usize, u8))) -> Vec<u8> {
    if bluetooth {
        let mut r = vec![0u8; 78];
        r[0] = 0x31;
        r[1] = (SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) & 0x0F) << 4;
        r[2] = 0x10;
        r[3] = flag0;
        fill(&mut |i, v| r[i + 2] = v);
        with_bt_crc(r)
    } else {
        let mut r = vec![0u8; 48];
        r[0] = 0x02;
        r[1] = flag0;
        fill(&mut |i, v| r[i] = v);
        r
    }
}

/// adaptive trigger: `Some((start, force))` makes the trigger push back from `start` (0..1 of its
/// travel) with `force` (0..1); `None` switches the effect off. dualsense only.
pub fn trigger_report(kind: HidPad, bluetooth: bool, right: bool, effect: Option<(f32, f32)>) -> Option<Vec<u8>> {
    if kind != HidPad::DualSense {
        return None;
    }
    let base = if right { 11 } else { 22 };
    let flag = if right { 0x04 } else { 0x08 };
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(ds5_output(bluetooth, flag, |w| match effect {
        Some((start, force)) => {
            w(base, 0x01); // continuous resistance
            w(base + 1, byte(start));
            w(base + 2, byte(force));
        }
        None => w(base, 0x00),
    }))
}

/// speaker and microphone levels (0..1) for the controller's own audio, sent so the usb audio
/// device is audible. usb only: the controller has no audio over bluetooth.
pub fn audio_report(kind: HidPad, bluetooth: bool, speaker: f32, mic: f32) -> Option<Vec<u8>> {
    if kind != HidPad::DualSense || bluetooth {
        return None;
    }
    // the speaker only does anything between about 0x3d and 0x64
    let sp = if speaker <= 0.0 { 0 } else { (0x3d as f32 + speaker.clamp(0.0, 1.0) * (0x64 - 0x3d) as f32) as u8 };
    let mc = (mic.clamp(0.0, 1.0) * 255.0) as u8;
    Some(ds5_output(false, 0xF0, |w| {
        w(5, sp); // headphone volume follows the speaker
        w(6, sp);
        w(7, mc);
        w(8, 0x30); // route audio to the speaker
    }))
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
        let items = decode(HidPad::DualSense, false, &ds5(128, 128, 127, 127, 0, 0, 0x08, 0, 0)).unwrap();
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
        let items = decode(HidPad::DualSense, false, &ds5(255, 0, 0, 255, 128, 255, 0x22, 0x21, 0x01)).unwrap();
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
        let items = decode(HidPad::DualSense, false, &ds5(128, 128, 128, 128, 0, 0, 0x01, 0, 0)).unwrap(); // up-right
        assert_eq!(get(&items, "DPadUp"), 1.0);
        assert_eq!(get(&items, "DPadRight"), 1.0);
        assert_eq!(get(&items, "DPadDown"), 0.0);
    }

    #[test]
    fn face_buttons_map_to_the_logical_names() {
        for (mask, name) in [(0x10u8, "West"), (0x20, "South"), (0x40, "East"), (0x80, "North")] {
            let items = decode(HidPad::DualSense, false, &ds5(128, 128, 128, 128, 0, 0, 0x08 | mask, 0, 0)).unwrap();
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
        let items = decode(HidPad::DualSense, false, &bt).unwrap();
        assert!(get(&items, "LeftStickX") > 0.99);
        assert_eq!(get(&items, "South"), 1.0);
        // simple: id, 4 sticks, b0, b1, b2, l2, r2
        let simple = [0x01u8, 128, 128, 128, 128, 0x48, 0x02, 0, 10, 20];
        let items = decode(HidPad::DualSense, true, &simple).unwrap();
        assert_eq!(get(&items, "East"), 1.0);
        assert_eq!(get(&items, "RightTrigger"), 1.0);
        assert!((get(&items, "RightTrigger2") - 20.0 / 255.0).abs() < 0.001);
    }

    #[test]
    fn dualshock4_usb_and_bluetooth() {
        let mut usb = vec![0u8; 64];
        usb[0] = 0x01;
        usb[1..10].copy_from_slice(&[0, 128, 255, 128, 0x88, 0x40, 0x01, 200, 0]); // square? 0x88: hat 8 + triangle; r3
        let items = decode(HidPad::DualShock4, false, &usb).unwrap();
        assert_eq!(get(&items, "North"), 1.0);
        assert_eq!(get(&items, "RightThumb"), 0.0);
        assert_eq!(get(&items, "LeftThumb"), 1.0);
        assert_eq!(get(&items, "Mode"), 1.0);
        assert!(get(&items, "LeftStickX") < -0.99);
        assert!((get(&items, "LeftTrigger2") - 200.0 / 255.0).abs() < 0.001);
        let mut bt = vec![0u8; 78];
        bt[0] = 0x11;
        bt[3..12].copy_from_slice(&[255, 128, 128, 128, 0x28, 0, 0, 0, 0]);
        let items = decode(HidPad::DualShock4, false, &bt).unwrap();
        assert_eq!(get(&items, "South"), 1.0);
    }

    #[test]
    fn windows_pads_the_bluetooth_simple_report_and_the_bus_picks_the_layout() {
        // a 10 byte simple report padded with zeros to 78 bytes, as windows hands it over
        let mut padded = vec![0u8; 78];
        padded[..10].copy_from_slice(&[0x01u8, 255, 128, 128, 128, 0x48, 0x02, 0, 10, 20]);
        let bt = decode(HidPad::DualSense, true, &padded).unwrap();
        assert!(get(&bt, "LeftStickX") > 0.99);
        assert_eq!(get(&bt, "East"), 1.0);
        assert_eq!(get(&bt, "RightTrigger"), 1.0);
        // the same bytes read as a usb report would put the sticks and buttons in the wrong places
        let wrong = decode(HidPad::DualSense, false, &padded).unwrap();
        assert_ne!(get(&wrong, "East"), 1.0);
    }

    #[test]
    fn dualsense_battery_charging_and_full() {
        let mut r = vec![0u8; 64];
        r[0] = 0x01;
        r[53] = 0x06; // discharging, level 6
        assert_eq!(battery(HidPad::DualSense, false, &r), Some(("discharging", Some(65))));
        r[53] = 0x18; // charging, level 8
        assert_eq!(battery(HidPad::DualSense, false, &r), Some(("charging", Some(85))));
        r[53] = 0x2A; // full
        assert_eq!(battery(HidPad::DualSense, false, &r), Some(("charged", Some(100))));
        assert_eq!(battery(HidPad::DualSense, false, &r[..20]), None);
    }

    #[test]
    fn dualshock4_battery_with_and_without_the_cable() {
        let mut r = vec![0u8; 64];
        r[0] = 0x01;
        r[30] = 0x04;
        assert_eq!(battery(HidPad::DualShock4, false, &r), Some(("discharging", Some(45))));
        r[30] = 0x10 | 0x05;
        assert_eq!(battery(HidPad::DualShock4, false, &r), Some(("charging", Some(50))));
        r[30] = 0x10 | 0x0B;
        assert_eq!(battery(HidPad::DualShock4, false, &r), Some(("charged", Some(100))));
    }

    #[test]
    fn rumble_reports_put_the_motors_in_the_right_bytes_and_skip_bluetooth() {
        let r = rumble_report(HidPad::DualSense, false, 1.0, 0.5).unwrap();
        assert_eq!((r[0], r[1], r[3], r[4], r.len()), (0x02, 0x03, 127, 255, 48));
        let r = rumble_report(HidPad::DualShock4, false, 0.0, 1.0).unwrap();
        assert_eq!((r[0], r[4], r[5], r.len()), (0x05, 255, 0, 32));
        assert!(rumble_report(HidPad::DualShock4, true, 1.0, 1.0).is_none());
        let off = rumble_report(HidPad::DualSense, false, 0.0, 0.0).unwrap();
        assert_eq!((off[3], off[4]), (0, 0));
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn dualsense_bluetooth_rumble_report_is_78_bytes_with_a_valid_crc() {
        let r = rumble_report(HidPad::DualSense, true, 1.0, 0.5).unwrap();
        assert_eq!((r[0], r[2], r[3], r[5], r[6], r.len()), (0x31, 0x10, 0x03, 127, 255, 78));
        let mut seeded = vec![0xA2u8];
        seeded.extend_from_slice(&r[..74]);
        assert_eq!(u32::from_le_bytes([r[74], r[75], r[76], r[77]]), crc32(&seeded));
        // the stop report is also valid
        let off = rumble_report(HidPad::DualSense, true, 0.0, 0.0).unwrap();
        assert_eq!((off[5], off[6]), (0, 0));
    }

    #[test]
    fn unknown_or_short_reports_decode_to_nothing() {
        assert!(decode(HidPad::DualSense, false, &[]).is_none());
        assert!(decode(HidPad::DualSense, false, &[0x01, 1, 2]).is_none());
        assert!(decode(HidPad::DualSense, false, &[0x77; 64]).is_none());
        assert!(decode(HidPad::DualShock4, false, &[0x01; 4]).is_none());
    }

    #[test]
    fn motion_is_read_from_the_dualsense_and_dualshock_reports() {
        let mut r = vec![0u8; 64];
        r[0] = 0x01;
        r[16..18].copy_from_slice(&(-1234i16).to_le_bytes()); // gyro pitch
        r[20..22].copy_from_slice(&500i16.to_le_bytes()); // gyro roll
        r[22..24].copy_from_slice(&100i16.to_le_bytes()); // accel x
        r[26..28].copy_from_slice(&8192i16.to_le_bytes()); // accel z
        let m = motion(HidPad::DualSense, false, &r).unwrap();
        assert_eq!((m.gyro, m.accel), ([-1234, 0, 500], [100, 0, 8192]));
        // bluetooth full report: everything one byte further
        let mut bt = vec![0u8; 78];
        bt[0] = 0x31;
        bt[17..19].copy_from_slice(&(-1234i16).to_le_bytes());
        assert_eq!(motion(HidPad::DualSense, true, &bt).unwrap().gyro[0], -1234);
        // the short bluetooth report and short buffers have none
        assert!(motion(HidPad::DualSense, true, &r).is_none());
        assert!(motion(HidPad::DualSense, false, &r[..20]).is_none());
        let mut d4 = vec![0u8; 64];
        d4[0] = 0x01;
        d4[13..15].copy_from_slice(&777i16.to_le_bytes());
        d4[19..21].copy_from_slice(&(-8000i16).to_le_bytes());
        let m = motion(HidPad::DualShock4, false, &d4).unwrap();
        assert_eq!((m.gyro[0], m.accel[0]), (777, -8000));
    }

    #[test]
    fn trigger_reports_target_the_right_bytes_and_bluetooth_gets_a_crc() {
        let r = trigger_report(HidPad::DualSense, false, true, Some((0.5, 1.0))).unwrap();
        assert_eq!((r[0], r[1], r[11], r[12], r[13], r.len()), (0x02, 0x04, 0x01, 128, 255, 48));
        let l = trigger_report(HidPad::DualSense, false, false, Some((0.0, 0.25))).unwrap();
        assert_eq!((l[1], l[22], l[24]), (0x08, 0x01, 64));
        let off = trigger_report(HidPad::DualSense, false, false, None).unwrap();
        assert_eq!((off[1], off[22]), (0x08, 0x00));
        let bt = trigger_report(HidPad::DualSense, true, true, Some((0.5, 1.0))).unwrap();
        assert_eq!((bt[0], bt[3], bt[13], bt[15], bt.len()), (0x31, 0x04, 0x01, 255, 78));
        let mut seeded = vec![0xA2u8];
        seeded.extend_from_slice(&bt[..74]);
        assert_eq!(u32::from_le_bytes([bt[74], bt[75], bt[76], bt[77]]), crc32(&seeded));
        assert!(trigger_report(HidPad::DualShock4, false, true, None).is_none());
    }

    #[test]
    fn audio_reports_set_the_levels_and_only_exist_over_usb() {
        let r = audio_report(HidPad::DualSense, false, 1.0, 0.5).unwrap();
        assert_eq!((r[0], r[1], r[6], r[7], r[8]), (0x02, 0xF0, 0x64, 127, 0x30));
        assert_eq!(audio_report(HidPad::DualSense, false, 0.0, 0.0).unwrap()[6], 0);
        assert!(audio_report(HidPad::DualSense, true, 1.0, 1.0).is_none());
        assert!(audio_report(HidPad::DualShock4, false, 1.0, 1.0).is_none());
    }
}
