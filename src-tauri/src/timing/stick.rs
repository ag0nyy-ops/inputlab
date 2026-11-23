// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! thumbstick and trigger analysis: circularity (the traced shape of the gate),
//! deadzone, max range, and resting drift. values are the raw -1..1 the pad
//! reports (no filtering), with y as the pad gives it.

use serde::Serialize;

const BINS: usize = 72;
const DRIFT_STICK: f32 = 0.05;
const DRIFT_TRIGGER: f32 = 0.03;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StickSnap {
    pub x: f32,
    pub y: f32,
    pub active: bool,
    /// smallest / largest gate radius over the covered angles. 1.0 = a perfect circle.
    pub circularity: Option<f32>,
    /// share of the 72 angle bins the stick has been pushed into
    pub coverage: f32,
    /// the largest radius seen (1.0 = full travel, a square gate reaches ~1.41 in the corners)
    pub max_range: f32,
    /// radius at which the output first leaves zero when moving out of the centre
    pub deadzone: Option<f32>,
    /// max radius per 5 degree bin, for the plot
    pub bins: Vec<f32>,
    pub path: Vec<(f32, f32)>,
}

pub struct StickTest {
    x: f32,
    y: f32,
    active: bool,
    bins: [f32; BINS],
    path: Vec<(f32, f32)>,
    leave: Vec<f32>,
    prev_r: f32,
    max_r: f32,
}

impl Default for StickTest {
    fn default() -> Self {
        Self::new()
    }
}

impl StickTest {
    pub fn new() -> Self {
        Self { x: 0.0, y: 0.0, active: false, bins: [0.0; BINS], path: vec![], leave: vec![], prev_r: 0.0, max_r: 0.0 }
    }
    pub fn start(&mut self) {
        let (x, y) = (self.x, self.y);
        *self = Self::new();
        self.x = x;
        self.y = y;
        self.active = true;
    }
    pub fn stop(&mut self) {
        self.active = false;
    }
    pub fn reset(&mut self) {
        self.start();
        self.active = false;
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn pos(&self) -> (f32, f32) {
        (self.x, self.y)
    }

    pub fn feed(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        if !self.active {
            return;
        }
        let r = x.hypot(y);
        if self.prev_r == 0.0 && r > 0.0 {
            self.leave.push(r);
        }
        self.prev_r = r;
        self.max_r = self.max_r.max(r);
        if r > 0.02 {
            let a = y.atan2(x).to_degrees().rem_euclid(360.0);
            let b = ((a / 360.0 * BINS as f32) as usize).min(BINS - 1);
            self.bins[b] = self.bins[b].max(r);
        }
        if self.path.last().is_none_or(|p| (p.0 - x).hypot(p.1 - y) > 0.01) {
            self.path.push((x, y));
            if self.path.len() > 1200 {
                self.path.drain(..400);
            }
        }
    }

    pub fn snapshot(&self) -> StickSnap {
        let covered: Vec<f32> = self.bins.iter().copied().filter(|&b| b > 0.0).collect();
        let coverage = covered.len() as f32 / BINS as f32;
        // an edge bin is only trustworthy once the stick has been pushed hard into it: use bins above half travel
        let circ = (coverage >= 0.9).then(|| {
            let lo = covered.iter().cloned().fold(f32::MAX, f32::min);
            let hi = covered.iter().cloned().fold(0.0, f32::max);
            lo / hi
        });
        let deadzone = (!self.leave.is_empty()).then(|| {
            let mut v = self.leave.clone();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        });
        StickSnap {
            x: self.x,
            y: self.y,
            active: self.active,
            circularity: circ,
            coverage,
            max_range: self.max_r,
            deadzone,
            bins: self.bins.to_vec(),
            path: self.path.iter().step_by((self.path.len() / 500).max(1)).copied().collect(),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RestEntry {
    pub name: String,
    pub mean_x: f32,
    pub mean_y: f32,
    /// distance of the resting position from centre (sticks) or from zero (triggers)
    pub offset: f32,
    /// peak-to-peak noise while resting
    pub noise: f32,
    pub drift: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RestSnap {
    pub running: bool,
    pub secs_left: f32,
    pub entries: Vec<RestEntry>,
}

struct Acc {
    name: String,
    stick: bool,
    n: u32,
    sx: f64,
    sy: f64,
    min: (f32, f32),
    max: (f32, f32),
}

/// hold the pad still for a few seconds; whatever it reports is its resting state
pub struct RestTest {
    until: u64,
    running: bool,
    accs: Vec<Acc>,
}

impl Default for RestTest {
    fn default() -> Self {
        Self::new()
    }
}

impl RestTest {
    pub fn new() -> Self {
        Self { until: 0, running: false, accs: vec![] }
    }
    pub fn start(&mut self, now: u64, secs: f32) {
        self.accs.clear();
        self.running = true;
        self.until = now + (secs.clamp(1.0, 15.0) as f64 * 1e9) as u64;
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn feed(&mut self, name: &str, stick: bool, x: f32, y: f32) {
        if !self.running {
            return;
        }
        let a = match self.accs.iter_mut().position(|a| a.name == name) {
            Some(i) => &mut self.accs[i],
            None => {
                self.accs.push(Acc { name: name.into(), stick, n: 0, sx: 0.0, sy: 0.0, min: (f32::MAX, f32::MAX), max: (f32::MIN, f32::MIN) });
                self.accs.last_mut().unwrap()
            }
        };
        a.n += 1;
        a.sx += x as f64;
        a.sy += y as f64;
        a.min = (a.min.0.min(x), a.min.1.min(y));
        a.max = (a.max.0.max(x), a.max.1.max(y));
    }
    /// returns true when the run just finished
    pub fn tick(&mut self, now: u64) -> bool {
        if self.running && now >= self.until {
            self.running = false;
            return true;
        }
        false
    }
    pub fn snapshot(&self, now: u64) -> RestSnap {
        RestSnap {
            running: self.running,
            secs_left: if self.running { (self.until.saturating_sub(now)) as f32 / 1e9 } else { 0.0 },
            entries: self
                .accs
                .iter()
                .filter(|a| a.n > 0)
                .map(|a| {
                    let (mx, my) = ((a.sx / a.n as f64) as f32, (a.sy / a.n as f64) as f32);
                    let offset = if a.stick { mx.hypot(my) } else { mx.abs() };
                    let noise = if a.stick { (a.max.0 - a.min.0).hypot(a.max.1 - a.min.1) } else { a.max.0 - a.min.0 };
                    RestEntry { name: a.name.clone(), mean_x: mx, mean_y: my, offset, noise, drift: offset > if a.stick { DRIFT_STICK } else { DRIFT_TRIGGER } }
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_gate(t: &mut StickTest, radius: impl Fn(f32) -> f32) {
        for i in 0..720 {
            let a = (i as f32 / 720.0) * std::f32::consts::TAU;
            let r = radius(a);
            t.feed(r * a.cos(), r * a.sin());
        }
    }

    #[test]
    fn round_gate_is_circular_and_reaches_full_range() {
        let mut t = StickTest::new();
        t.start();
        run_gate(&mut t, |_| 1.0);
        let s = t.snapshot();
        assert!(s.circularity.unwrap() > 0.99);
        assert!((s.max_range - 1.0).abs() < 0.01);
        assert!(s.coverage > 0.99);
    }

    #[test]
    fn square_gate_reads_as_less_circular() {
        let mut t = StickTest::new();
        t.start();
        // a square: radius grows to 1.414 in the corners
        run_gate(&mut t, |a| 1.0 / a.cos().abs().max(a.sin().abs()));
        let s = t.snapshot();
        assert!(s.circularity.unwrap() < 0.75 && s.circularity.unwrap() > 0.65, "{:?}", s.circularity);
        assert!(s.max_range > 1.35);
    }

    #[test]
    fn a_stick_never_pushed_around_gives_no_verdict() {
        let mut t = StickTest::new();
        t.start();
        for i in 0..200 {
            t.feed(i as f32 / 200.0, 0.0); // only to the right
        }
        let s = t.snapshot();
        assert!(s.circularity.is_none());
        assert!(s.coverage < 0.1);
    }

    #[test]
    fn deadzone_is_where_the_output_first_leaves_zero() {
        let mut t = StickTest::new();
        t.start();
        for _ in 0..5 {
            for k in 0..40 {
                // reports 0 until the hardware deadzone of 0.12 is passed
                let v = k as f32 / 40.0;
                t.feed(if v < 0.12 { 0.0 } else { v }, 0.0);
            }
            t.feed(0.0, 0.0);
        }
        let dz = t.snapshot().deadzone.unwrap();
        assert!((dz - 0.12).abs() < 0.03, "{dz}");
    }

    #[test]
    fn nothing_is_recorded_while_stopped() {
        let mut t = StickTest::new();
        run_gate(&mut t, |_| 1.0);
        assert_eq!(t.snapshot().coverage, 0.0);
        assert_eq!(t.pos().0.round(), 1.0, "the live position still follows");
    }

    #[test]
    fn rest_test_flags_drift_and_reports_noise() {
        let mut r = RestTest::new();
        r.start(0, 3.0);
        for i in 0..300 {
            let n = if i % 2 == 0 { 0.004 } else { -0.004 };
            r.feed("left", true, 0.09 + n, 0.02); // drifting right
            r.feed("right", true, 0.003 + n, -0.002); // healthy
            r.feed("lt", false, 0.0, 0.0);
            r.feed("rt", false, 0.06, 0.0); // trigger not returning to zero
        }
        assert!(!r.tick(2_000_000_000));
        assert!(r.tick(3_000_000_000));
        let s = r.snapshot(3_000_000_000);
        let get = |n: &str| s.entries.iter().find(|e| e.name == n).unwrap();
        assert!(get("left").drift && (get("left").offset - 0.092).abs() < 0.01);
        assert!(!get("right").drift);
        assert!(!get("lt").drift);
        assert!(get("rt").drift);
        assert!(get("left").noise > 0.0);
        assert!(!s.running);
    }

    #[test]
    fn rest_test_ignores_input_when_not_running() {
        let mut r = RestTest::new();
        r.feed("left", true, 1.0, 1.0);
        assert!(r.snapshot(0).entries.is_empty());
    }
}
