// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
//! sensor checks from raw movement: slow-circle smoothness (jitter) and angle
//! snapping, plus per-button click timing. these measure; they do not grade.
//! there is no calibrated pass/fail for jitter, so the ui shows numbers to
//! compare between mice and surfaces. only angle snapping gets a flag.

use serde::Serialize;

/// movement is grouped into steps of at least this many counts before its
/// direction is read: at 1-2 counts per report the angle is just quantisation.
const STEP_COUNTS: f64 = 14.0;
const SNAP_TOL_DEG: f64 = 2.5;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SensorSnap {
    pub active: bool,
    pub steps: u32,
    /// radius of the best-fit circle through the path, in counts
    pub radius: Option<f64>,
    /// high-frequency wobble of the path as % of step length. lower is smoother.
    pub jitter_pct: Option<f64>,
    /// 1.0 = directions spread evenly. well above 1 = movement clings to 45-degree angles.
    pub snap_ratio: Option<f64>,
    pub snapping: bool,
    /// 36 buckets of 10 degrees, for the direction plot
    pub angles: Vec<u32>,
    /// the path so far, subsampled (counts), for the trace plot
    pub path: Vec<(f32, f32)>,
}

pub struct SensorTest {
    active: bool,
    acc: (f64, f64),
    steps: Vec<(f64, f64)>,
    pos: (f64, f64),
    path: Vec<(f32, f32)>,
}

impl Default for SensorTest {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorTest {
    pub fn new() -> Self {
        Self { active: false, acc: (0.0, 0.0), steps: vec![], pos: (0.0, 0.0), path: vec![] }
    }
    pub fn start(&mut self) {
        *self = Self::new();
        self.active = true;
    }
    pub fn stop(&mut self) {
        self.active = false;
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn add(&mut self, dx: i32, dy: i32) {
        if !self.active {
            return;
        }
        self.acc.0 += dx as f64;
        self.acc.1 += dy as f64;
        self.pos.0 += dx as f64;
        self.pos.1 += dy as f64;
        if self.acc.0.hypot(self.acc.1) >= STEP_COUNTS {
            self.steps.push(self.acc);
            self.acc = (0.0, 0.0);
            self.path.push((self.pos.0 as f32, self.pos.1 as f32));
            if self.steps.len() > 20_000 {
                self.steps.drain(..10_000);
                self.path.drain(..10_000);
            }
        }
    }

    pub fn snapshot(&self) -> SensorSnap {
        let n = self.steps.len();
        let mut angles = vec![0u32; 36];
        let mut near45 = 0usize;
        for &(x, y) in &self.steps {
            let a = y.atan2(x).to_degrees().rem_euclid(360.0);
            angles[((a / 10.0) as usize).min(35)] += 1;
            let m = a % 45.0;
            if m.min(45.0 - m) <= SNAP_TOL_DEG {
                near45 += 1;
            }
        }
        let expected = 2.0 * SNAP_TOL_DEG / 45.0;
        let snap_ratio = (n >= 40).then(|| (near45 as f64 / n as f64) / expected);

        // jitter: second difference of consecutive step vectors, relative to step length.
        // a smooth circle has almost none; sensor noise and snapping add a lot.
        let jitter = (n >= 40).then(|| {
            let mut sum = 0.0;
            let mut len = 0.0;
            for w in self.steps.windows(3) {
                let dx = w[2].0 - 2.0 * w[1].0 + w[0].0;
                let dy = w[2].1 - 2.0 * w[1].1 + w[0].1;
                sum += dx * dx + dy * dy;
                len += w[1].0.hypot(w[1].1);
            }
            let m = (n - 2) as f64;
            100.0 * (sum / m).sqrt() / (len / m)
        });

        SensorSnap {
            active: self.active,
            steps: n as u32,
            radius: fit_radius(&self.path),
            jitter_pct: jitter,
            snap_ratio,
            snapping: snap_ratio.is_some_and(|r| r >= 2.5),
            angles,
            path: subsample(&self.path, 400),
        }
    }
}

fn subsample(p: &[(f32, f32)], max: usize) -> Vec<(f32, f32)> {
    if p.len() <= max {
        return p.to_vec();
    }
    let step = p.len() as f64 / max as f64;
    (0..max).map(|i| p[(i as f64 * step) as usize]).collect()
}

/// kasa least-squares circle fit
fn fit_radius(p: &[(f32, f32)]) -> Option<f64> {
    if p.len() < 40 {
        return None;
    }
    let n = p.len() as f64;
    let (mx, my) = p.iter().fold((0.0, 0.0), |a, q| (a.0 + q.0 as f64, a.1 + q.1 as f64));
    let (mx, my) = (mx / n, my / n);
    let (mut suu, mut suv, mut svv, mut suuu, mut svvv, mut suvv, mut svuu) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for q in p {
        let u = q.0 as f64 - mx;
        let v = q.1 as f64 - my;
        suu += u * u;
        suv += u * v;
        svv += v * v;
        suuu += u * u * u;
        svvv += v * v * v;
        suvv += u * v * v;
        svuu += v * u * u;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-9 {
        return None;
    }
    let a = 0.5 * (suuu + suvv);
    let b = 0.5 * (svvv + svuu);
    let uc = (a * svv - b * suv) / det;
    let vc = (suu * b - suv * a) / det;
    Some((uc * uc + vc * vc + (suu + svv) / n).sqrt())
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ButtonStat {
    pub n: u8,
    pub clicks: u32,
    pub hold_min: Option<f64>,
    pub hold_avg: Option<f64>,
    pub hold_max: Option<f64>,
    /// release->press gaps under the chatter threshold: a switch that double-fires
    pub chatter: u32,
    /// shortest release->press gap seen
    pub min_gap: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClickOutcome {
    pub hold_ms: Option<f64>,
    pub gap_ms: Option<f64>,
    pub chatter: bool,
}

#[derive(Default)]
struct BtnRec {
    down_at: Option<u64>,
    up_at: Option<u64>,
    stat: ButtonStat,
    hold_sum: f64,
    holds: u32,
}

pub struct ButtonTracker {
    b: [BtnRec; 5],
    chatter_ns: u64,
}

impl ButtonTracker {
    pub fn new(chatter_ms: f64) -> Self {
        let mut t = Self { b: Default::default(), chatter_ns: (chatter_ms * 1e6) as u64 };
        for (i, r) in t.b.iter_mut().enumerate() {
            r.stat.n = i as u8 + 1;
        }
        t
    }

    pub fn event(&mut self, button: u8, down: bool, ts: u64) -> Option<ClickOutcome> {
        let r = self.b.get_mut(button.checked_sub(1)? as usize)?;
        if down {
            if r.down_at.is_some() {
                return None;
            }
            let gap = r.up_at.map(|u| ts.saturating_sub(u));
            r.down_at = Some(ts);
            r.stat.clicks += 1;
            let chatter = gap.is_some_and(|g| g < self.chatter_ns);
            if chatter {
                r.stat.chatter += 1;
            }
            if let Some(g) = gap {
                let g = g as f64 / 1e6;
                r.stat.min_gap = Some(r.stat.min_gap.map_or(g, |m| m.min(g)));
            }
            Some(ClickOutcome { hold_ms: None, gap_ms: gap.map(|g| g as f64 / 1e6), chatter })
        } else {
            let d = r.down_at.take()?;
            r.up_at = Some(ts);
            let h = ts.saturating_sub(d) as f64 / 1e6;
            r.holds += 1;
            r.hold_sum += h;
            r.stat.hold_min = Some(r.stat.hold_min.map_or(h, |m| m.min(h)));
            r.stat.hold_max = Some(r.stat.hold_max.map_or(h, |m| m.max(h)));
            r.stat.hold_avg = Some(r.hold_sum / r.holds as f64);
            Some(ClickOutcome { hold_ms: Some(h), gap_ms: None, chatter: false })
        }
    }

    pub fn stats(&self) -> Vec<ButtonStat> {
        self.b.iter().map(|r| r.stat.clone()).collect()
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.chatter_ns as f64 / 1e6);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// a slow circle drawn as raw integer counts, like a sensor reports it
    fn circle(t: &mut SensorTest, r: f64, turns: f64, noise: impl Fn(usize) -> (f64, f64)) {
        let n = (turns * 1500.0) as usize;
        let (mut px, mut py) = (r, 0.0);
        let (mut ix, mut iy) = (0.0f64, 0.0f64);
        for i in 1..=n {
            let a = TAU * turns * i as f64 / n as f64;
            let (nx, ny) = noise(i);
            let (x, y) = (r * a.cos() + nx, r * a.sin() + ny);
            ix += x - px;
            iy += y - py;
            px = x;
            py = y;
            let (dx, dy) = (ix.round() as i32, iy.round() as i32);
            ix -= dx as f64;
            iy -= dy as f64;
            t.add(dx, dy);
        }
    }

    #[test]
    fn clean_circle_has_even_directions_and_a_matching_radius() {
        let mut t = SensorTest::new();
        t.start();
        circle(&mut t, 900.0, 2.0, |_| (0.0, 0.0));
        let s = t.snapshot();
        assert!((s.radius.unwrap() - 900.0).abs() < 40.0, "radius {:?}", s.radius);
        assert!(s.snap_ratio.unwrap() < 1.8, "snap {:?}", s.snap_ratio);
        assert!(!s.snapping);
        assert!(s.jitter_pct.unwrap() < 25.0, "jitter {:?}", s.jitter_pct);
    }

    #[test]
    fn noisy_sensor_reads_as_more_jitter() {
        let clean = {
            let mut t = SensorTest::new();
            t.start();
            circle(&mut t, 900.0, 2.0, |_| (0.0, 0.0));
            t.snapshot().jitter_pct.unwrap()
        };
        let noisy = {
            let mut t = SensorTest::new();
            t.start();
            // deterministic pseudo-noise of +-6 counts
            circle(&mut t, 900.0, 2.0, |i| (((i * 7919) % 13) as f64 - 6.0, ((i * 104729) % 11) as f64 - 5.0));
            t.snapshot().jitter_pct.unwrap()
        };
        assert!(noisy > clean * 1.5, "noisy {noisy} vs clean {clean}");
    }

    #[test]
    fn angle_snapping_is_flagged() {
        let mut t = SensorTest::new();
        t.start();
        // a sensor that snaps every movement to the nearest 45 degrees
        for i in 0..6000 {
            let a = TAU * (i as f64 / 6000.0);
            let snapped = (a / (TAU / 8.0)).round() * (TAU / 8.0);
            t.add((3.0 * snapped.cos()).round() as i32 * 5, (3.0 * snapped.sin()).round() as i32 * 5);
        }
        let s = t.snapshot();
        assert!(s.snapping, "ratio {:?}", s.snap_ratio);
        assert!(s.snap_ratio.unwrap() > 4.0);
    }

    #[test]
    fn too_little_data_gives_no_verdict() {
        let mut t = SensorTest::new();
        t.start();
        for _ in 0..30 {
            t.add(5, 5);
        }
        let s = t.snapshot();
        assert!(s.snap_ratio.is_none() && s.jitter_pct.is_none() && s.radius.is_none());
    }

    #[test]
    fn inactive_test_ignores_movement() {
        let mut t = SensorTest::new();
        t.add(100, 100);
        assert_eq!(t.snapshot().steps, 0);
    }

    #[test]
    fn click_hold_and_bounce_are_measured() {
        let mut b = ButtonTracker::new(30.0);
        let ms = 1_000_000u64;
        b.event(1, true, 100 * ms);
        let up = b.event(1, false, 190 * ms).unwrap();
        assert!((up.hold_ms.unwrap() - 90.0).abs() < 0.001);
        let bounce = b.event(1, true, 205 * ms).unwrap(); // 15ms later
        assert!(bounce.chatter);
        b.event(1, false, 260 * ms);
        let ok = b.event(1, true, 500 * ms).unwrap();
        assert!(!ok.chatter);
        let s = &b.stats()[0];
        assert_eq!(s.clicks, 3);
        assert_eq!(s.chatter, 1);
        assert!((s.min_gap.unwrap() - 15.0).abs() < 0.001);
        assert!((s.hold_min.unwrap() - 55.0).abs() < 0.001);
    }

    #[test]
    fn buttons_are_independent_and_out_of_range_is_safe() {
        let mut b = ButtonTracker::new(30.0);
        b.event(1, true, 0);
        assert!(b.event(2, false, 10).is_none(), "release without press");
        assert!(b.event(9, true, 10).is_none());
        assert!(b.event(0, true, 10).is_none());
        assert_eq!(b.stats()[1].clicks, 0);
    }
}
