//! Fixed-step 35 Hz tic accumulator for the windowed host.
//!
//! `abrash`'s `run_windowed` drives `update`/`render` at a variable (per-redraw)
//! rate, but Doom's simulation must advance at a fixed 35 Hz. This mirrors
//! doom-tui's `drain_ready_tics` accumulator, reusing the same
//! [`doom_tui::TIC_DURATION`] constant so the two hosts stay in
//! lockstep. The struct holds no game state, so its arithmetic is unit-testable
//! without a window.

use doom_tui::TIC_DURATION;
use std::time::Duration;

/// Accumulates elapsed wall-clock time and releases whole 35 Hz tics.
#[derive(Debug, Default)]
pub struct TicAccumulator {
    remainder: Duration,
}

impl TicAccumulator {
    /// Create an empty accumulator.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            remainder: Duration::ZERO,
        }
    }

    /// Add `dt` of elapsed time and return how many whole tics are now due.
    ///
    /// The sub-tic remainder carries over to the next call, exactly matching
    /// doom-tui's `while accumulator >= TIC_DURATION { accumulator -= TIC_DURATION }`.
    pub fn advance(&mut self, dt: Duration) -> u32 {
        self.remainder += dt;
        let mut tics = 0u32;
        while self.remainder >= TIC_DURATION {
            self.remainder -= TIC_DURATION;
            tics += 1;
        }
        tics
    }

    /// Convenience wrapper for the `f32` seconds delta that abrash's
    /// `WindowContext::dt_seconds` provides.
    pub fn advance_secs(&mut self, dt_seconds: f32) -> u32 {
        let clamped = dt_seconds.max(0.0);
        self.advance(Duration::from_secs_f32(clamped))
    }

    /// The current sub-tic carry (time not yet consumed as a tic).
    #[must_use]
    pub const fn remainder(&self) -> Duration {
        self.remainder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_tui::TIC_RATE_HZ;

    #[test]
    fn one_second_yields_35_tics() {
        let mut acc = TicAccumulator::new();
        assert_eq!(acc.advance(Duration::from_secs(1)), TIC_RATE_HZ);
    }

    #[test]
    fn fractional_time_carries_over() {
        let mut acc = TicAccumulator::new();
        // 2.5 tics of time → 2 tics, half a tic carried.
        let elapsed = TIC_DURATION * 2 + TIC_DURATION / 2;
        assert_eq!(acc.advance(elapsed), 2);
        assert_eq!(acc.remainder(), TIC_DURATION / 2);
        // Adding another half-tic completes the third tic.
        assert_eq!(acc.advance(TIC_DURATION / 2), 1);
        assert!(acc.remainder() < TIC_DURATION);
    }

    #[test]
    fn sub_tic_delta_yields_zero() {
        let mut acc = TicAccumulator::new();
        assert_eq!(acc.advance(TIC_DURATION / 3), 0);
        assert_eq!(acc.advance(TIC_DURATION / 3), 0);
        // Third third completes one tic.
        assert_eq!(acc.advance(TIC_DURATION / 3 + Duration::from_nanos(2)), 1);
    }

    #[test]
    fn advance_secs_matches_duration_path() {
        let mut acc = TicAccumulator::new();
        assert_eq!(acc.advance_secs(1.0), TIC_RATE_HZ);
    }

    #[test]
    fn negative_secs_is_clamped() {
        let mut acc = TicAccumulator::new();
        assert_eq!(acc.advance_secs(-5.0), 0);
    }
}
