//! gilrs lives on its own thread (it is not Send on every platform). it keeps
//! the list of connected pads, streams raw button/axis events to the engine,
//! plays rumble effects and reads the battery for the pad under test.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gilrs::ff::{BaseEffect, BaseEffectType, EffectBuilder, Replay, Ticks};
use gilrs::{Event, EventType, Gilrs, GilrsBuilder, PowerInfo};

use super::PadInfo;
use crate::timing::now_ns;

#[derive(Clone, Default)]
pub struct PadList(pub Arc<Mutex<Vec<PadInfo>>>);

impl PadList {
    pub fn snapshot(&self) -> Vec<PadInfo> {
        self.0.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

/// one raw event from a pad. `name` is gilrs's logical name (South, LeftStickX...),
/// `code` the device's own code string, so odd pads still show something real.
#[derive(Clone, Debug)]
pub struct PadEvent {
    pub pad: usize,
    pub name: String,
    pub code: String,
    pub value: f32,
    pub axis: bool,
    pub ts: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PadStatus {
    pub pad: usize,
    pub force_feedback: bool,
    /// "wired" | "discharging" | "charging" | "charged" | "unknown"
    pub power: String,
    pub battery: Option<u8>,
}

#[derive(Clone, Debug)]
pub enum PadMsg {
    Event(PadEvent),
    Status(PadStatus),
}

pub enum PadCmd {
    Rumble { pad: usize, strong: f32, weak: f32, ms: u32 },
    /// report battery for this pad (None stops)
    Watch(Option<usize>),
}

#[derive(Clone)]
pub struct PadHandle {
    pub list: PadList,
    pub cmd: Sender<PadCmd>,
}

fn uuid_hex(u: [u8; 16]) -> String {
    u.iter().map(|b| format!("{b:02x}")).collect()
}

fn status_of(g: &Gilrs, pad: usize) -> Option<PadStatus> {
    let (_, p) = g.gamepads().find(|(id, _)| usize::from(*id) == pad)?;
    let (power, battery) = match p.power_info() {
        PowerInfo::Unknown => ("unknown", None),
        PowerInfo::Wired => ("wired", None),
        PowerInfo::Discharging(n) => ("discharging", Some(n)),
        PowerInfo::Charging(n) => ("charging", Some(n)),
        PowerInfo::Charged => ("charged", Some(100)),
    };
    Some(PadStatus { pad, force_feedback: p.is_ff_supported(), power: power.into(), battery })
}

/// spawn the gilrs thread. `on_change` fires when a pad connects/disconnects; `sink` gets raw events.
pub fn spawn(on_change: impl Fn() + Send + 'static, sink: impl Fn(PadMsg) + Send + 'static) -> PadHandle {
    let list = PadList::default();
    let shared = list.clone();
    let (cmd_tx, cmd_rx): (Sender<PadCmd>, Receiver<PadCmd>) = mpsc::channel();

    std::thread::Builder::new()
        .name("inputlab-gilrs".into())
        .spawn(move || {
            // raw values on purpose: no deadzone or jitter filter, the tester needs the truth
            let mut gilrs = match GilrsBuilder::new().with_default_filters(false).build() {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("[inputlab] gilrs unavailable: {e}");
                    return;
                }
            };
            let refresh = |g: &Gilrs| {
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
                if let Ok(mut l) = shared.0.lock() {
                    *l = pads;
                }
            };
            refresh(&gilrs);
            on_change();

            let mut watch: Option<usize> = None;
            let mut last_power = Instant::now() - Duration::from_secs(10);
            let mut last_status: Option<PadStatus> = None;
            let mut effect = None;

            loop {
                while let Some(Event { id, event, .. }) = gilrs.next_event_blocking(Some(Duration::from_millis(20))) {
                    let pad = usize::from(id);
                    let ts = now_ns();
                    let (name, code, value, axis) = match event {
                        EventType::ButtonPressed(b, c) => (format!("{b:?}"), c.to_string(), 1.0, false),
                        EventType::ButtonReleased(b, c) => (format!("{b:?}"), c.to_string(), 0.0, false),
                        EventType::ButtonChanged(b, v, c) => (format!("{b:?}"), c.to_string(), v, false),
                        EventType::AxisChanged(a, v, c) => (format!("{a:?}"), c.to_string(), v, true),
                        EventType::Connected | EventType::Disconnected => {
                            refresh(&gilrs);
                            on_change();
                            continue;
                        }
                        _ => continue,
                    };
                    sink(PadMsg::Event(PadEvent { pad, name, code, value, axis, ts }));
                }

                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        PadCmd::Watch(p) => {
                            watch = p;
                            last_status = None;
                            last_power = Instant::now() - Duration::from_secs(10);
                        }
                        PadCmd::Rumble { pad, strong, weak, ms } => {
                            let ticks = Ticks::from_ms(ms.clamp(50, 5000));
                            let play = |kind| BaseEffect { kind, scheduling: Replay { play_for: ticks, ..Default::default() }, ..Default::default() };
                            let id = gilrs.gamepads().find(|(i, _)| usize::from(*i) == pad).map(|(i, _)| i);
                            if let Some(id) = id {
                                let mut b = EffectBuilder::new();
                                b.add_effect(play(BaseEffectType::Strong { magnitude: (strong.clamp(0.0, 1.0) * u16::MAX as f32) as u16 }));
                                b.add_effect(play(BaseEffectType::Weak { magnitude: (weak.clamp(0.0, 1.0) * u16::MAX as f32) as u16 }));
                                b.gamepads(&[id]);
                                match b.finish(&mut gilrs) {
                                    Ok(e) => {
                                        let _ = e.play();
                                        effect = Some(e); // dropping the old effect stops it
                                    }
                                    Err(e) => eprintln!("[inputlab] rumble failed: {e}"),
                                }
                            }
                        }
                    }
                }

                if let Some(p) = watch {
                    if last_power.elapsed() >= Duration::from_secs(2) {
                        last_power = Instant::now();
                        if let Some(s) = status_of(&gilrs, p) {
                            if last_status.as_ref() != Some(&s) {
                                sink(PadMsg::Status(s.clone()));
                                last_status = Some(s);
                            }
                        }
                    }
                }
                let _ = &effect;
            }
        })
        .ok();

    PadHandle { list, cmd: cmd_tx }
}
