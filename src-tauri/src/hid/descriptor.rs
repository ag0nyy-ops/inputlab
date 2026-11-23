// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! hand-rolled hid report descriptor parser: enough of the spec to turn
//! gamepads, joysticks, wheels, pedals, arcade sticks and odd gadgets into
//! labelled buttons and axes. input items only; long items and vendor-defined
//! multi-bit fields are skipped (they still show in the raw hex dump).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Button,
    Axis,
    Hat,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub report_id: u8,
    pub kind: FieldKind,
    pub usage_page: u16,
    pub usage: u16,
    pub bit_offset: usize,
    pub bit_size: usize,
    pub min: i32,
    pub max: i32,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub fields: Vec<Field>,
    pub uses_report_ids: bool,
    /// how many distinct keys the device can report on the keyboard page (modifier bits +
    /// the key array or bitmap). a real keyboard is 100+; hotkey and backlight endpoints are tiny.
    pub key_capacity: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decoded {
    pub id: Option<u8>,
    /// one entry per button, hats expanded to up/right/down/left
    pub buttons: Vec<bool>,
    /// one entry per axis, 0..1
    pub axes: Vec<f32>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AxisInfo {
    pub name: String,
    pub min: i32,
    pub max: i32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutInfo {
    pub parsed: bool,
    pub buttons: Vec<String>,
    pub axes: Vec<AxisInfo>,
    pub report_ids: Vec<u8>,
    pub descriptor_len: usize,
}

fn axis_name(page: u16, usage: u16) -> Option<&'static str> {
    match (page, usage) {
        (0x01, 0x30) => Some("x"),
        (0x01, 0x31) => Some("y"),
        (0x01, 0x32) => Some("z"),
        (0x01, 0x33) => Some("rx"),
        (0x01, 0x34) => Some("ry"),
        (0x01, 0x35) => Some("rz"),
        (0x01, 0x36) => Some("slider"),
        (0x01, 0x37) => Some("dial"),
        (0x01, 0x38) => Some("wheel"),
        (0x02, 0xBA) => Some("rudder"),
        (0x02, 0xBB) => Some("throttle"),
        (0x02, 0xC4) => Some("accelerator"),
        (0x02, 0xC5) => Some("brake"),
        (0x02, 0xC6) => Some("clutch"),
        (0x02, 0xC8) => Some("steering"),
        _ => None,
    }
}

fn signed(v: u32, bytes: usize) -> i32 {
    match bytes {
        1 => v as u8 as i8 as i32,
        2 => v as u16 as i16 as i32,
        _ => v as i32,
    }
}

#[derive(Clone, Default)]
struct Globals {
    page: u16,
    min: i32,
    max: i32,
    size: usize,
    count: usize,
    id: u8,
}

#[derive(Default)]
struct Locals {
    usages: Vec<u32>,
    min: Option<u32>,
    max: Option<u32>,
}

pub fn parse(desc: &[u8]) -> Layout {
    let mut g = Globals::default();
    let mut stack: Vec<Globals> = vec![];
    let mut l = Locals::default();
    let mut offsets: Vec<(u8, usize)> = vec![]; // per report id: bits used so far
    let mut out = Layout::default();
    let mut i = 0;

    while i < desc.len() {
        let b = desc[i];
        if b == 0xFE {
            // long item: skip it whole
            let len = *desc.get(i + 1).unwrap_or(&0) as usize;
            i += 3 + len;
            continue;
        }
        let size = match b & 3 {
            3 => 4,
            n => n as usize,
        };
        let (ty, tag) = ((b >> 2) & 3, b >> 4);
        let end = (i + 1 + size).min(desc.len());
        let data = &desc[i + 1..end];
        let u: u32 = data.iter().enumerate().fold(0, |a, (k, &v)| a | (v as u32) << (8 * k));
        i = end;

        match ty {
            1 => match tag {
                0x0 => g.page = u as u16,
                0x1 => g.min = signed(u, size),
                0x2 => {
                    // a max that does not fit a signed field when min is 0 is unsigned
                    g.max = if g.min >= 0 { u as i32 } else { signed(u, size) };
                }
                0x7 => g.size = u as usize,
                0x8 => {
                    g.id = u as u8;
                    out.uses_report_ids = true;
                }
                0x9 => g.count = u as usize,
                0xA => stack.push(g.clone()),
                0xB => {
                    if let Some(p) = stack.pop() {
                        g = p;
                    }
                }
                _ => {}
            },
            2 => match tag {
                0x0 => l.usages.push(u),
                0x1 => l.min = Some(u),
                0x2 => l.max = Some(u),
                _ => {}
            },
            0 => {
                if tag == 0x8 {
                    // input
                    let slot = match offsets.iter().position(|(id, _)| *id == g.id) {
                        Some(p) => p,
                        None => {
                            offsets.push((g.id, 0));
                            offsets.len() - 1
                        }
                    };
                    let constant = u & 1 != 0;
                    let variable = u & 2 != 0;
                    if g.page == 0x07 && !constant {
                        if variable {
                            if g.size == 1 {
                                out.key_capacity += g.count as u32;
                            }
                        } else {
                            // an array of key codes: capacity is the range of codes it can name
                            let span = (g.max as i64 - g.min as i64 + 1).clamp(0, 4096) as u32;
                            out.key_capacity += span;
                        }
                    }
                    for n in 0..g.count {
                        let off = offsets[slot].1;
                        offsets[slot].1 += g.size;
                        if constant || !variable || g.size == 0 {
                            continue;
                        }
                        let usage: u32 = if !l.usages.is_empty() {
                            *l.usages.get(n).or(l.usages.last()).unwrap()
                        } else if let (Some(a), Some(z)) = (l.min, l.max) {
                            (a + n as u32).min(z)
                        } else {
                            0
                        };
                        let (page, usage) = if usage > 0xFFFF { ((usage >> 16) as u16, (usage & 0xFFFF) as u16) } else { (g.page, usage as u16) };
                        let kind = if page == 0x09 {
                            Some(FieldKind::Button)
                        } else if page == 0x01 && usage == 0x39 {
                            Some(FieldKind::Hat)
                        } else if axis_name(page, usage).is_some() {
                            Some(FieldKind::Axis)
                        } else if g.size == 1 {
                            Some(FieldKind::Button)
                        } else {
                            None
                        };
                        if let Some(kind) = kind {
                            let name = match kind {
                                FieldKind::Button if page == 0x09 => format!("button {usage}"),
                                FieldKind::Button => format!("usage {page:02x}:{usage:02x}"),
                                FieldKind::Hat => "hat".to_string(),
                                FieldKind::Axis => axis_name(page, usage).unwrap().to_string(),
                            };
                            out.fields.push(Field { report_id: g.id, kind, usage_page: page, usage, bit_offset: off, bit_size: g.size, min: g.min, max: g.max, name });
                        }
                    }
                }
                // every main item clears the locals; the descriptor is the spec
                l = Locals::default();
            }
            _ => {}
        }
    }
    out
}

fn bits(data: &[u8], off: usize, size: usize) -> Option<u32> {
    if size == 0 || size > 32 || (off + size).div_ceil(8) > data.len() {
        return None;
    }
    let mut v = 0u32;
    for k in 0..size {
        let bit = off + k;
        if data[bit / 8] >> (bit % 8) & 1 != 0 {
            v |= 1 << k;
        }
    }
    Some(v)
}

impl Layout {
    pub fn info(&self, descriptor_len: usize) -> LayoutInfo {
        let mut buttons = vec![];
        let mut axes = vec![];
        for f in &self.fields {
            match f.kind {
                FieldKind::Button => buttons.push(f.name.clone()),
                FieldKind::Hat => buttons.extend(["hat up", "hat right", "hat down", "hat left"].map(String::from)),
                FieldKind::Axis => axes.push(AxisInfo { name: f.name.clone(), min: f.min, max: f.max }),
            }
        }
        let mut ids: Vec<u8> = self.fields.iter().map(|f| f.report_id).collect();
        ids.sort_unstable();
        ids.dedup();
        LayoutInfo { parsed: !self.fields.is_empty(), buttons, axes, report_ids: if self.uses_report_ids { ids } else { vec![] }, descriptor_len }
    }

    /// decode one input report. buttons and axes come out in the same order as `info()`.
    pub fn decode(&self, report: &[u8]) -> Decoded {
        let (id, payload) = if self.uses_report_ids && !report.is_empty() { (Some(report[0]), &report[1..]) } else { (None, report) };
        let mut d = Decoded { id, ..Default::default() };
        for f in &self.fields {
            let mine = id.is_none_or(|i| i == f.report_id);
            let raw = if mine { bits(payload, f.bit_offset, f.bit_size) } else { None };
            match f.kind {
                FieldKind::Button => d.buttons.push(raw.is_some_and(|v| v != 0)),
                FieldKind::Hat => {
                    let dir = raw.map(|v| v as i32 - f.min).filter(|v| (0..8).contains(v));
                    // 0 = up, 1 = up-right, 2 = right ... 7 = up-left
                    d.buttons.push(dir.is_some_and(|d| matches!(d, 7 | 0 | 1)));
                    d.buttons.push(dir.is_some_and(|d| matches!(d, 1 | 2 | 3)));
                    d.buttons.push(dir.is_some_and(|d| matches!(d, 3 | 4 | 5)));
                    d.buttons.push(dir.is_some_and(|d| matches!(d, 5 | 6 | 7)));
                }
                FieldKind::Axis => {
                    let v = raw.map(|r| if f.min < 0 { signed(r, f.bit_size.div_ceil(8)) as f32 } else { r as f32 });
                    let span = (f.max - f.min) as f32;
                    d.axes.push(match v {
                        Some(v) if span > 0.0 => ((v - f.min as f32) / span).clamp(0.0, 1.0),
                        _ => 0.0,
                    });
                }
            }
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a typical 16 button, 4 axis, 1 hat gamepad
    const GAMEPAD: &[u8] = &[
        0x05, 0x01, 0x09, 0x05, 0xA1, 0x01, //
        0x05, 0x09, 0x19, 0x01, 0x29, 0x10, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x10, 0x81, 0x02, //
        0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x32, 0x09, 0x35, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x04, 0x81, 0x02, //
        0x05, 0x01, 0x09, 0x39, 0x15, 0x00, 0x25, 0x07, 0x75, 0x04, 0x95, 0x01, 0x81, 0x42, //
        0x75, 0x04, 0x95, 0x01, 0x81, 0x03, //
        0xC0,
    ];

    /// the boot mouse descriptor
    const MOUSE: &[u8] = &[
        0x05, 0x01, 0x09, 0x02, 0xA1, 0x01, 0x09, 0x01, 0xA1, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29, 0x03, 0x15, 0x00, 0x25, 0x01, 0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01,
        0x75, 0x05, 0x81, 0x01, 0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x38, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x03, 0x81, 0x06, 0xC0, 0xC0,
    ];

    #[test]
    fn a_boot_keyboard_declares_a_full_set_of_keys() {
        // 8 modifier bits + an array of 6 key codes 0..101
        let desc = [
            0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x05, 0x07, 0x19, 0xE0, 0x29, 0xE7, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x06,
            0x75, 0x08, 0x15, 0x00, 0x25, 0x65, 0x05, 0x07, 0x19, 0x00, 0x29, 0x65, 0x81, 0x00, 0xC0,
        ];
        assert!(parse(&desc).key_capacity >= 100);
    }

    #[test]
    fn a_hotkey_endpoint_declares_almost_no_keys() {
        // four single-bit keys on the keyboard page
        let desc = [0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x05, 0x07, 0x19, 0x3A, 0x29, 0x3D, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x04, 0x81, 0x02, 0xC0];
        let c = parse(&desc).key_capacity;
        assert!(c > 0 && c < 40, "{c}");
    }

    #[test]
    fn parses_a_gamepad() {
        let l = parse(GAMEPAD);
        let info = l.info(GAMEPAD.len());
        assert!(info.parsed);
        assert_eq!(info.buttons.len(), 16 + 4, "16 buttons + the hat as 4 directions");
        assert_eq!(info.axes.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), vec!["x", "y", "z", "rz"]);
        assert_eq!(info.buttons[0], "button 1");
        assert_eq!(info.buttons[15], "button 16");
        assert!(info.report_ids.is_empty());
    }

    #[test]
    fn decodes_a_gamepad_report() {
        let l = parse(GAMEPAD);
        // buttons 1 and 3, x full right, y centre, z full left, rz centre, hat pointing right
        let d = l.decode(&[0b0000_0101, 0x00, 0x7F, 0x00, 0x81, 0x00, 0x02]);
        assert!(d.buttons[0] && !d.buttons[1] && d.buttons[2]);
        assert!(d.axes[0] > 0.99, "x {}", d.axes[0]);
        assert!((d.axes[1] - 0.5).abs() < 0.01);
        assert!(d.axes[2] < 0.01);
        assert!(d.buttons[16 + 1], "hat right");
        assert!(!d.buttons[16] && !d.buttons[16 + 2] && !d.buttons[16 + 3]);
    }

    #[test]
    fn hat_diagonals_light_two_directions_and_neutral_none() {
        let l = parse(GAMEPAD);
        let up_right = l.decode(&[0, 0, 0, 0, 0, 0, 0x01]);
        assert!(up_right.buttons[16] && up_right.buttons[17] && !up_right.buttons[18]);
        let neutral = l.decode(&[0, 0, 0, 0, 0, 0, 0x0F]); // out of range = released
        assert!(neutral.buttons[16..20].iter().all(|b| !b));
    }

    #[test]
    fn parses_the_boot_mouse() {
        let l = parse(MOUSE);
        let info = l.info(MOUSE.len());
        assert_eq!(info.buttons.len(), 3);
        assert_eq!(info.axes.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), vec!["x", "y", "wheel"]);
        let d = l.decode(&[0b0000_0010, 0x7F, 0x81, 0x00]);
        assert!(d.buttons[1] && !d.buttons[0]);
        assert!(d.axes[0] > 0.99 && d.axes[1] < 0.01);
    }

    #[test]
    fn report_ids_select_the_right_fields() {
        // report 1: 8 buttons. report 2: one 8-bit x axis.
        let desc = [
            0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, //
            0x85, 0x01, 0x05, 0x09, 0x19, 0x01, 0x29, 0x08, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, //
            0x85, 0x02, 0x05, 0x01, 0x09, 0x30, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02, //
            0xC0,
        ];
        let l = parse(&desc);
        let info = l.info(desc.len());
        assert_eq!(info.report_ids, vec![1, 2]);
        let r1 = l.decode(&[1, 0b1000_0000]);
        assert_eq!(r1.id, Some(1));
        assert!(r1.buttons[7]);
        let r2 = l.decode(&[2, 0xFF]);
        assert_eq!(r2.id, Some(2));
        assert!(r2.axes[0] > 0.99);
        assert!(r2.buttons.iter().all(|b| !b), "report 2 carries no buttons");
    }

    #[test]
    fn sixteen_bit_signed_axes_normalise() {
        let desc = [0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, 0x09, 0x30, 0x16, 0x00, 0x80, 0x26, 0xFF, 0x7F, 0x75, 0x10, 0x95, 0x01, 0x81, 0x02, 0xC0];
        let l = parse(&desc);
        assert!((l.decode(&0i16.to_le_bytes())).axes[0] > 0.49);
        assert!(l.decode(&i16::MIN.to_le_bytes()).axes[0] < 0.01);
        assert!(l.decode(&i16::MAX.to_le_bytes()).axes[0] > 0.99);
    }

    #[test]
    fn pedals_and_wheels_get_names() {
        let desc = [0x05, 0x02, 0x09, 0xC4, 0x09, 0xC5, 0x09, 0xC8, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x03, 0x81, 0x02];
        let names: Vec<_> = parse(&desc).info(0).axes.iter().map(|a| a.name.clone()).collect();
        assert_eq!(names, vec!["accelerator", "brake", "steering"]);
    }

    #[test]
    fn garbage_and_truncated_descriptors_never_panic() {
        for d in [&[][..], &[0xFF], &[0xFE, 0xFF], &[0x05], &[0x75, 0x08, 0x95, 0x08, 0x81], &GAMEPAD[..30], &[0x81, 0x02, 0x81, 0x02]] {
            let l = parse(d);
            l.decode(&[]);
            l.decode(&[1, 2, 3]);
            let _ = l.info(d.len());
        }
        // pseudo-random bytes
        let mut x = 12345u32;
        let junk: Vec<u8> = (0..500).map(|_| { x = x.wrapping_mul(1664525).wrapping_add(1013904223); (x >> 24) as u8 }).collect();
        let l = parse(&junk);
        l.decode(&junk);
    }

    #[test]
    fn short_reports_do_not_panic_decoding() {
        let l = parse(GAMEPAD);
        let d = l.decode(&[0xFF]);
        assert_eq!(d.buttons.len(), 20);
    }
}
