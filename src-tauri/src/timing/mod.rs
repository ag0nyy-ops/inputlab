// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
//! monotonic clock shared by every backend. all timestamps in rust are
//! nanoseconds on this clock, so polling math never depends on frame timing.

pub mod dpi;
pub mod keystats;
pub mod poll;
pub mod sensor;
pub mod stick;

use std::sync::OnceLock;
use std::time::Instant;

static ORIGIN: OnceLock<Instant> = OnceLock::new();

/// nanoseconds since the first call. on windows `Instant` is backed by
/// QueryPerformanceCounter; the windows platform also exposes a direct QPC
/// reading that is rebased onto this same origin.
pub fn now_ns() -> u64 {
    let origin = ORIGIN.get_or_init(Instant::now);
    origin.elapsed().as_nanos() as u64
}

pub fn ns_to_ms(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_is_monotonic() {
        let a = now_ns();
        let b = now_ns();
        assert!(b >= a);
    }

    #[test]
    fn ms_conversion() {
        assert_eq!(ns_to_ms(1_500_000), 1.5);
    }
}
