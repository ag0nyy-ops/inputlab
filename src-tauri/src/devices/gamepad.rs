//! gilrs lives on its own thread (it is not Send on every platform). this
//! module keeps a shared list of connected pads and pings the hub on change.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gilrs::{EventType, GilrsBuilder};

use super::PadInfo;

#[derive(Clone, Default)]
pub struct PadList(pub Arc<Mutex<Vec<PadInfo>>>);

impl PadList {
    pub fn snapshot(&self) -> Vec<PadInfo> {
        self.0.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

fn uuid_hex(u: [u8; 16]) -> String {
    u.iter().map(|b| format!("{b:02x}")).collect()
}

/// spawn the gilrs thread. `on_change` is called whenever a pad connects or disconnects.
pub fn spawn(on_change: impl Fn() + Send + 'static) -> PadList {
    let list = PadList::default();
    let shared = list.clone();
    std::thread::Builder::new()
        .name("inputlab-gilrs-list".into())
        .spawn(move || {
            // raw values on purpose: no deadzone or jitter filter, the tester needs the truth
            let built = GilrsBuilder::new().with_default_filters(false).build();
            let mut gilrs = match built {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("[inputlab] gilrs unavailable: {e}");
                    return;
                }
            };
            let refresh = |g: &gilrs::Gilrs| {
                let pads: Vec<PadInfo> = g
                    .gamepads()
                    .filter(|(_, p)| p.is_connected())
                    .map(|(id, p)| PadInfo {
                        id: usize::from(id),
                        name: p.name().to_string(),
                        vid: p.vendor_id(),
                        pid: p.product_id(),
                        uuid: uuid_hex(p.uuid()),
                        force_feedback: p.is_ff_supported(),
                    })
                    .collect();
                if let Ok(mut g) = shared.0.lock() {
                    *g = pads;
                }
            };
            refresh(&gilrs);
            on_change();
            loop {
                while let Some(ev) = gilrs.next_event_blocking(Some(Duration::from_millis(250))) {
                    if matches!(ev.event, EventType::Connected | EventType::Disconnected) {
                        refresh(&gilrs);
                        on_change();
                    }
                }
            }
        })
        .ok();
    list
}
