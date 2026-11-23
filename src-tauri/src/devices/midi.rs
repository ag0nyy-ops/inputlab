// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
//! midi input ports via midir. listing only here; connecting happens in a session.

use midir::MidiInput;

pub fn list_ports() -> Vec<String> {
    let Ok(input) = MidiInput::new("inputlab-scan") else {
        return vec![];
    };
    input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .collect()
}

/// connect to the port called `name`. the returned guard keeps the connection alive; drop it to close.
pub fn open(name: &str, on_msg: impl Fn(u64, Vec<u8>) + Send + 'static) -> Result<Box<dyn std::any::Any + Send>, String> {
    let input = MidiInput::new("inputlab").map_err(|e| e.to_string())?;
    let port = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).is_ok_and(|n| n == name))
        .ok_or("that midi port is gone")?;
    let conn = input
        .connect(&port, "inputlab-in", move |_stamp, msg, _| on_msg(crate::timing::now_ns(), msg.to_vec()), ())
        .map_err(|e| e.to_string())?;
    Ok(Box::new(conn))
}

const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

pub fn note_name(n: u8) -> String {
    format!("{}{}", NOTES[(n % 12) as usize], n as i32 / 12 - 1)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MidiMsg {
    pub kind: &'static str,
    pub channel: u8,
    pub a: u8,
    pub b: u8,
    /// pitch bend -8192..8191, otherwise b
    pub value: i32,
    pub text: String,
}

/// decode one midi message. running status never reaches us (the os layer resolves it).
pub fn decode(bytes: &[u8]) -> Option<MidiMsg> {
    let s = *bytes.first()?;
    if s < 0x80 {
        return None;
    }
    let ch = s & 0x0F;
    let a = bytes.get(1).copied().unwrap_or(0) & 0x7F;
    let b = bytes.get(2).copied().unwrap_or(0) & 0x7F;
    let (kind, value, text) = match s & 0xF0 {
        0x90 if b > 0 => ("noteOn", b as i32, format!("note on {} velocity {b}", note_name(a))),
        0x90 | 0x80 => ("noteOff", b as i32, format!("note off {}", note_name(a))),
        0xA0 => ("aftertouch", b as i32, format!("key pressure {} {b}", note_name(a))),
        0xB0 => ("cc", b as i32, format!("cc {a} = {b}")),
        0xC0 => ("program", a as i32, format!("program {a}")),
        0xD0 => ("pressure", a as i32, format!("channel pressure {a}")),
        0xE0 => {
            let v = ((b as i32) << 7 | a as i32) - 8192;
            ("pitch", v, format!("pitch bend {v}"))
        }
        _ => ("system", 0, format!("system {:#04x} ({} bytes)", s, bytes.len())),
    };
    Some(MidiMsg { kind, channel: ch + 1, a, b, value, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_notes_with_names() {
        let m = decode(&[0x90, 60, 100]).unwrap();
        assert_eq!((m.kind, m.channel, m.a, m.value), ("noteOn", 1, 60, 100));
        assert_eq!(m.text, "note on C4 velocity 100");
        assert_eq!(note_name(21), "A0");
        assert_eq!(note_name(108), "C8");
    }

    #[test]
    fn note_on_with_zero_velocity_is_a_note_off() {
        assert_eq!(decode(&[0x93, 64, 0]).unwrap().kind, "noteOff");
        assert_eq!(decode(&[0x80, 64, 40]).unwrap().kind, "noteOff");
    }

    #[test]
    fn controllers_pitch_bend_and_system() {
        let cc = decode(&[0xB2, 1, 77]).unwrap();
        assert_eq!((cc.kind, cc.channel, cc.a, cc.value), ("cc", 3, 1, 77));
        assert_eq!(decode(&[0xE0, 0, 64]).unwrap().value, 0, "centre");
        assert_eq!(decode(&[0xE0, 0, 0]).unwrap().value, -8192);
        assert_eq!(decode(&[0xE0, 127, 127]).unwrap().value, 8191);
        assert_eq!(decode(&[0xF8]).unwrap().kind, "system");
    }

    #[test]
    fn junk_is_ignored() {
        assert!(decode(&[]).is_none());
        assert!(decode(&[0x40, 1, 2]).is_none());
        assert!(decode(&[0x90]).is_some(), "a truncated note on still decodes");
    }
}
