//! scanning thread + subscriber fan-out. hotplug is handled two ways: the os
//! layer and gilrs nudge the thread when they see a change, and the thread
//! also rescans on a timer in case os hotplug events are flaky.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;

use super::gamepad::PadList;
use super::{build_device_list, gamepad, midi, DeviceInfo, Registry, ScanInputs};
use crate::hid;
use crate::platform::Platform;

const POLLED_RESCAN: Duration = Duration::from_millis(1500);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceListMsg {
    pub devices: Vec<DeviceInfo>,
}

#[derive(Default)]
pub struct Nudge {
    flag: Mutex<bool>,
    cv: Condvar,
}

impl Nudge {
    pub fn ping(&self) {
        if let Ok(mut f) = self.flag.lock() {
            *f = true;
            self.cv.notify_all();
        }
    }
    fn wait(&self, timeout: Duration) {
        if let Ok(mut f) = self.flag.lock() {
            if !*f {
                if let Ok((g, _)) = self.cv.wait_timeout(f, timeout) {
                    f = g;
                } else {
                    return;
                }
            }
            *f = false;
        }
    }
}

pub struct Hub {
    platform: Arc<dyn Platform>,
    registry: Mutex<Registry>,
    pads: PadList,
    subs: Mutex<Vec<Channel<DeviceListMsg>>>,
    pub nudge: Arc<Nudge>,
}

impl Hub {
    pub fn start(platform: Arc<dyn Platform>) -> Arc<Hub> {
        let nudge = Arc::new(Nudge::default());
        let n2 = nudge.clone();
        let pads = gamepad::spawn(move || n2.ping());
        let hub = Arc::new(Hub {
            platform,
            registry: Mutex::new(Registry::default()),
            pads,
            subs: Mutex::new(vec![]),
            nudge,
        });
        let h = hub.clone();
        std::thread::Builder::new()
            .name("inputlab-scan".into())
            .spawn(move || loop {
                h.rescan();
                h.nudge.wait(POLLED_RESCAN);
            })
            .ok();
        hub
    }

    fn scan_inputs(&self) -> ScanInputs {
        ScanInputs {
            raw: self.platform.enumerate_raw(),
            hid: hid::enumerate(),
            pads: self.pads.snapshot(),
            midi: midi::list_ports(),
            raw_backend: self.platform.raw_backend().to_string(),
        }
    }

    /// scan now; push to subscribers only when the list changed.
    pub fn rescan(&self) {
        let fresh = build_device_list(&self.scan_inputs());
        let changed = self
            .registry
            .lock()
            .map(|mut r| r.merge(fresh))
            .unwrap_or(false);
        if changed {
            self.broadcast();
        }
    }

    pub fn devices(&self) -> Vec<DeviceInfo> {
        self.registry.lock().map(|r| r.devices.clone()).unwrap_or_default()
    }

    pub fn find(&self, id: &str) -> Option<DeviceInfo> {
        self.registry.lock().ok().and_then(|r| r.get(id).cloned())
    }

    pub fn subscribe(&self, ch: Channel<DeviceListMsg>) {
        let _ = ch.send(DeviceListMsg { devices: self.devices() });
        if let Ok(mut s) = self.subs.lock() {
            s.push(ch);
        }
    }

    fn broadcast(&self) {
        let msg = DeviceListMsg { devices: self.devices() };
        if let Ok(mut subs) = self.subs.lock() {
            subs.retain(|ch| ch.send(msg.clone()).is_ok());
        }
    }
}
