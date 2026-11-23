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
