// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! hidapi side: enumeration, report reading, descriptor parsing, analog profiles.
//! one shared HidApi instance: the crate only allows one at a time.

pub mod analog;
pub mod descriptor;
pub mod pads;
pub mod reader;

use std::sync::{Mutex, OnceLock};

use hidapi::HidApi;

#[derive(Clone, Debug, Default)]
pub struct HidInfo {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub product: Option<String>,
    pub manufacturer: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub interface: i32,
    /// keys the device declares on the keyboard page, for keyboard-class interfaces (None = unknown)
    pub key_capacity: Option<u32>,
    pub bluetooth: bool,
}

static API: OnceLock<Result<Mutex<HidApi>, String>> = OnceLock::new();

/// the one shared hidapi context. none when hidapi failed to initialise.
pub fn api() -> Option<&'static Mutex<HidApi>> {
    API.get_or_init(|| {
        HidApi::new()
            .map(|api| {
                // macos hidapi seizes devices by default, which would starve the system, gilrs
                // and inputlab's own iokit reader of the same keyboard or controller
                #[cfg(target_os = "macos")]
                api.set_open_exclusive(false);
                Mutex::new(api)
            })
            .map_err(|e| e.to_string())
    })
        .as_ref()
        .ok()
}

pub fn init_error() -> Option<String> {
    API.get().and_then(|r| r.as_ref().err().cloned())
}

static CAPACITY: OnceLock<Mutex<std::collections::HashMap<String, Option<u32>>>> = OnceLock::new();

/// how many keys a keyboard-class interface declares, from its report descriptor. cached per path:
/// opening a device on every scan would be wasteful, and a descriptor never changes.
fn key_capacity(api: &HidApi, path: &str) -> Option<u32> {
    let cache = CAPACITY.get_or_init(Default::default);
    if let Some(v) = cache.lock().ok()?.get(path) {
        return *v;
    }
    let v = std::ffi::CString::new(path).ok().and_then(|c| api.open_path(&c).ok()).and_then(|d| {
        let mut buf = vec![0u8; 4096];
        let n = d.get_report_descriptor(&mut buf).ok()?;
        Some(descriptor::parse(&buf[..n]).key_capacity)
    });
    if let Ok(mut c) = cache.lock() {
        c.insert(path.to_string(), v);
    }
    v
}

/// enumerate every hid interface hidapi can see (refreshes the device list).
pub fn enumerate() -> Vec<HidInfo> {
    let Some(api) = api() else { return vec![] };
    let Ok(mut api) = api.lock() else { return vec![] };
    if api.refresh_devices().is_err() {
        return vec![];
    }
    let list: Vec<&hidapi::DeviceInfo> = api.device_list().collect();
    list.into_iter()
        .map(|d| HidInfo {
            path: d.path().to_string_lossy().into_owned(),
            vid: d.vendor_id(),
            pid: d.product_id(),
            product: d.product_string().map(str::to_string),
            manufacturer: d.manufacturer_string().map(str::to_string),
            usage_page: d.usage_page(),
            usage: d.usage(),
            interface: d.interface_number(),
            key_capacity: if d.usage_page() == 0x01 && d.usage() == 0x06 { key_capacity(&api, &d.path().to_string_lossy()) } else { None },
            bluetooth: matches!(d.bus_type(), hidapi::BusType::Bluetooth),
        })
        .collect()
}
