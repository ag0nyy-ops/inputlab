// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! hidapi side: enumeration, report reading, descriptor parsing, analog profiles.
//! one shared HidApi instance: the crate only allows one at a time.

pub mod analog;
pub mod descriptor;
pub mod pads;
pub mod reader;

use std::sync::{Mutex, OnceLock};

use hidapi::HidApi;

#[derive(Clone, Debug)]
pub struct HidInfo {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub product: Option<String>,
    pub manufacturer: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub interface: i32,
}

static API: OnceLock<Result<Mutex<HidApi>, String>> = OnceLock::new();

/// the one shared hidapi context. none when hidapi failed to initialise.
pub fn api() -> Option<&'static Mutex<HidApi>> {
    API.get_or_init(|| HidApi::new().map(Mutex::new).map_err(|e| e.to_string()))
        .as_ref()
        .ok()
}

pub fn init_error() -> Option<String> {
    API.get().and_then(|r| r.as_ref().err().cloned())
}

/// enumerate every hid interface hidapi can see (refreshes the device list).
pub fn enumerate() -> Vec<HidInfo> {
    let Some(api) = api() else { return vec![] };
    let Ok(mut api) = api.lock() else { return vec![] };
    if api.refresh_devices().is_err() {
        return vec![];
    }
    api.device_list()
        .map(|d| HidInfo {
            path: d.path().to_string_lossy().into_owned(),
            vid: d.vendor_id(),
            pid: d.product_id(),
            product: d.product_string().map(str::to_string),
            manufacturer: d.manufacturer_string().map(str::to_string),
            usage_page: d.usage_page(),
            usage: d.usage(),
            interface: d.interface_number(),
        })
        .collect()
}
