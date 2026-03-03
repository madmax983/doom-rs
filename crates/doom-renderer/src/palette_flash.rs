//! Palette flash controller for pain, pickup, and radiation effects.
//!
//! Doom swaps the entire PLAYPAL palette to produce full-screen colour tints:
//!
//! | Palette index | Effect                                  |
//! |---------------|-----------------------------------------|
//! | 0             | Normal (no tint)                        |
//! | 1-8           | Pain flash (red tint, increasing)       |
//! | 9-12          | Pickup flash (gold/yellow tint)         |
//! | 13            | Radiation suit (green tint)             |
//!
//! `PaletteFlash` tracks the active palette index and a duration in tics.
//! Each call to `tick()` decrements the remaining duration; when it hits
//! zero the palette snaps back to index 0.  A new `trigger()` call
//! overrides any ongoing flash.

/// Number of available palette entries in PLAYPAL (indices 0-13).
pub const MAX_PALETTE_INDEX: usize = 13;

/// Palette flash state machine.
///
/// Tracks which palette tint is active and how many tics remain before
/// reverting to the default (index 0) palette.
#[derive(Clone, Debug)]
pub struct PaletteFlash {
    /// Currently active palette index (0 = normal).
    palette_index: usize,
    /// Remaining tics before the flash expires and palette returns to 0.
    remaining_tics: u32,
}

impl PaletteFlash {
    /// Create a new flash controller starting at palette 0 (no flash).
    pub fn new() -> Self {
        Self {
            palette_index: 0,
            remaining_tics: 0,
        }
    }

    /// Trigger a palette flash.
    ///
    /// - `palette_index` — which PLAYPAL palette to use (1-13 for effects;
    ///   0 effectively cancels any active flash).
    /// - `duration` — how many tics the flash should last before reverting
    ///   to palette 0.
    ///
    /// A new trigger *always* overrides any ongoing flash.
    pub fn trigger(&mut self, palette_index: usize, duration: u32) {
        self.palette_index = palette_index.min(MAX_PALETTE_INDEX);
        self.remaining_tics = duration;
    }

    /// Advance the flash by one tic.
    ///
    /// Decrements the remaining duration. When it reaches zero, the palette
    /// index is reset to 0 (normal).
    pub fn tick(&mut self) {
        if self.remaining_tics > 0 {
            self.remaining_tics -= 1;
            if self.remaining_tics == 0 {
                self.palette_index = 0;
            }
        }
    }

    /// Return the currently active palette index.
    ///
    /// Callers use this to select which PLAYPAL palette to blit with.
    pub fn active_palette(&self) -> usize {
        self.palette_index
    }

    /// Return the remaining duration in tics.
    pub fn remaining(&self) -> u32 {
        self.remaining_tics
    }
}

impl Default for PaletteFlash {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_palette_zero() {
        let flash = PaletteFlash::new();
        assert_eq!(flash.active_palette(), 0);
        assert_eq!(flash.remaining(), 0);
    }

    #[test]
    fn trigger_sets_palette_and_duration() {
        let mut flash = PaletteFlash::new();
        flash.trigger(3, 10);
        assert_eq!(flash.active_palette(), 3);
        assert_eq!(flash.remaining(), 10);
    }

    #[test]
    fn tick_decrements_duration() {
        let mut flash = PaletteFlash::new();
        flash.trigger(5, 3);
        flash.tick();
        assert_eq!(flash.remaining(), 2);
        assert_eq!(flash.active_palette(), 5);
    }

    #[test]
    fn returns_to_palette_zero_when_duration_expires() {
        let mut flash = PaletteFlash::new();
        flash.trigger(8, 2);

        flash.tick(); // remaining = 1, still active
        assert_eq!(flash.active_palette(), 8);

        flash.tick(); // remaining = 0, reset to palette 0
        assert_eq!(flash.active_palette(), 0);
        assert_eq!(flash.remaining(), 0);
    }

    #[test]
    fn active_palette_returns_current_index() {
        let mut flash = PaletteFlash::new();
        assert_eq!(flash.active_palette(), 0);

        flash.trigger(12, 5);
        assert_eq!(flash.active_palette(), 12);

        // Tick down partially — still active.
        flash.tick();
        flash.tick();
        assert_eq!(flash.active_palette(), 12);
    }

    #[test]
    fn new_trigger_overrides_old_one() {
        let mut flash = PaletteFlash::new();
        flash.trigger(3, 100);
        assert_eq!(flash.active_palette(), 3);
        assert_eq!(flash.remaining(), 100);

        // Override with a new flash.
        flash.trigger(9, 5);
        assert_eq!(flash.active_palette(), 9);
        assert_eq!(flash.remaining(), 5);
    }

    #[test]
    fn tick_is_noop_when_no_flash_active() {
        let mut flash = PaletteFlash::new();
        flash.tick(); // Should not panic or underflow.
        assert_eq!(flash.active_palette(), 0);
        assert_eq!(flash.remaining(), 0);
    }

    #[test]
    fn trigger_clamps_palette_index_to_max() {
        let mut flash = PaletteFlash::new();
        flash.trigger(999, 10);
        assert_eq!(flash.active_palette(), MAX_PALETTE_INDEX);
    }

    #[test]
    fn trigger_with_zero_duration_resets_immediately() {
        let mut flash = PaletteFlash::new();
        flash.trigger(5, 0);
        // Duration is 0, so after next tick it should be at 0.
        // But even before tick, palette_index is 5 with duration 0.
        // Next tick won't decrement past 0.
        assert_eq!(flash.active_palette(), 5);
        assert_eq!(flash.remaining(), 0);
        flash.tick();
        // Tick with remaining=0 is a noop — palette stays as-is.
        assert_eq!(flash.active_palette(), 5);
    }

    #[test]
    fn full_pain_flash_lifecycle() {
        let mut flash = PaletteFlash::new();

        // Take damage: pain flash (palette 4, red tint) for 10 tics.
        flash.trigger(4, 10);
        for _ in 0..9 {
            assert_eq!(flash.active_palette(), 4);
            flash.tick();
        }
        // 10th tick → expires.
        assert_eq!(flash.active_palette(), 4);
        flash.tick();
        assert_eq!(flash.active_palette(), 0);
    }

    #[test]
    fn default_matches_new() {
        let default_flash = PaletteFlash::default();
        let new_flash = PaletteFlash::new();
        assert_eq!(default_flash.active_palette(), new_flash.active_palette());
        assert_eq!(default_flash.remaining(), new_flash.remaining());
    }
}
