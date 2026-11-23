//! polling rate from raw report timestamps. every number here comes from the
//! nanosecond timestamps of the reports themselves, never from frame timing,
//! so it stays accurate for 2k, 4k and 8k mice as long as the os delivers them.
//!
//! idle time is excluded: a mouse only sends reports while it moves, so gaps
//! longer than `PAUSE_NS` are pauses, not slow polling.

use std::collections::VecDeque;

const PAUSE_NS: u64 = 50_000_000;
const WINDOW_NS: u64 = 100_000_000;
const LIVE_SPAN_NS: u64 = 400_000_000;
const GRAPH_LEN: usize = 60;
const NOMINAL: [u32; 8] = [125, 250, 500, 800, 1000, 2000, 4000, 8000];

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PollSnap {
    /// rate over the last ~0.4s of movement. None when the mouse is still.
    pub hz: Option<f64>,
    /// average over all movement this session
    pub avg_hz: Option<f64>,
    /// slowest and fastest 100ms windows seen
    pub min_hz: Option<f64>,
    pub max_hz: Option<f64>,
    pub interval_min_us: Option<f64>,
    pub interval_avg_us: Option<f64>,
    pub interval_max_us: Option<f64>,
    /// standard deviation of the report interval
    pub jitter_us: Option<f64>,
    /// the usual polling rate this is closest to, when it is within 8%
    pub nominal: Option<u32>,
    pub reports: u64,
    /// recent 100ms-window rates, oldest first, for the stability graph
    pub graph: Vec<f64>,
}

pub struct PollEstimator {
    last: Option<u64>,
    reports: u64,
    interval_n: u64,
    interval_sum: f64,
    interval_sq: f64,
    interval_min: u64,
    interval_max: u64,
    recent: VecDeque<u64>,
    window: Vec<u64>,
    graph: VecDeque<f64>,
    min_hz: Option<f64>,
    max_hz: Option<f64>,
}

impl Default for PollEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl PollEstimator {
    pub fn new() -> Self {
        Self {
            last: None,
            reports: 0,
            interval_n: 0,
            interval_sum: 0.0,
            interval_sq: 0.0,
            interval_min: u64::MAX,
            interval_max: 0,
            recent: VecDeque::new(),
            window: Vec::new(),
            graph: VecDeque::new(),
            min_hz: None,
            max_hz: None,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// one movement report at `ts` (ns)
    pub fn push(&mut self, ts: u64) {
        self.reports += 1;
        if let Some(last) = self.last {
            let d = ts.saturating_sub(last);
            if d > 0 && d < PAUSE_NS {
                self.interval_n += 1;
                self.interval_sum += d as f64;
                self.interval_sq += (d as f64) * (d as f64);
                self.interval_min = self.interval_min.min(d);
                self.interval_max = self.interval_max.max(d);
            } else if d >= PAUSE_NS {
                self.close_window();
            }
        }
        self.last = Some(ts);
        self.recent.push_back(ts);
        while self.recent.front().is_some_and(|&t| ts.saturating_sub(t) > LIVE_SPAN_NS) {
            self.recent.pop_front();
        }
        if let Some(&first) = self.window.first() {
            if ts.saturating_sub(first) >= WINDOW_NS {
                self.window.push(ts);
                self.close_window();
                return;
            }
        }
        self.window.push(ts);
    }

    /// finish the current 100ms window: its rate is (reports-1)/span, which stays
    /// right even when the window only partly overlaps the movement.
    fn close_window(&mut self) {
        let w = std::mem::take(&mut self.window);
        if w.len() >= 4 {
            let span = (w[w.len() - 1] - w[0]) as f64;
            if span >= 60_000_000.0 {
                let hz = (w.len() - 1) as f64 / (span / 1e9);
                self.min_hz = Some(self.min_hz.map_or(hz, |m| m.min(hz)));
                self.max_hz = Some(self.max_hz.map_or(hz, |m| m.max(hz)));
                self.graph.push_back(hz);
                while self.graph.len() > GRAPH_LEN {
                    self.graph.pop_front();
                }
            }
        }
    }

    pub fn snapshot(&self, now: u64) -> PollSnap {
        let live = match (self.recent.front(), self.recent.back()) {
            (Some(&a), Some(&b)) if self.recent.len() >= 4 && now.saturating_sub(b) < 150_000_000 && b > a => Some((self.recent.len() - 1) as f64 / ((b - a) as f64 / 1e9)),
            _ => None,
        };
        let avg_interval = (self.interval_n > 0).then(|| self.interval_sum / self.interval_n as f64);
        let avg_hz = avg_interval.map(|i| 1e9 / i);
        let jitter = avg_interval.filter(|_| self.interval_n > 1).map(|m| ((self.interval_sq / self.interval_n as f64) - m * m).max(0.0).sqrt() / 1000.0);
        let nominal = avg_hz.and_then(|h| {
            NOMINAL.iter().copied().find(|&n| (h - n as f64).abs() / n as f64 <= 0.08)
        });
        PollSnap {
            hz: live,
            avg_hz,
            min_hz: self.min_hz,
            max_hz: self.max_hz,
            interval_min_us: (self.interval_n > 0).then(|| self.interval_min as f64 / 1000.0),
            interval_avg_us: avg_interval.map(|i| i / 1000.0),
            interval_max_us: (self.interval_n > 0).then(|| self.interval_max as f64 / 1000.0),
            jitter_us: jitter,
            nominal,
            reports: self.reports,
            graph: self.graph.iter().copied().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(p: &mut PollEstimator, hz: f64, secs: f64, start: u64) -> u64 {
        let step = 1e9 / hz;
        let n = (hz * secs) as u64;
        let mut t = start as f64;
        for _ in 0..n {
            p.push(t as u64);
            t += step;
        }
        t as u64
    }

    fn near(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn measures_125_1000_and_8000hz() {
        for hz in [125.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0] {
            let mut p = PollEstimator::new();
            let end = feed(&mut p, hz, 2.0, 1_000_000_000);
            let s = p.snapshot(end);
            assert!(near(s.avg_hz.unwrap(), hz, hz * 0.005), "{hz}: avg {:?}", s.avg_hz);
            assert!(near(s.hz.unwrap(), hz, hz * 0.02), "{hz}: live {:?}", s.hz);
            assert!(near(s.min_hz.unwrap(), hz, hz * 0.03), "{hz}: min {:?}", s.min_hz);
            assert!(near(s.max_hz.unwrap(), hz, hz * 0.03), "{hz}: max {:?}", s.max_hz);
            assert_eq!(s.nominal, Some(hz as u32));
            assert!(s.jitter_us.unwrap() < 1.0, "perfect timing has no jitter");
        }
    }

    #[test]
    fn eight_khz_interval_is_125_microseconds() {
        let mut p = PollEstimator::new();
        let end = feed(&mut p, 8000.0, 1.0, 0);
        let s = p.snapshot(end);
        assert!(near(s.interval_avg_us.unwrap(), 125.0, 0.5));
        assert!(near(s.interval_min_us.unwrap(), 125.0, 1.0));
    }

    #[test]
    fn jitter_and_extremes_show_up() {
        let mut p = PollEstimator::new();
        let mut t = 0u64;
        for i in 0..3000u64 {
            // 1000hz with every 100th report arriving 2ms late
            t += 1_000_000 + if i % 100 == 99 { 2_000_000 } else { 0 };
            p.push(t);
        }
        let s = p.snapshot(t);
        assert!(s.interval_max_us.unwrap() >= 2900.0);
        assert!(near(s.interval_min_us.unwrap(), 1000.0, 1.0));
        assert!(s.jitter_us.unwrap() > 100.0);
        assert!(s.avg_hz.unwrap() < 1000.0);
    }

    #[test]
    fn pauses_are_not_slow_polling() {
        let mut p = PollEstimator::new();
        let mut t = feed(&mut p, 1000.0, 1.0, 0);
        t += 3_000_000_000; // hand off the mouse for 3 seconds
        let end = feed(&mut p, 1000.0, 1.0, t);
        let s = p.snapshot(end);
        assert!(near(s.avg_hz.unwrap(), 1000.0, 5.0));
        assert!(s.min_hz.unwrap() > 950.0, "the pause must not create a 0hz window");
    }

    #[test]
    fn live_rate_goes_away_when_the_mouse_stops() {
        let mut p = PollEstimator::new();
        let end = feed(&mut p, 1000.0, 1.0, 0);
        assert!(p.snapshot(end).hz.is_some());
        assert!(p.snapshot(end + 400_000_000).hz.is_none());
    }

    #[test]
    fn odd_rates_have_no_nominal() {
        let mut p = PollEstimator::new();
        let end = feed(&mut p, 650.0, 1.0, 0);
        assert_eq!(p.snapshot(end).nominal, None);
    }

    #[test]
    fn empty_and_single_report_are_safe() {
        let mut p = PollEstimator::new();
        let s = p.snapshot(0);
        assert!(s.avg_hz.is_none() && s.hz.is_none());
        p.push(5);
        let s = p.snapshot(10);
        assert_eq!(s.reports, 1);
        assert!(s.avg_hz.is_none());
    }

    #[test]
    fn graph_holds_the_recent_windows_only() {
        let mut p = PollEstimator::new();
        let end = feed(&mut p, 1000.0, 10.0, 0);
        let s = p.snapshot(end);
        assert_eq!(s.graph.len(), GRAPH_LEN);
    }

    #[test]
    fn reset_clears_everything() {
        let mut p = PollEstimator::new();
        feed(&mut p, 1000.0, 1.0, 0);
        p.reset();
        assert_eq!(p.snapshot(0).reports, 0);
    }
}
