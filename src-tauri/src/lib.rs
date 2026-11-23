//! inputlab: native input device tester. rust owns every device read, every
//! timestamp and all the math; the webview only renders what it is sent.

pub mod commands;
pub mod devices;
pub mod hid;
pub mod platform;
pub mod timing;

use commands::AppState;

pub fn run() {
    let no_capture = std::env::args().any(|a| a == "--no-capture")
        || std::env::var("INPUTLAB_NO_CAPTURE").is_ok_and(|v| !v.is_empty() && v != "0");

    // whatever goes wrong, never leave a clip or hook behind.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        platform::emergency_release();
        default_hook(info);
    }));

    let platform = platform::current();
    let hub = devices::hub::Hub::start(platform.clone());

    tauri::Builder::default()
        .manage(AppState { hub, platform, no_capture })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::subscribe_devices,
            commands::rescan_devices,
        ])
        .run(tauri::generate_context!())
        .expect("error while running inputlab");
}
