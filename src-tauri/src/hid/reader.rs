// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! raw hid input-report reader. one thread per open device, each report stamped
//! with the shared monotonic clock the moment read() returns. used for the real
//! polling rate of controllers and by the generic hid view.

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::timing::now_ns;

pub struct HidReport {
    pub ts: u64,
    pub data: Vec<u8>,
}

pub struct HidReader {
    stop: Arc<AtomicBool>,
    writes: std::sync::mpsc::Sender<Vec<u8>>,
    /// the device's report descriptor, when the os would give it to us
    pub descriptor: Vec<u8>,
}

impl HidReader {
    /// open `path` and call `on_report` for every input report until dropped.
    /// returns Err when the device cannot be opened (many system devices refuse).
    pub fn open(path: &str, on_report: impl Fn(HidReport) + Send + 'static) -> Result<Self, String> {
        Self::open_with(path, |_| {}, on_report)
    }

    /// like `open`, but `init` runs once on the freshly opened device (a feature report that switches a
    /// pad into its full report mode, for instance).
    pub fn open_with(path: &str, init: impl FnOnce(&hidapi::HidDevice), on_report: impl Fn(HidReport) + Send + 'static) -> Result<Self, String> {
        let api = super::api().ok_or("hidapi is not available")?;
        let cpath = CString::new(path).map_err(|e| e.to_string())?;
        let dev = {
            let api = api.lock().map_err(|_| "hidapi lock poisoned".to_string())?;
            api.open_path(&cpath).map_err(|e| e.to_string())?
        };
        init(&dev);
        let mut desc = vec![0u8; 4096];
        let descriptor = match dev.get_report_descriptor(&mut desc) {
            Ok(n) => desc[..n].to_vec(),
            Err(_) => vec![],
        };
        let stop = Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        let (writes, wrx) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::Builder::new()
            .name("inputlab-hid-read".into())
            .spawn(move || {
                let mut buf = vec![0u8; 512];
                while !s2.load(Ordering::Relaxed) {
                    // output reports (rumble) go out between reads
                    while let Ok(w) = wrx.try_recv() {
                        let _ = dev.write(&w);
                    }
                    match dev.read_timeout(&mut buf, 100) {
                        Ok(0) => {}
                        Ok(n) => on_report(HidReport { ts: now_ns(), data: buf[..n].to_vec() }),
                        Err(_) => break, // unplugged
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { stop, writes, descriptor })
    }
}

impl HidReader {
    /// queue an output report. it goes out within one read interval (about 100 ms).
    pub fn write(&self, data: Vec<u8>) {
        let _ = self.writes.send(data);
    }
}

impl Drop for HidReader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
