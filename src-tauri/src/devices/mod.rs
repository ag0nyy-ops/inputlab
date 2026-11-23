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
}

impl DeviceInfo {
    fn base(id: String, name: String, kind: DeviceKind, viewer: Viewer, backend: &str) -> Self {
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
            srcs: vec![],
            hid_paths: vec![],
            analog_path: None,
            pad_id: None,
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
const USAGE_PAGE_CONSUMER: u16 = 0x0C;

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

/// merge everything the backends found into the device list shown in the sidebar.
pub fn build_device_list(inp: &ScanInputs) -> Vec<DeviceInfo> {
    let mut out: Vec<DeviceInfo> = Vec::new();

    // 1. keyboards + mice from the platform raw layer, grouped by vid/pid so
    //    the several interfaces one physical device exposes become one entry.
    for kind in [DeviceKind::Keyboard, DeviceKind::Mouse] {
        let mut groups: Vec<(Option<u16>, Option<u16>, Vec<&RawDevice>)> = Vec::new();
        for d in inp.raw.iter().filter(|d| d.kind == kind) {
            match groups
                .iter_mut()
                .find(|(v, p, _)| (*v, *p) == (d.vid, d.pid))
            {
                Some(g) => g.2.push(d),
                None => groups.push((d.vid, d.pid, vec![d])),
            }
        }
        for (vid, pid, members) in groups {
            let (prefix, label, viewer) = match kind {
                DeviceKind::Keyboard => ("kbd", "keyboard", Viewer::Keyboard),
                _ => ("mouse", "mouse", Viewer::Mouse),
            };
            let name = product_for(&inp.hid, vid, pid).unwrap_or_else(|| match (vid, pid) {
                (Some(_), Some(_)) => format!("generic {label}"),
                _ => format!("system {label} (ps/2, virtual or remote)"),
            });
            let mut d = DeviceInfo::base(
                format!("{prefix}-{}", fmt_id(vid, pid)),
                name,
                kind,
                viewer,
                &inp.raw_backend,
            );
            d.vid = vid;
            d.pid = pid;
            d.interfaces = members.len() as u32;
            d.srcs = members.iter().map(|m| m.src).collect();

            if kind == DeviceKind::Keyboard {
                if let (Some(v), Some(p)) = (vid, pid) {
                    if let Some((profile, path)) = analog::detect(&inp.hid, v, p) {
                        d.analog_profile = Some(profile.to_string());
                        d.analog_path = Some(path);
                    }
                }
            }
            out.push(d);
        }
    }

    // 2. gamepads. gilrs first (standard mapping); hidapi-only pads fall back
    //    to the generic hid view.
    for pad in &inp.pads {
        let mut d = DeviceInfo::base(
            format!("pad-{}-{}", pad.uuid, pad.id),
            pad.name.clone(),
            DeviceKind::Gamepad,
            Viewer::Gamepad,
            "gilrs",
        );
        d.vid = pad.vid;
        d.pid = pad.pid;
        d.pad_id = Some(pad.id);
        if let (Some(v), Some(p)) = (pad.vid, pad.pid) {
            // keep the matching hid path so the raw table / real poll rate can read reports
            if let Some(h) = inp.hid.iter().find(|h| {
                h.vid == v
                    && h.pid == p
                    && h.usage_page == USAGE_PAGE_GENERIC
                    && matches!(h.usage, 0x04 | 0x05)
            }) {
                d.hid_paths.push(h.path.clone());
            }
        }
        out.push(d);
    }

    // 3. everything else hidapi can see
    for h in &inp.hid {
        let is_kbd_mouse = h.usage_page == USAGE_PAGE_GENERIC && matches!(h.usage, 0x02 | 0x06 | 0x01 | 0x80);
        let is_consumer = h.usage_page == USAGE_PAGE_CONSUMER;
        let covered_by_raw = inp
            .raw
            .iter()
            .any(|r| r.vid == Some(h.vid) && r.pid == Some(h.pid));
        let covered_by_pad = inp
            .pads
            .iter()
            .any(|p| p.vid == Some(h.vid) && p.pid == Some(h.pid));
        let is_pad_usage = h.usage_page == USAGE_PAGE_GENERIC && matches!(h.usage, 0x04 | 0x05);

        if (is_kbd_mouse || is_consumer) && covered_by_raw {
            continue;
        }
        if is_pad_usage && covered_by_pad {
            continue;
        }
        // vendor interface already used as the analog source of a keyboard
        if out
            .iter()
            .any(|d| d.analog_path.as_deref() == Some(h.path.as_str()))
        {
            continue;
        }
        // raw-capable devices on platforms without a raw layer still show up
        let (kind, viewer) = if is_pad_usage {
            (DeviceKind::Gamepad, Viewer::Hid)
        } else {
            (DeviceKind::Hid, Viewer::Hid)
        };
        let base_name = h
            .product
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("hid device {:04x}:{:04x}", h.vid, h.pid));
        let mut d = DeviceInfo::base(
            format!(
                "hid-{:04x}{:04x}-{:04x}{:04x}-{}",
                h.vid, h.pid, h.usage_page, h.usage, h.interface
            ),
            if is_vendor_page(h.usage_page) {
                format!("{base_name} (vendor)")
            } else {
                base_name
            },
            kind,
            viewer,
            "hidapi",
        );
        d.vid = Some(h.vid);
        d.pid = Some(h.pid);
        d.hid_paths.push(h.path.clone());
        d.note = Some(format!("usage {:04x}:{:04x}", h.usage_page, h.usage));
        out.push(d);
    }

    // 4. midi
    for name in &inp.midi {
        out.push(DeviceInfo::base(
            format!("midi-{name}"),
            name.clone(),
            DeviceKind::Midi,
            Viewer::Midi,
            "midir",
        ));
    }

    // duplicate ids (two identical hid interfaces) get a numeric suffix
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
        RawDevice {
            src,
            kind,
            path: format!("path-{src}"),
            vid,
            pid,
        }
    }

    fn hid(vid: u16, pid: u16, page: u16, usage: u16, iface: i32, name: &str) -> HidInfo {
        HidInfo {
            path: format!("hid-{vid:x}-{pid:x}-{page:x}-{usage:x}-{iface}"),
            vid,
            pid,
            product: Some(name.to_string()),
            manufacturer: None,
            usage_page: page,
            usage,
            interface: iface,
        }
    }

    #[test]
    fn keyboard_interfaces_fold_into_one_device() {
        let inp = ScanInputs {
            raw_backend: "rawinput".into(),
            raw: vec![
                raw(1, DeviceKind::Keyboard, Some(0x046d), Some(0xc33c)),
                raw(2, DeviceKind::Keyboard, Some(0x046d), Some(0xc33c)),
                raw(3, DeviceKind::Mouse, Some(0x046d), Some(0xc547)),
            ],
            hid: vec![
                hid(0x046d, 0xc33c, 1, 6, 0, "G512 keyboard"),
                hid(0x046d, 0xc33c, 0x0c, 1, 1, "G512 keyboard"),
            ],
            ..Default::default()
        };
        let list = build_device_list(&inp);
        let kbds: Vec<_> = list.iter().filter(|d| d.kind == DeviceKind::Keyboard).collect();
        assert_eq!(kbds.len(), 1);
        assert_eq!(kbds[0].interfaces, 2);
        assert_eq!(kbds[0].name, "G512 keyboard");
        assert_eq!(kbds[0].srcs, vec![1, 2]);
        // consumer-control interface of a raw-covered keyboard is not its own device
        assert!(!list.iter().any(|d| d.kind == DeviceKind::Hid));
        assert_eq!(list.iter().filter(|d| d.kind == DeviceKind::Mouse).count(), 1);
    }

    #[test]
    fn system_devices_without_vid_pid_group_together() {
        let inp = ScanInputs {
            raw_backend: "rawinput".into(),
            raw: vec![
                raw(1, DeviceKind::Keyboard, None, None),
                raw(2, DeviceKind::Keyboard, None, None),
            ],
            ..Default::default()
        };
        let list = build_device_list(&inp);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "kbd-sys");
    }

    #[test]
    fn gamepad_seen_by_gilrs_and_hid_is_listed_once() {
        let inp = ScanInputs {
            raw_backend: "rawinput".into(),
            pads: vec![PadInfo {
                id: 0,
                name: "dualsense".into(),
                vid: Some(0x054c),
                pid: Some(0x0ce6),
                uuid: "abc".into(),
                force_feedback: true,
            }],
            hid: vec![hid(0x054c, 0x0ce6, 1, 5, 0, "dualsense")],
            ..Default::default()
        };
        let list = build_device_list(&inp);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].viewer, Viewer::Gamepad);
        assert_eq!(list[0].hid_paths.len(), 1);
    }

    #[test]
    fn hid_only_pad_falls_back_to_hid_viewer() {
        let inp = ScanInputs {
            raw_backend: "rawinput".into(),
            hid: vec![hid(0x1234, 0x5678, 1, 4, 0, "arcade stick")],
            ..Default::default()
        };
        let list = build_device_list(&inp);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, DeviceKind::Gamepad);
        assert_eq!(list[0].viewer, Viewer::Hid);
        assert_eq!(list[0].backend, "hidapi");
    }

    #[test]
    fn duplicate_ids_get_suffixes() {
        let inp = ScanInputs {
            raw_backend: "rawinput".into(),
            hid: vec![
                hid(1, 2, 0xff00, 1, 0, "thing"),
                hid(1, 2, 0xff00, 1, 0, "thing"),
            ],
            ..Default::default()
        };
        let list = build_device_list(&inp);
        assert_eq!(list.len(), 2);
        assert_ne!(list[0].id, list[1].id);
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
        // replugged
        assert!(r.merge(vec![mk("a"), mk("b")]));
        assert!(r.get("b").unwrap().connected);
    }
}
