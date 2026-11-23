// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
//! guided dpi (cpi) test. the user moves the mouse a known physical distance
//! while we sum the raw counts. raw input bypasses os acceleration and pointer
//! speed, so counts / inches is the sensor's true cpi, as accurate as the
//! user's measuring.

use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DpiState {
    Idle,
    Counting,
    Done,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DpiSnap {
    pub state: DpiState,
    pub distance_in: f64,
    /// net displacement in counts (wobble cancels out)
    pub counts: f64,
    /// total path length in counts
    pub path: f64,
    /// net / path: 1.0 is a perfectly straight move
    pub straightness: f64,
    pub dpi: Option<f64>,
}

pub struct DpiTest {
    state: DpiState,
    distance_in: f64,
    sx: i64,
    sy: i64,
    path: f64,
}

impl Default for DpiTest {
    fn default() -> Self {
        Self::new()
    }
}

impl DpiTest {
    pub fn new() -> Self {
        Self { state: DpiState::Idle, distance_in: 2.0, sx: 0, sy: 0, path: 0.0 }
    }

    pub fn start(&mut self, distance_in: f64) {
        self.distance_in = if distance_in.is_finite() { distance_in.clamp(0.25, 40.0) } else { 2.0 };
        self.sx = 0;
        self.sy = 0;
        self.path = 0.0;
        self.state = DpiState::Counting;
    }

    pub fn stop(&mut self) {
        if self.state == DpiState::Counting {
            self.state = DpiState::Done;
        }
    }

    pub fn reset(&mut self) {
        let d = self.distance_in;
        *self = Self::new();
        self.distance_in = d;
    }

    pub fn is_counting(&self) -> bool {
        self.state == DpiState::Counting
    }

    /// relative counts from one report. absolute devices (tablets, remote desktop) are skipped by the caller.
    pub fn add(&mut self, dx: i32, dy: i32) {
        if self.state != DpiState::Counting {
            return;
        }
        self.sx += dx as i64;
        self.sy += dy as i64;
        self.path += ((dx as f64).powi(2) + (dy as f64).powi(2)).sqrt();
    }

    pub fn snapshot(&self) -> DpiSnap {
        let counts = ((self.sx as f64).powi(2) + (self.sy as f64).powi(2)).sqrt();
        DpiSnap {
            state: self.state,
            distance_in: self.distance_in,
            counts,
            path: self.path,
            straightness: if self.path > 0.0 { counts / self.path } else { 1.0 },
            dpi: (self.state != DpiState::Idle && counts > 0.0).then(|| counts / self.distance_in),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(test: &mut DpiTest, dist_in: f64, cpi: f64, per_report: i32) {
        let total = (dist_in * cpi) as i32;
        let mut left = total;
        while left > 0 {
            let d = left.min(per_report);
            test.add(d, 0);
            left -= d;
        }
    }

    #[test]
    fn two_inches_at_1600_cpi() {
        let mut t = DpiTest::new();
        t.start(2.0);
        run(&mut t, 2.0, 1600.0, 7);
        t.stop();
        let s = t.snapshot();
        assert_eq!(s.state, DpiState::Done);
        assert!((s.dpi.unwrap() - 1600.0).abs() < 1.0);
    }

    #[test]
    fn centimetres_work_through_inches() {
        let mut t = DpiTest::new();
        t.start(5.0 / 2.54); // 5 cm
        run(&mut t, 5.0 / 2.54, 800.0, 3);
        assert!((t.snapshot().dpi.unwrap() - 800.0).abs() < 1.5);
    }

    #[test]
    fn wobble_cancels_in_net_displacement() {
        let mut t = DpiTest::new();
        t.start(1.0);
        for i in 0..1000 {
            t.add(2, if i % 2 == 0 { 40 } else { -40 }); // zig-zag while moving right
        }
        let s = t.snapshot();
        assert!((s.counts - 2000.0).abs() < 50.0, "net {}", s.counts);
        assert!(s.path > 30_000.0);
        assert!(s.straightness < 0.1);
    }

    #[test]
    fn diagonal_moves_use_the_vector_length() {
        let mut t = DpiTest::new();
        t.start(1.0);
        for _ in 0..300 {
            t.add(3, 4);
        }
        assert!((t.snapshot().counts - 1500.0).abs() < 0.001);
    }

    #[test]
    fn out_and_back_nets_to_zero() {
        let mut t = DpiTest::new();
        t.start(2.0);
        t.add(1000, 0);
        t.add(-1000, 0);
        assert_eq!(t.snapshot().counts, 0.0);
        assert!(t.snapshot().dpi.is_none());
    }

    #[test]
    fn nothing_counts_before_start_or_after_stop() {
        let mut t = DpiTest::new();
        t.add(500, 0);
        assert_eq!(t.snapshot().counts, 0.0);
        t.start(2.0);
        t.add(100, 0);
        t.stop();
        t.add(900, 0);
        assert_eq!(t.snapshot().counts, 100.0);
    }

    #[test]
    fn reset_keeps_the_distance_and_clears_counts() {
        let mut t = DpiTest::new();
        t.start(5.0);
        t.add(100, 0);
        t.reset();
        let s = t.snapshot();
        assert_eq!(s.state, DpiState::Idle);
        assert_eq!(s.distance_in, 5.0);
        assert_eq!(s.counts, 0.0);
    }

    #[test]
    fn hostile_distances_are_clamped() {
        let mut t = DpiTest::new();
        t.start(0.0);
        assert_eq!(t.snapshot().distance_in, 0.25);
        t.start(f64::NAN);
        assert_eq!(t.snapshot().distance_in, 2.0);
    }
}
