// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! device model + the pure logic that merges every backend's findings into
//! one list. scanning (the io part) lives in `hub`; merging lives here so it
//! can be unit tested with fake inputs.

pub mod gamepad;
pub mod hub;
pub mod midi;

use serde::Serialize;

use crate::hid::{analog, HidInfo};
use crate::platform::RawDevice;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Keyboard,
    Mouse,
    Gamepad,
    Hid,
    Midi,
}

/// which frontend view renders a device. the frontend never has to guess
/// from kind + backend.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Viewer {
    Keyboard,
    Mouse,
    Gamepad,
    Hid,
    Midi,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub viewer: Viewer,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub connected: bool,
    /// the raw backend that reads this device: rawinput, evdev, iokit, gilrs, hidapi, midir
    pub backend: String,
    /// how many os-level interfaces were folded into this entry
    pub interfaces: u32,
    pub analog_profile: Option<String>,
    pub note: Option<String>,
    /// false for interfaces windows (or the device) exposes that nobody can hold: hidden by default
    pub usable: bool,
    pub hidden_reason: Option<String>,
    /// other os-level interfaces of this physical device that were folded in and hidden
    pub hidden_interfaces: u32,

    /// raw-input handles that belong to this device (keyboards, mice)
    #[serde(skip)]
    pub srcs: Vec<u64>,
    /// hidapi paths: the primary read path first
    #[serde(skip)]
    pub hid_paths: Vec<String>,
    /// vendor-specific hid path used for analog data on analog keyboards
    #[serde(skip)]
    pub analog_path: Option<String>,
    #[serde(skip)]
    pub pad_id: Option<usize>,
    /// the device talks over bluetooth (changes how some pads' reports are laid out)
    #[serde(skip)]
    pub bluetooth: bool,
}

impl DeviceInfo {
    pub(crate) fn base(id: String, name: String, kind: DeviceKind, viewer: Viewer, backend: &str) -> Self {
        Self {
            id,
            name,
            kind,
            viewer,
            vid: None,
            pid: None,
            connected: true,
            backend: backend.to_string(),
            interfaces: 1,
            analog_profile: None,
            note: None,
            usable: true,
            hidden_reason: None,
            hidden_interfaces: 0,
            srcs: vec![],
            hid_paths: vec![],
            analog_path: None,
            pad_id: None,
            bluetooth: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PadInfo {
    pub id: usize,
    pub name: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub uuid: String,
    pub force_feedback: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ScanInputs {
    pub raw: Vec<RawDevice>,
    pub hid: Vec<HidInfo>,
    pub pads: Vec<PadInfo>,
    pub midi: Vec<String>,
    pub raw_backend: String,
}

const USAGE_PAGE_GENERIC: u16 = 0x01;
const USAGE_PAGE_SIM: u16 = 0x02;
const USAGE_PAGE_CONSUMER: u16 = 0x0C;
const USAGE_PAGE_DIGITIZER: u16 = 0x0D;

fn is_vendor_page(page: u16) -> bool {
    page >= 0xFF00
}

fn fmt_id(vid: Option<u16>, pid: Option<u16>) -> String {
    match (vid, pid) {
        (Some(v), Some(p)) => format!("{v:04x}{p:04x}"),
        _ => "sys".to_string(),
    }
}

fn product_for(hid: &[HidInfo], vid: Option<u16>, pid: Option<u16>) -> Option<String> {
    let (v, p) = (vid?, pid?);
    hid.iter()
        .filter(|h| h.vid == v && h.pid == p)
        .filter_map(|h| h.product.clone())
        .find(|s| !s.trim().is_empty())
}

fn maker_for(hid: &[HidInfo], vid: Option<u16>, pid: Option<u16>) -> Option<String> {
    let (v, p) = (vid?, pid?);
    hid.iter()
        .filter(|h| h.vid == v && h.pid == p)
        .filter_map(|h| h.manufacturer.clone())
        .find(|s| !s.trim().is_empty() && !s.to_lowercase().contains("(standard"))
}

fn vendor_name(vid: u16) -> Option<&'static str> {
    Some(match vid {
        0x046d => "Logitech",
        0x045e => "Microsoft",
        0x1532 => "Razer",
        0x1038 => "SteelSeries",
        0x054c => "Sony",
        0x057e => "Nintendo",
        0x1b1c => "Corsair",
        0x046a => "Cherry",
        0x17ef => "Lenovo",
        0x413c => "Dell",
        0x03f0 => "HP",
        0x0b05 => "Asus",
        0x258a | 0x320f => "Glorious",
        0x3434 => "Keychron",
        0x0951 => "HyperX",
        0x1e7d => "Roccat",
        _ => return None,
    })
}

/// hid endpoints that firmware vendors expose next to a real keyboard (hotkeys, backlight, ec).
fn is_plumbing_name(p: &str) -> bool {
    let l = p.trim().to_lowercase();
    l.contains("backlight") || l.contains("hotkey")
}

/// windows names an ITE keyboard controller "ITE Device(8295)". 8000-8999 are the laptop's own keyboard
/// (the legacy ps/2 "built-in keyboard" next to them is only a placeholder).
fn ite_laptop_keyboard(name: &str) -> bool {
    let l = name.trim().to_lowercase();
    l.strip_prefix("ite device(")
        .and_then(|r| r.strip_suffix(')'))
        .and_then(|n| n.parse::<u32>().ok())
        .is_some_and(|n| (8000..9000).contains(&n))
}

/// names windows gives to nearly everything: they say nothing about what the thing is
fn generic_product(p: &str) -> bool {
    let l = p.to_lowercase();
    ["receiver", "dongle", "usb input device", "hid-compliant", "usb device", "hid keyboard", "hid mouse", "composite"].iter().any(|g| l.contains(g)) || l.trim() == "mouse" || l.trim() == "keyboard"
}

/// a name a person recognises. a product string that actually names the model wins; "usb receiver"
/// does not, so a wireless mouse becomes "logitech wireless mouse (usb receiver)" and the model picker
/// takes it from there. devices with no usb ids are told apart by what their windows path says.
fn friendly_raw_name(kind: DeviceKind, vid: Option<u16>, pid: Option<u16>, product: Option<String>, maker: Option<String>, paths: &[&str]) -> String {
    let label = if kind == DeviceKind::Keyboard { "keyboard" } else { "mouse" };
    if vid.is_some() && pid.is_some() {
        match product {
            Some(p) if !generic_product(&p) => return p,
            p => {
                let wireless = p.as_deref().is_some_and(|x| {
                    let l = x.to_lowercase();
                    l.contains("receiver") || l.contains("dongle")
                });
                let who = maker.or_else(|| vid.and_then(vendor_name).map(String::from)).unwrap_or_default();
                let what = if wireless { format!("wireless {label} (usb receiver)") } else { format!("usb {label}") };
                return format!("{who} {what}").trim().to_string();
            }
        }
    }
    let up: Vec<String> = paths.iter().map(|p| p.to_ascii_uppercase()).collect();
    let has = |toks: &[&str]| up.iter().any(|p| toks.iter().any(|t| p.contains(t)));
    match kind {
        DeviceKind::Mouse if has(&["ELAN", "SYNA", "ALPS", "MSFT", "I2C", "TOUCHPAD", "TPD", "SYNP", "CRQ"]) => "laptop touchpad".into(),
        DeviceKind::Mouse if has(&["PNP0F"]) => "ps/2 mouse".into(),
        DeviceKind::Mouse => "system pointing device (touchpad or ps/2)".into(),
        _ if has(&["PNP0303", "I8042"]) => "laptop keyboard (ps/2)".into(),
        _ => "built-in keyboard".into(),
    }
}

fn hide(mut d: DeviceInfo, why: &str) -> DeviceInfo {
    d.usable = false;
    d.hidden_reason = Some(why.to_string());
    d
}

/// how useful a hid collection is as "the" collection of a standalone device. lower is better,
/// None means it is plumbing (config channels, system control...), never an input.
fn collection_rank(h: &HidInfo) -> Option<u8> {
    match (h.usage_page, h.usage) {
        (USAGE_PAGE_GENERIC, 0x04 | 0x05 | 0x08) => Some(0), // joystick, gamepad, multi-axis
        (USAGE_PAGE_SIM, _) => Some(1),                      // wheels, pedals, throttles
        (USAGE_PAGE_DIGITIZER, 0x0E) => None,                // windows device-configuration collection
        (USAGE_PAGE_DIGITIZER, _) => Some(2),                // tablets, pens, touch
        (USAGE_PAGE_GENERIC, 0x07) => Some(3),               // keypad
        (USAGE_PAGE_CONSUMER, _) => Some(4),                 // remotes
        (USAGE_PAGE_GENERIC, 0x01 | 0x02 | 0x06) => Some(5),
        (USAGE_PAGE_GENERIC, 0x80) => None,                  // system control (power button...)
        (p, _) if is_vendor_page(p) => None,
        _ => Some(6),
    }
}

/// merge everything the backends found into the device list shown in the sidebar.
///
/// windows (and the hardware) expose one physical device as many os interfaces: a mouse can
/// be a mouse, a keyboard (macros), and two or three vendor config channels. the rule here is
/// one entry per physical device (usb vid/pid), typed by what it is, with the plumbing folded
/// in and hidden. nothing is deleted: hidden entries stay in the list, flagged `usable: false`.
pub fn build_device_list(inp: &ScanInputs) -> Vec<DeviceInfo> {
    let mut out: Vec<DeviceInfo> = Vec::new();
    let mut hidden: Vec<DeviceInfo> = Vec::new();

    // 1. keyboards + mice from the platform raw layer, grouped by physical device.
    let mut groups: Vec<(Option<u16>, Option<u16>, Vec<&RawDevice>)> = Vec::new();
    for d in &inp.raw {
        // remote desktop, hyper-v and friends: windows-internal, not something you can hold
        if crate::platform::is_virtual_path(&d.path) {
            let (prefix, kind, viewer) = match d.kind {
                DeviceKind::Keyboard => ("kbd", DeviceKind::Keyboard, Viewer::Keyboard),
                _ => ("mouse", DeviceKind::Mouse, Viewer::Mouse),
            };
            let mut v = DeviceInfo::base(format!("{prefix}-virtual-{}", d.src), "virtual input device".into(), kind, viewer, &inp.raw_backend);
            v.srcs = vec![d.src];
            hidden.push(hide(v, "a virtual input device windows creates (remote desktop, hyper-v, converted hid)"));
            continue;
        }
        // a device with no usb ids (ps/2, i2c touchpad, laptop keyboard) is only grouped with its own kind
        let key_ids = (d.vid, d.pid);
        let found = groups.iter_mut().find(|(v, p, m)| {
            (*v, *p) == key_ids && (key_ids.0.is_some() || m.iter().all(|x| x.kind == d.kind))
        });
        match found {
            Some(g) => g.2.push(d),
            None => groups.push((d.vid, d.pid, vec![d])),
        }
    }

    for (vid, pid, members) in groups {
        let has_k = members.iter().any(|m| m.kind == DeviceKind::Keyboard);
        let has_m = members.iter().any(|m| m.kind == DeviceKind::Mouse);
        let product = product_for(&inp.hid, vid, pid);
        let lname = product.as_deref().unwrap_or("").to_lowercase();
        let receiver = lname.contains("receiver") || lname.contains("dongle");
        // a laptop's built-in "keyboard / trackpad" shares one id but is two devices
        let with_pad = lname.contains("trackpad") || lname.contains("touchpad");

        // one entry per kind when the ids are shared by genuinely different things (a wireless
        // receiver with a keyboard and a mouse paired); otherwise the keyboard interface of a mouse
        // (macro keys) is a side channel of that mouse, not a keyboard.
        let split = vid.is_none() || receiver || with_pad;
        let kinds: Vec<DeviceKind> = if has_k && has_m && !split {
            let keyboardish = lname.contains("keyboard") || lname.contains("keypad");
            vec![if keyboardish { DeviceKind::Keyboard } else { DeviceKind::Mouse }]
        } else {
            [DeviceKind::Keyboard, DeviceKind::Mouse].into_iter().filter(|k| members.iter().any(|m| m.kind == *k)).collect()
        };

        // a keyboard-class interface that is really a hotkey/backlight endpoint, not a keyboard
        let kb_caps: Vec<u32> = inp.hid.iter().filter(|h| Some(h.vid) == vid && Some(h.pid) == pid && h.usage_page == 1 && h.usage == 6).filter_map(|h| h.key_capacity).collect();
        let laptop_kbd = product.as_deref().is_some_and(ite_laptop_keyboard);
        let not_a_keyboard = !laptop_kbd && !kb_caps.is_empty() && kb_caps.iter().all(|c| *c < 40);
        let plumbing_named = !laptop_kbd && product.as_deref().is_some_and(is_plumbing_name);

        for kind in kinds {
            let (prefix, viewer) = match kind {
                DeviceKind::Keyboard => ("kbd", Viewer::Keyboard),
                _ => ("mouse", Viewer::Mouse),
            };
            // every interface of a merged device feeds it; the session logic ignores events it does not use
            let mine: Vec<&&RawDevice> = if split { members.iter().filter(|m| m.kind == kind).collect() } else { members.iter().collect() };
            let paths: Vec<&str> = members.iter().filter(|m| m.kind == kind).map(|m| m.path.as_str()).collect();
            let name = friendly_raw_name(kind, vid, pid, product.clone(), maker_for(&inp.hid, vid, pid), &paths);
            let mut d = DeviceInfo::base(format!("{prefix}-{}", fmt_id(vid, pid)), name, kind, viewer, &inp.raw_backend);
            d.vid = vid;
            d.pid = pid;
            d.interfaces = mine.len() as u32;
            d.srcs = mine.iter().map(|m| m.src).collect();
            if !split && has_k && has_m {
                let side = if kind == DeviceKind::Mouse { "keyboard" } else { "mouse" };
                d.note = Some(format!("also exposes a {side} interface (macro keys or a pointer); it is folded in here"));
            }
            if kind == DeviceKind::Keyboard {
                if let (Some(v), Some(p)) = (vid, pid) {
                    if let Some((profile, path)) = analog::detect(&inp.hid, v, p) {
                        d.analog_profile = Some(profile.to_string());
                        d.analog_path = Some(path);
                    }
                }
            }
            // a wireless receiver always carries a keyboard interface (macro keys) next to the mouse,
            // whether or not a keyboard is paired: hidden unless it sends keys is the honest default
            if kind == DeviceKind::Keyboard && receiver && has_m {
                d = hide(d, "the keyboard interface of a wireless receiver. it appears here whether or not a keyboard is paired; show hidden interfaces if you have a paired keyboard");
            } else if kind == DeviceKind::Keyboard && (not_a_keyboard || plumbing_named) {
                d = hide(d, "a hotkey, backlight or firmware endpoint, not a full keyboard");
            }
            out.push(d);
        }
    }

    // 2. gamepads. gilrs first (standard mapping); hidapi-only pads fall back to the generic hid view.
    for pad in &inp.pads {
        let mut d = DeviceInfo::base(format!("pad-{}-{}", pad.uuid, pad.id), pad.name.clone(), DeviceKind::Gamepad, Viewer::Gamepad, "gilrs");
        d.vid = pad.vid;
        d.pid = pad.pid;
        d.pad_id = Some(pad.id);
        if let (Some(v), Some(p)) = (pad.vid, pad.pid) {
            // keep the matching hid path so the real report rate can be read
            if let Some(h) = inp.hid.iter().find(|h| h.vid == v && h.pid == p && h.usage_page == USAGE_PAGE_GENERIC && matches!(h.usage, 0x04 | 0x05)) {
                d.hid_paths.push(h.path.clone());
            }
        }
        out.push(d);
    }
    let any_xinput_pad = inp.pads.iter().any(|p| p.vid.is_none());

    // 3. everything else hidapi sees, one entry per physical device
    let mut phys: Vec<((u16, u16), Vec<&HidInfo>)> = Vec::new();
    for h in &inp.hid {
        match phys.iter_mut().find(|(k, _)| *k == (h.vid, h.pid)) {
            Some(g) => g.1.push(h),
            None => phys.push(((h.vid, h.pid), vec![h])),
        }
    }
    for ((vid, pid), cols) in phys {
        // already shown as a keyboard, mouse or pad: its other interfaces are plumbing of that device
        if let Some(owner) = out.iter_mut().find(|d| d.vid == Some(vid) && d.pid == Some(pid)) {
            owner.hidden_interfaces += cols.len() as u32;
            continue;
        }
        // xinput pads show up as hid devices too, but are read through their gamepad entry
        if cols.iter().all(|c| c.path.to_ascii_uppercase().contains("&IG_")) {
            if any_xinput_pad {
                continue;
            }
        }
        let mut ranked: Vec<(u8, &HidInfo)> = cols.iter().filter_map(|c| collection_rank(c).map(|r| (r, *c))).collect();
        ranked.sort_by_key(|(r, c)| (*r, c.interface));
        let product = cols.iter().find_map(|c| c.product.clone().filter(|s| !s.trim().is_empty()));
        let lname = product.as_deref().unwrap_or("").to_lowercase();
        let (primary, usable, why) = match ranked.first() {
            _ if vid == 0 => (cols[0], false, "no usb vendor id: a software or firmware placeholder"),
            _ if lname.contains("input configuration") => (cols[0], false, "a windows configuration device"),
            Some((_, c)) => (*c, true, ""),
            None => (cols[0], false, "only vendor-defined or system-control interfaces: a configuration channel, not an input"),
        };
        let pad_like = ranked.first().is_some_and(|(r, _)| *r == 0);
        // playstation pads that gilrs cannot see are decoded straight from their hid reports
        let hid_pad = usable && pad_like && crate::hid::pads::kind_for(vid, pid).is_some();
        let base_name = product.unwrap_or_else(|| format!("hid device {vid:04x}:{pid:04x}"));
        let mut d = DeviceInfo::base(
            format!("hid-{vid:04x}{pid:04x}"),
            base_name,
            if pad_like { DeviceKind::Gamepad } else { DeviceKind::Hid },
            if hid_pad { Viewer::Gamepad } else { Viewer::Hid },
            "hidapi",
        );
        d.vid = Some(vid);
        d.pid = Some(pid);
        d.interfaces = cols.len() as u32;
        d.bluetooth = cols.iter().any(|c| c.bluetooth);
        // the primary collection first: that is the one the session reads
        d.hid_paths.push(primary.path.clone());
        d.hid_paths.extend(cols.iter().filter(|c| c.path != primary.path).map(|c| c.path.clone()));
        d.note = Some(format!("usage {:04x}:{:04x}", primary.usage_page, primary.usage));
        out.push(if usable { d } else { hide(d, why) });
    }

    // the laptop's own keyboard: its ite controller exposes several endpoints. one entry, and the legacy
    // ps/2 keyboard next to it (a placeholder that never sends anything) is hidden.
    let ite: Vec<usize> = out.iter().enumerate().filter(|(_, d)| d.kind == DeviceKind::Keyboard && ite_laptop_keyboard(&d.name)).map(|(i, _)| i).collect();
    if !ite.is_empty() {
        let mut merged = out[ite[0]].clone();
        merged.id = "kbd-laptop".into();
        merged.name = "laptop keyboard".into();
        merged.usable = true;
        merged.hidden_reason = None;
        merged.srcs = ite.iter().flat_map(|&i| out[i].srcs.clone()).collect();
        merged.interfaces = ite.iter().map(|&i| out[i].interfaces).sum();
        merged.note = Some(format!("reported through its ite keyboard controller ({})", ite.iter().map(|&i| out[i].name.clone()).collect::<Vec<_>>().join(", ")));
        for &i in ite.iter().rev() {
            out.remove(i);
        }
        for d in out.iter_mut() {
            if d.id == "kbd-sys" && d.usable {
                *d = hide(d.clone(), "the legacy ps/2 keyboard: a placeholder. this laptop's keyboard is the ITE controller above");
            }
        }
        out.push(merged);
    }

    // a laptop touchpad is two things to windows: a pointing device (no usb ids, named by its acpi id)
    // and a hid-over-i2c device with the real ids. same physical thing: fold the hid side in.
    if let Some(ti) = out.iter().position(|d| d.kind == DeviceKind::Mouse && d.name == "laptop touchpad") {
        let mut folded = 0;
        for d in out.iter_mut() {
            let digitizer = d.note.as_deref().is_some_and(|n| n.starts_with("usage 000d:"));
            let i2c = d.name.to_lowercase().contains("i2c");
            if d.kind == DeviceKind::Hid && d.backend == "hidapi" && d.usable && (i2c || digitizer) {
                *d = hide(d.clone(), "the hid interface of the laptop touchpad");
                folded += 1;
            }
        }
        out[ti].hidden_interfaces += folded;
    }

    // 4. midi
    for name in &inp.midi {
        out.push(DeviceInfo::base(format!("midi-{name}"), name.clone(), DeviceKind::Midi, Viewer::Midi, "midir"));
    }

    out.extend(hidden);

    // duplicate ids (two identical devices) get a numeric suffix
    let mut seen: std::collections::HashMap<String, u32> = Default::default();
    for d in &mut out {
        let n = seen.entry(d.id.clone()).or_insert(0);
        if *n > 0 {
            d.id = format!("{}~{}", d.id, n);
        }
        *n += 1;
    }
    out
}

/// device list that remembers unplugged devices (flagged, not dropped) so the
/// sidebar can show "unplugged" instead of silently losing the entry.
#[derive(Default)]
pub struct Registry {
    pub devices: Vec<DeviceInfo>,
}

impl Registry {
    /// returns true when the visible list changed
    pub fn merge(&mut self, fresh: Vec<DeviceInfo>) -> bool {
        let before = serde_json::to_string(&self.devices).unwrap_or_default();
        let fresh_ids: Vec<String> = fresh.iter().map(|d| d.id.clone()).collect();
        let mut next = fresh;
        for old in self.devices.drain(..) {
            if !fresh_ids.contains(&old.id) {
                let mut gone = old;
                gone.connected = false;
                next.push(gone);
            }
        }
        self.devices = next;
        let after = serde_json::to_string(&self.devices).unwrap_or_default();
        before != after
    }

    pub fn get(&self, id: &str) -> Option<&DeviceInfo> {
        self.devices.iter().find(|d| d.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(src: u64, kind: DeviceKind, vid: Option<u16>, pid: Option<u16>) -> RawDevice {
        RawDevice { src, kind, path: format!("path-{src}"), vid, pid }
    }

    fn hid(vid: u16, pid: u16, page: u16, usage: u16, iface: i32, name: &str) -> HidInfo {
        HidInfo { path: format!("hid-{vid:x}-{pid:x}-{page:x}-{usage:x}-{iface}"), vid, pid, product: Some(name.to_string()), usage_page: page, usage, interface: iface, ..Default::default() }
    }

    fn inputs() -> ScanInputs {
        ScanInputs { raw_backend: "rawinput".into(), ..Default::default() }
    }

    fn visible(list: &[DeviceInfo]) -> Vec<&DeviceInfo> {
        list.iter().filter(|d| d.usable).collect()
    }

    #[test]
    fn a_wireless_receiver_is_named_for_what_it_carries_not_usb_receiver() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Mouse, Some(0x046d), Some(0xc539))];
        let mut h = hid(0x046d, 0xc539, 1, 2, 0, "USB Receiver");
        h.manufacturer = Some("Logitech".into());
        i.hid = vec![h];
        let list = build_device_list(&i);
        assert_eq!(list[0].name, "Logitech wireless mouse (usb receiver)");
    }

    #[test]
    fn a_real_product_name_is_kept() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Mouse, Some(0x046d), Some(0xc08b))];
        i.hid = vec![hid(0x046d, 0xc08b, 1, 2, 0, "G502 HERO Gaming Mouse")];
        assert_eq!(build_device_list(&i)[0].name, "G502 HERO Gaming Mouse");
    }

    #[test]
    fn laptop_pointing_devices_are_named_by_their_path() {
        let mk = |path: &str, kind| RawDevice { src: 1, kind, path: path.into(), vid: None, pid: None };
        let name = |d: RawDevice| {
            let mut i = inputs();
            i.raw = vec![d];
            build_device_list(&i)[0].name.clone()
        };
        assert_eq!(name(mk(r"\\?\HID#ELAN0406&Col01#5&1#{a}", DeviceKind::Mouse)), "laptop touchpad");
        assert_eq!(name(mk(r"\\?\ACPI#PNP0F13#4&1#{a}", DeviceKind::Mouse)), "ps/2 mouse");
        assert_eq!(name(mk(r"\\?\ACPI#PNP0303#4&1#{a}", DeviceKind::Keyboard)), "laptop keyboard (ps/2)");
    }

    #[test]
    fn keyboard_interfaces_fold_into_one_device() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Keyboard, Some(0x046d), Some(0xc33c)), raw(2, DeviceKind::Keyboard, Some(0x046d), Some(0xc33c)), raw(3, DeviceKind::Mouse, Some(0x046d), Some(0xc547))];
        i.hid = vec![hid(0x046d, 0xc33c, 1, 6, 0, "G512 keyboard"), hid(0x046d, 0xc33c, 0x0c, 1, 1, "G512 keyboard")];
        let list = build_device_list(&i);
        let kbds: Vec<_> = list.iter().filter(|d| d.kind == DeviceKind::Keyboard).collect();
        assert_eq!(kbds.len(), 1);
        assert_eq!(kbds[0].interfaces, 2);
        assert_eq!(kbds[0].name, "G512 keyboard");
        assert_eq!(kbds[0].srcs, vec![1, 2]);
        assert!(!list.iter().any(|d| d.kind == DeviceKind::Hid), "the consumer-control interface is plumbing");
        assert_eq!(kbds[0].hidden_interfaces, 2);
    }

    #[test]
    fn a_gaming_mouse_with_keyboard_and_vendor_interfaces_is_one_device() {
        // the case that flooded the list: mouse + macro keyboard + three logitech vendor channels
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Mouse, Some(0x046d), Some(0xc08b)), raw(2, DeviceKind::Keyboard, Some(0x046d), Some(0xc08b)), raw(3, DeviceKind::Mouse, Some(0x046d), Some(0xc08b))];
        i.hid = vec![
            hid(0x046d, 0xc08b, 1, 2, 0, "G502 HERO Gaming Mouse"),
            hid(0x046d, 0xc08b, 1, 6, 1, "G502 HERO Gaming Mouse"),
            hid(0x046d, 0xc08b, 0xff00, 1, 1, "G502 HERO Gaming Mouse"),
            hid(0x046d, 0xc08b, 0xff00, 2, 2, "G502 HERO Gaming Mouse"),
            hid(0x046d, 0xc08b, 0xff43, 0x202, 2, "G502 HERO Gaming Mouse"),
        ];
        let list = build_device_list(&i);
        assert_eq!(visible(&list).len(), 1, "{:#?}", list.iter().map(|d| (&d.id, d.usable)).collect::<Vec<_>>());
        let d = &list[0];
        assert_eq!(d.kind, DeviceKind::Mouse);
        assert_eq!(d.name, "G502 HERO Gaming Mouse");
        assert_eq!(d.srcs.len(), 3, "events from every interface still reach it");
        assert!(d.note.as_deref().unwrap().contains("keyboard interface"));
    }

    #[test]
    fn a_real_keyboard_with_a_pointer_interface_stays_a_keyboard() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Keyboard, Some(0x17ef), Some(0x6047)), raw(2, DeviceKind::Mouse, Some(0x17ef), Some(0x6047))];
        i.hid = vec![hid(0x17ef, 0x6047, 1, 6, 0, "ThinkPad Compact USB Keyboard with TrackPoint")];
        let v = build_device_list(&i);
        assert_eq!(visible(&v).len(), 1);
        assert_eq!(v[0].kind, DeviceKind::Keyboard);
    }

    #[test]
    fn a_shared_receiver_with_a_keyboard_and_mouse_stays_two_devices() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Keyboard, Some(0x046d), Some(0xc52b)), raw(2, DeviceKind::Mouse, Some(0x046d), Some(0xc52b))];
        i.hid = vec![hid(0x046d, 0xc52b, 1, 6, 0, "USB Receiver")];
        let list = build_device_list(&i);
        // the receiver's keyboard interface is hidden by default; the mouse stays
        assert_eq!(visible(&list).len(), 1);
        assert_eq!(visible(&list)[0].kind, DeviceKind::Mouse);
        assert_eq!(list.len(), 2, "the keyboard half is still there under hidden");
    }

    #[test]
    fn a_macbook_keyboard_and_trackpad_are_two_devices() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Keyboard, Some(0x05ac), Some(0x0343)), raw(2, DeviceKind::Mouse, Some(0x05ac), Some(0x0343))];
        i.hid = vec![hid(0x05ac, 0x0343, 1, 6, 0, "Apple Internal Keyboard / Trackpad")];
        let list = build_device_list(&i);
        let kinds: Vec<_> = visible(&list).iter().map(|d| (d.kind, d.srcs.clone())).collect();
        assert_eq!(kinds, vec![(DeviceKind::Keyboard, vec![1]), (DeviceKind::Mouse, vec![2])]);
    }

    #[test]
    fn hotkey_and_backlight_endpoints_are_not_keyboards() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Keyboard, Some(0x048d), Some(0xc955)), raw(2, DeviceKind::Keyboard, Some(0x1234), Some(0x1))];
        let mut a = hid(0x048d, 0xc955, 1, 6, 0, "Keyboard Backlight Controller");
        a.key_capacity = Some(12);
        let mut b = hid(0x1234, 0x1, 1, 6, 0, "Real Keyboard");
        b.key_capacity = Some(110);
        i.hid = vec![a, b];
        let list = build_device_list(&i);
        assert_eq!(visible(&list).len(), 1);
        assert_eq!(visible(&list)[0].name, "Real Keyboard");
    }

    #[test]
    fn ite_8xxx_devices_are_the_laptop_keyboard_and_the_ps2_placeholder_is_hidden() {
        let mut i = inputs();
        i.raw = vec![
            raw(1, DeviceKind::Keyboard, Some(0x048d), Some(0xc955)),
            raw(2, DeviceKind::Keyboard, Some(0x048d), Some(0x8910)),
            RawDevice { src: 3, kind: DeviceKind::Keyboard, path: r"\\?\ACPI#PNP0303#4&1#{a}".into(), vid: None, pid: None },
        ];
        let mut a = hid(0x048d, 0xc955, 1, 6, 0, "ITE Device(8295)");
        a.key_capacity = Some(12); // tiny: must not matter, this is the keyboard
        i.hid = vec![a, hid(0x048d, 0x8910, 1, 6, 0, "ITE Device(8910)")];
        let list = build_device_list(&i);
        let vis = visible(&list);
        assert_eq!(vis.len(), 1, "{:?}", list.iter().map(|d| (&d.name, d.usable)).collect::<Vec<_>>());
        assert_eq!(vis[0].name, "laptop keyboard");
        assert_eq!(vis[0].srcs, vec![1, 2], "events from both ite endpoints reach it");
        assert!(list.iter().any(|d| d.id == "kbd-sys" && !d.usable));
    }

    #[test]
    fn the_touchpads_hid_side_is_folded_into_the_laptop_touchpad() {
        let mut i = inputs();
        i.raw = vec![RawDevice { src: 1, kind: DeviceKind::Mouse, path: r"\\?\HID#ELAN06FA&Col01#5&1#{a}".into(), vid: None, pid: None }];
        i.hid = vec![hid(0x04f3, 0x3186, 0x0d, 0x05, 0, "HIDI2C Device")];
        let list = build_device_list(&i);
        let vis = visible(&list);
        assert_eq!(vis.len(), 1);
        assert_eq!(vis[0].name, "laptop touchpad");
        assert_eq!(vis[0].hidden_interfaces, 1);
        assert!(list.iter().any(|d| d.name == "HIDI2C Device" && !d.usable));
    }

    #[test]
    fn a_standalone_i2c_device_without_a_touchpad_is_left_alone() {
        let mut i = inputs();
        i.hid = vec![hid(0x04f3, 0x3186, 0x0d, 0x05, 0, "HIDI2C Device")];
        assert_eq!(visible(&build_device_list(&i)).len(), 1);
    }

    #[test]
    fn ite_devices_outside_8000_to_8999_are_not_assumed_to_be_the_keyboard() {
        assert!(ite_laptop_keyboard("ITE Device(8000)"));
        assert!(ite_laptop_keyboard("ITE Device(8999)"));
        assert!(!ite_laptop_keyboard("ITE Device(7999)"));
        assert!(!ite_laptop_keyboard("ITE Device(9000)"));
        assert!(!ite_laptop_keyboard("ITE Device(abc)"));
        assert!(!ite_laptop_keyboard("Some Keyboard"));
    }

    #[test]
    fn a_keyboard_with_an_unreadable_descriptor_stays_visible() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Keyboard, Some(0x1234), Some(0x2))];
        i.hid = vec![hid(0x1234, 0x2, 1, 6, 0, "Some Keyboard")]; // key_capacity None
        assert_eq!(visible(&build_device_list(&i)).len(), 1);
    }

    #[test]
    fn a_generic_name_borrows_the_vendor_from_the_usb_id() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Keyboard, Some(0x046d), Some(0xc232))];
        i.hid = vec![hid(0x046d, 0xc232, 1, 6, 0, "USB Input Device")];
        assert_eq!(build_device_list(&i)[0].name, "Logitech usb keyboard");
    }

    #[test]
    fn system_devices_without_ids_split_by_kind() {
        let mut i = inputs();
        i.raw = vec![raw(1, DeviceKind::Keyboard, None, None), raw(2, DeviceKind::Keyboard, None, None), raw(3, DeviceKind::Mouse, None, None)];
        let list = build_device_list(&i);
        assert_eq!(list.iter().find(|d| d.id == "kbd-sys").unwrap().interfaces, 2);
        assert!(list.iter().any(|d| d.id == "mouse-sys"));
    }

    #[test]
    fn windows_internal_devices_are_hidden_not_listed() {
        let mut i = inputs();
        i.raw = vec![
            RawDevice { src: 1, kind: DeviceKind::Keyboard, path: r"\\?\Root#RDP_KBD#0000#{a}".into(), vid: None, pid: None },
            RawDevice { src: 2, kind: DeviceKind::Mouse, path: r"\\?\HID#ConvertedDevice&Col01#1&0#{a}".into(), vid: None, pid: None },
            raw(3, DeviceKind::Keyboard, Some(1), Some(2)),
        ];
        let list = build_device_list(&i);
        assert_eq!(visible(&list).len(), 1);
        assert_eq!(list.iter().filter(|d| !d.usable).count(), 2);
        assert!(list.iter().filter(|d| !d.usable).all(|d| d.hidden_reason.is_some()));
    }

    #[test]
    fn gamepad_seen_by_gilrs_and_hid_is_listed_once() {
        let mut i = inputs();
        i.pads = vec![PadInfo { id: 0, name: "dualsense".into(), vid: Some(0x054c), pid: Some(0x0ce6), uuid: "abc".into(), force_feedback: true }];
        i.hid = vec![hid(0x054c, 0x0ce6, 1, 5, 0, "dualsense"), hid(0x054c, 0x0ce6, 0xff00, 1, 1, "dualsense")];
        let list = build_device_list(&i);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].viewer, Viewer::Gamepad);
        assert_eq!(list[0].hid_paths.len(), 1);
    }

    #[test]
    fn xinput_hid_twins_are_dropped_when_a_pad_is_read_through_gilrs() {
        let mut i = inputs();
        i.pads = vec![PadInfo { id: 0, name: "Xbox Controller".into(), vid: None, pid: None, uuid: "x".into(), force_feedback: true }];
        let mut twin = hid(0x045e, 0x028e, 1, 5, 0, "Controller (Xbox 360 For Windows)");
        twin.path = r"\\?\hid#vid_045e&pid_028e&ig_00#7&1#{a}".into();
        i.hid = vec![twin];
        assert_eq!(build_device_list(&i).len(), 1);
    }

    #[test]
    fn a_dualsense_only_hid_can_see_gets_the_controller_view() {
        let mut i = inputs();
        i.hid = vec![hid(0x054c, 0x0ce6, 1, 5, 0, "Wireless Controller"), hid(0x054c, 0x0ce6, 0xff00, 1, 1, "Wireless Controller")];
        let list = build_device_list(&i);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, DeviceKind::Gamepad);
        assert_eq!(list[0].viewer, Viewer::Gamepad);
        assert_eq!(list[0].backend, "hidapi");
        assert!(list[0].pad_id.is_none());
    }

    #[test]
    fn a_flight_stick_without_a_decoder_still_gets_the_hid_view() {
        let mut i = inputs();
        i.hid = vec![hid(0x1234, 0x5678, 1, 4, 0, "arcade stick")];
        assert_eq!(build_device_list(&i)[0].viewer, Viewer::Hid);
    }

    #[test]
    fn a_standalone_flight_stick_is_one_entry_with_its_interfaces_folded() {
        let mut i = inputs();
        i.hid = vec![hid(0x1234, 0x5678, 1, 4, 0, "arcade stick"), hid(0x1234, 0x5678, 0xff00, 1, 1, "arcade stick"), hid(0x1234, 0x5678, 0x0c, 1, 2, "arcade stick")];
        let list = build_device_list(&i);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, DeviceKind::Gamepad);
        assert_eq!(list[0].viewer, Viewer::Hid);
        assert_eq!(list[0].backend, "hidapi");
        assert_eq!(list[0].interfaces, 3);
        assert!(list[0].usable);
        assert!(list[0].hid_paths[0].contains("-1-4-"), "the joystick collection is the primary one");
    }

    #[test]
    fn standalone_remotes_and_tablets_are_kept() {
        let mut i = inputs();
        i.hid = vec![hid(1, 1, 0x0c, 1, 0, "media remote"), hid(2, 2, 0x0d, 0x02, 0, "pen tablet")];
        assert_eq!(visible(&build_device_list(&i)).len(), 2);
    }

    #[test]
    fn config_channels_and_system_plumbing_are_hidden() {
        let mut i = inputs();
        i.hid = vec![
            hid(0x1532, 0x00aa, 0xff00, 1, 0, "vendor gadget"),
            hid(0x045e, 0x0001, 0x0d, 0x0e, 0, "Microsoft Input Configuration Device"),
            hid(0x0000, 0x0001, 1, 4, 0, "software placeholder"),
            hid(0x0501, 0x0001, 1, 0x80, 0, "power button"),
        ];
        let list = build_device_list(&i);
        assert_eq!(list.len(), 4);
        assert!(visible(&list).is_empty(), "{:?}", list.iter().map(|d| (&d.name, &d.hidden_reason)).collect::<Vec<_>>());
    }

    #[test]
    fn two_different_devices_get_distinct_ids() {
        let mut i = inputs();
        i.hid = vec![hid(1, 2, 1, 4, 0, "a"), hid(3, 4, 1, 4, 0, "b")];
        let l = build_device_list(&i);
        assert_ne!(l[0].id, l[1].id);
    }

    #[test]
    fn registry_flags_unplugged_devices() {
        let mk = |id: &str| DeviceInfo::base(id.into(), id.into(), DeviceKind::Hid, Viewer::Hid, "hidapi");
        let mut r = Registry::default();
        assert!(r.merge(vec![mk("a"), mk("b")]));
        assert!(!r.merge(vec![mk("a"), mk("b")]));
        assert!(r.merge(vec![mk("a")]));
        assert!(r.get("a").unwrap().connected);
        assert!(!r.get("b").unwrap().connected);
        assert!(r.merge(vec![mk("a"), mk("b")]));
        assert!(r.get("b").unwrap().connected);
    }
}
