//! tauri commands. thin: they validate, then hand off to the hub / engine.

use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use crate::devices::hub::{DeviceListMsg, Hub};
use crate::platform::{Platform, PlatformCaps};

pub struct AppState {
    pub hub: Arc<Hub>,
    pub platform: Arc<dyn Platform>,
    pub no_capture: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub os: &'static str,
    pub raw_backend: &'static str,
    pub caps: PlatformCaps,
    pub no_capture: bool,
    pub hid_error: Option<String>,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        os: state.platform.os(),
        raw_backend: state.platform.raw_backend(),
        caps: state.platform.capabilities(),
        no_capture: state.no_capture,
        hid_error: crate::hid::init_error(),
    }
}

#[tauri::command]
pub fn subscribe_devices(state: State<'_, AppState>, on_event: Channel<DeviceListMsg>) {
    state.hub.subscribe(on_event);
}

#[tauri::command]
pub fn rescan_devices(state: State<'_, AppState>) {
    state.hub.nudge.ping();
}
