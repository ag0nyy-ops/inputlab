// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! per-key timing from raw timestamps: hold time, chatter (release->press
//! bounce shorter than the threshold), and how many keys are down at once.
//! os auto-repeat (a press while already down) is ignored, not counted as chatter.

use std::collections::HashMap;

use crate::timing::ns_to_ms;

pub const CHATTER_DEFAULT_MS: f64 = 30.0;

#[derive(Debug, Clone, PartialEq)]
pub struct KeyOutcome {
    /// ms the key was held (on release)
    pub hold_ms: Option<f64>,
    /// ms since the previous release of this key (on press)
    pub gap_ms: Option<f64>,
    /// press came back faster than the chatter threshold
    pub chatter: bool,
    /// keys down right now, after this event
    pub held: u32,
    /// most keys ever down at once this session
    pub max_held: u32,
}

pub struct KeyTracker {
    down: HashMap<&'static str, u64>,
    last_up: HashMap<&'static str, u64>,
    chatter_ns: u64,
    max_held: u32,
}

impl KeyTracker {
    pub fn new(chatter_ms: f64) -> Self {
        Self { down: HashMap::new(), last_up: HashMap::new(), chatter_ns: (chatter_ms * 1e6) as u64, max_held: 0 }
    }

    /// None when the event carries no new information (auto-repeat, stray release).
    pub fn event(&mut self, code: &'static str, is_down: bool, ts: u64) -> Option<KeyOutcome> {
        if is_down {
            if self.down.contains_key(code) {
                return None;
            }
            let gap = self.last_up.get(code).map(|&u| ts.saturating_sub(u));
            self.down.insert(code, ts);
            let held = self.down.len() as u32;
            self.max_held = self.max_held.max(held);
            Some(KeyOutcome {
                hold_ms: None,
                gap_ms: gap.map(ns_to_ms),
                chatter: gap.is_some_and(|g| g < self.chatter_ns),
                held,
                max_held: self.max_held,
            })
        } else {
            let since = self.down.remove(code)?;
            self.last_up.insert(code, ts);
            Some(KeyOutcome { hold_ms: Some(ns_to_ms(ts.saturating_sub(since))), gap_ms: None, chatter: false, held: self.down.len() as u32, max_held: self.max_held })
        }
    }

    pub fn reset(&mut self) {
        self.down.clear();
        self.last_up.clear();
        self.max_held = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const MS: u64 = 1_000_000;

    #[test]
    fn hold_time_is_measured_on_release() {
        let mut t = KeyTracker::new(30.0);
        t.event("KeyA", true, 10 * MS).unwrap();
        let up = t.event("KeyA", false, 135 * MS).unwrap();
        assert!((up.hold_ms.unwrap() - 125.0).abs() < 0.001);
    }

    #[test]
    fn auto_repeat_is_ignored() {
        let mut t = KeyTracker::new(30.0);
        assert!(t.event("KeyA", true, 0).is_some());
        for i in 1..20 {
            assert!(t.event("KeyA", true, i * 30 * MS).is_none(), "repeat must not be an event");
        }
        let up = t.event("KeyA", false, 600 * MS).unwrap();
        assert!((up.hold_ms.unwrap() - 600.0).abs() < 0.001, "hold counts from the first press");
    }

    #[test]
    fn bounce_under_threshold_is_chatter() {
        let mut t = KeyTracker::new(30.0);
        t.event("KeyA", true, 0);
        t.event("KeyA", false, 80 * MS);
        let again = t.event("KeyA", true, 95 * MS).unwrap(); // 15ms after release
        assert!(again.chatter);
        assert!((again.gap_ms.unwrap() - 15.0).abs() < 0.001);
    }

    #[test]
    fn a_normal_retap_is_not_chatter() {
        let mut t = KeyTracker::new(30.0);
        t.event("KeyA", true, 0);
        t.event("KeyA", false, 60 * MS);
        assert!(!t.event("KeyA", true, 160 * MS).unwrap().chatter);
    }

    #[test]
    fn first_press_has_no_gap_and_never_chatters() {
        let mut t = KeyTracker::new(30.0);
        let d = t.event("KeyZ", true, 0).unwrap();
        assert!(d.gap_ms.is_none() && !d.chatter);
    }

    #[test]
    fn chatter_is_per_key() {
        let mut t = KeyTracker::new(30.0);
        t.event("KeyA", true, 0);
        t.event("KeyA", false, 50 * MS);
        assert!(!t.event("KeyB", true, 55 * MS).unwrap().chatter);
    }

    #[test]
    fn rollover_counts_simultaneous_keys_and_remembers_the_max() {
        let mut t = KeyTracker::new(30.0);
        let keys = ["KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ"];
        let mut last = None;
        for (i, k) in keys.iter().enumerate() {
            last = t.event(k, true, i as u64 * MS);
        }
        assert_eq!(last.as_ref().unwrap().held, 7);
        for (i, k) in keys.iter().enumerate().take(5) {
            last = t.event(k, false, (100 + i as u64) * MS);
        }
        let o = last.unwrap();
        assert_eq!(o.held, 2);
        assert_eq!(o.max_held, 7);
    }

    #[test]
    fn a_ghosted_key_that_never_reports_is_not_counted() {
        // 6kro boards drop the 7th key: the tracker only knows what it is told
        let mut t = KeyTracker::new(30.0);
        for (i, k) in ["KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH"].iter().enumerate() {
            t.event(k, true, i as u64);
        }
        assert_eq!(t.event("KeyH", false, 10 * MS).unwrap().max_held, 6);
    }

    #[test]
    fn stray_release_is_ignored_and_reset_clears() {
        let mut t = KeyTracker::new(30.0);
        assert!(t.event("KeyA", false, 0).is_none());
        t.event("KeyA", true, 1);
        t.reset();
        assert!(t.event("KeyA", false, 2).is_none());
        assert_eq!(t.event("KeyB", true, 3).unwrap().max_held, 1);
    }
}
