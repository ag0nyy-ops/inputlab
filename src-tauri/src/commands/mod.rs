//! tauri commands. thin: they validate, then hand off to the hub / engine.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use crate::capture::SessionSettings;
use crate::devices::hub::{DeviceListMsg, Hub};
use crate::devices::Viewer;
use crate::engine::{EngineHandle, EngineMsg, SessionInfo, StartReq};
use crate::events::{EndReason, Frame};
use crate::platform::{KeyLabels, Platform, PlatformCaps};

pub struct AppState {
    pub hub: Arc<Hub>,
    pub platform: Arc<dyn Platform>,
    pub engine: EngineHandle,
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

#[cfg(windows)]
fn native_handle(w: &WebviewWindow) -> Option<isize> {
    w.hwnd().ok().map(|h| h.0 as isize)
}
#[cfg(not(windows))]
fn native_handle(_: &WebviewWindow) -> Option<isize> {
    None
}

#[tauri::command]
pub fn start_session(
    state: State<'_, AppState>,
    window: WebviewWindow,
    device_id: String,
    settings: Option<SessionSettings>,
    on_frame: Channel<Frame>,
) -> Result<SessionInfo, String> {
    let device = state.hub.find(&device_id).ok_or("that device is not in the list any more")?;
    if !device.connected {
        return Err("that device is unplugged".into());
    }
    if matches!(device.viewer, Viewer::Keyboard | Viewer::Mouse) && !state.platform.capabilities().raw_input {
        return Err(state.platform.capabilities().notes.first().cloned().unwrap_or_else(|| "raw input is not available on this platform".into()));
    }
    let (tx, rx) = mpsc::channel();
    let req = StartReq { device, settings: settings.unwrap_or_default(), window: native_handle(&window), sink: Box::new(on_frame), reply: tx };
    state.engine.tx.send(EngineMsg::Start(Box::new(req))).map_err(|_| "the engine is not running")?;
    rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "the engine did not answer in time".to_string())?
}

/// the on-screen exit button. the engine refuses it in mouse sessions.
#[tauri::command]
pub fn end_session(state: State<'_, AppState>) {
    let _ = state.engine.tx.send(EngineMsg::End(EndReason::ExitButton));
}

#[tauri::command]
pub fn session_heartbeat(state: State<'_, AppState>) {
    let _ = state.engine.tx.send(EngineMsg::Heartbeat);
}

#[tauri::command]
pub fn key_labels(state: State<'_, AppState>) -> Option<KeyLabels> {
    state.platform.key_labels()
}

pub fn session_active(state: &AppState) -> bool {
    state.engine.active.load(Ordering::SeqCst)
}
