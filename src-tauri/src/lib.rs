// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! inputlab: native input device tester. rust owns every device read, every
//! timestamp and all the math; the webview only renders what it is sent.

pub mod capture;
pub mod commands;
pub mod devices;
pub mod engine;
pub mod events;
pub mod hid;
pub mod keys;
pub mod platform;
pub mod timing;

use tauri::{Manager, RunEvent, WindowEvent};

use commands::AppState;
use engine::EngineMsg;

pub fn run() {
    // failsafe: `--no-capture` (or INPUTLAB_NO_CAPTURE=1) disables every hook, clip and grab.
    let no_capture = std::env::args().any(|a| a == "--no-capture")
        || std::env::var("INPUTLAB_NO_CAPTURE").is_ok_and(|v| !v.is_empty() && v != "0");

    // whatever goes wrong, never leave a clip, hook or grab behind.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        platform::emergency_release();
        default_hook(info);
    }));

    let platform = platform::current();
    let nudge = std::sync::Arc::new(devices::hub::Nudge::default());
    let (tx, rx) = engine::channel();
    let pads = {
        let (n, t) = (nudge.clone(), tx.clone());
        devices::gamepad::spawn(move || n.ping(), move |m| {
            let _ = t.send(EngineMsg::Pad(m));
        })
    };
    let hub = devices::hub::Hub::start(platform.clone(), nudge.clone(), pads.clone());
    let engine = engine::spawn(rx, tx, platform.clone(), nudge, no_capture, pads.cmd.clone());

    let app = tauri::Builder::default()
        // the window toolkit registers its own raw input for the whole process, which replaces ours
        .device_event_filter(tauri::DeviceEventFilter::Always)
        .manage(AppState { hub, platform, engine, no_capture })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::subscribe_devices,
            commands::rescan_devices,
            commands::start_session,
            commands::end_session,
            commands::session_heartbeat,
            commands::session_command,
            commands::key_labels,
        ])
        .on_window_event(|window, event| {
            let state = window.state::<AppState>();
            match event {
                // during a session the title-bar x and alt+f4 do nothing
                WindowEvent::CloseRequested { api, .. } if commands::session_active(&state) => api.prevent_close(),
                // windows decides focus from the foreground-window check in the platform layer;
                // webview2 shuffles focus between the window and its child, which would false-trigger this.
                #[cfg(not(windows))]
                WindowEvent::Focused(false) => {
                    let _ = state.engine.tx.send(EngineMsg::FocusLost);
                }
                WindowEvent::Destroyed => state.platform.end_capture(),
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building inputlab");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, code, .. } => {
            // a user-initiated quit is vetoed while a session holds capture
            if code.is_none() && commands::session_active(&app.state::<AppState>()) {
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            let state = app.state::<AppState>();
            state.platform.end_capture();
            let _ = state.engine.tx.send(EngineMsg::Shutdown);
        }
        _ => {}
    });
}
