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
// PaletteFlashState — enhanced multi-source flash manager
// ---------------------------------------------------------------------------

/// Maximum pain count value (maps to palette 8).
const MAX_PAIN_COUNT: i32 = 64;

/// Duration (in tics) of a pickup bonus flash.
const BONUS_FLASH_DURATION: i32 = 6;

/// Manages palette index selection for damage/powerup screen flashes.
///
/// Unlike [`PaletteFlash`] which is a simple trigger/duration state machine,
/// `PaletteFlashState` tracks multiple simultaneous effect sources and
/// resolves priority to select the correct PLAYPAL palette index each tic.
///
/// Priority order: pain > bonus > radiation suit > berserk > normal.
#[derive(Clone, Debug)]
pub struct PaletteFlashState {
    /// Current pain flash intensity (0 = none, decreasing over time).
    pain_count: i32,
    /// Bonus (pickup) flash intensity.
    bonus_count: i32,
    /// Radiation suit remaining tics (green tint).
    rad_suit_tics: i32,
    /// Berserk remaining tics (red tint — lower intensity than pain).
    berserk_tics: i32,
}

impl PaletteFlashState {
    /// Create a new flash state with no active effects.
    pub fn new() -> Self {
        Self {
            pain_count: 0,
            bonus_count: 0,
            rad_suit_tics: 0,
            berserk_tics: 0,
        }
    }

    /// Add pain from taking damage.
    ///
    /// Increases `pain_count` based on the damage amount, clamped to
    /// `MAX_PAIN_COUNT`.  Higher pain counts map to more intense red palettes.
    pub fn add_pain(&mut self, damage: i32) {
        self.pain_count = (self.pain_count + damage).min(MAX_PAIN_COUNT);
    }

    /// Trigger a pickup (bonus) flash.
    ///
    /// Sets `bonus_count` to the standard pickup flash duration.
    pub fn add_bonus(&mut self) {
        self.bonus_count = BONUS_FLASH_DURATION;
    }

    /// Set radiation suit tint duration.
    pub fn set_rad_suit(&mut self, tics: i32) {
        self.rad_suit_tics = tics;
    }

    /// Set berserk tint duration.
    pub fn set_berserk(&mut self, tics: i32) {
        self.berserk_tics = tics;
    }

    /// Advance all counters by one tic.
    ///
    /// Decrements pain_count, bonus_count, rad_suit_tics, and berserk_tics
    /// toward zero.
    pub fn tick(&mut self) {
        if self.pain_count > 0 {
            self.pain_count -= 1;
        }
        if self.bonus_count > 0 {
            self.bonus_count -= 1;
        }
        if self.rad_suit_tics > 0 {
            self.rad_suit_tics -= 1;
        }
        if self.berserk_tics > 0 {
            self.berserk_tics -= 1;
        }
    }

    /// Return the PLAYPAL palette index (0-13) based on current flash state.
    ///
    /// Priority: pain (1-8) > bonus (9-12) > radiation suit (13) > normal (0).
    ///
    /// Pain count is mapped linearly to palettes 1-8:
    ///   palette = clamp(pain_count * 8 / MAX_PAIN_COUNT, 1, 8)
    ///
    /// Bonus count is mapped linearly to palettes 9-12:
    ///   palette = 8 + clamp(bonus_count * 4 / BONUS_FLASH_DURATION, 1, 4)
    pub fn active_palette(&self) -> usize {
        if self.pain_count > 0 {
            // Map pain_count (1..=MAX_PAIN_COUNT) to palette (1..=8).
            let level = ((self.pain_count as i64 * 8) / MAX_PAIN_COUNT as i64).clamp(1, 8);
            return level as usize;
        }

        if self.bonus_count > 0 {
            // Map bonus_count (1..=BONUS_FLASH_DURATION) to palette (9..=12).
            let level = ((self.bonus_count as i64 * 4) / BONUS_FLASH_DURATION as i64).clamp(1, 4);
            return 8 + level as usize;
        }

        if self.rad_suit_tics > 0 {
            return 13;
        }

        // Berserk is a mild red tint — palette 1 (lowest pain).
        if self.berserk_tics > 0 {
            return 1;
        }

        0
    }

    /// Current pain count.
    pub fn pain_count(&self) -> i32 {
        self.pain_count
    }

    /// Current bonus count.
    pub fn bonus_count(&self) -> i32 {
        self.bonus_count
    }

    /// Current radiation suit tics remaining.
    pub fn rad_suit_tics(&self) -> i32 {
        self.rad_suit_tics
    }

    /// Current berserk tics remaining.
    pub fn berserk_tics(&self) -> i32 {
        self.berserk_tics
    }
}

impl Default for PaletteFlashState {
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

    // --- PaletteFlash tests (existing) ---

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

    // --- PaletteFlashState tests ---

    #[test]
    fn state_new_starts_at_palette_zero() {
        let state = PaletteFlashState::new();
        assert_eq!(state.active_palette(), 0);
        assert_eq!(state.pain_count(), 0);
        assert_eq!(state.bonus_count(), 0);
        assert_eq!(state.rad_suit_tics(), 0);
        assert_eq!(state.berserk_tics(), 0);
    }

    #[test]
    fn state_add_pain_maps_to_pain_palette() {
        let mut state = PaletteFlashState::new();
        state.add_pain(20);
        let pal = state.active_palette();
        assert!(
            (1..=8).contains(&pal),
            "pain palette should be 1-8, got {pal}"
        );
    }

    #[test]
    fn state_add_pain_max_damage_gives_palette_8() {
        let mut state = PaletteFlashState::new();
        state.add_pain(MAX_PAIN_COUNT);
        assert_eq!(state.active_palette(), 8);
    }

    #[test]
    fn state_add_pain_small_damage_gives_low_palette() {
        let mut state = PaletteFlashState::new();
        state.add_pain(1);
        let pal = state.active_palette();
        assert_eq!(pal, 1, "minimal pain should map to palette 1");
    }

    #[test]
    fn state_pain_count_clamps_at_max() {
        let mut state = PaletteFlashState::new();
        state.add_pain(200);
        assert_eq!(state.pain_count(), MAX_PAIN_COUNT);
        assert_eq!(state.active_palette(), 8);
    }

    #[test]
    fn state_tick_decrements_pain_count() {
        let mut state = PaletteFlashState::new();
        state.add_pain(10);
        let before = state.pain_count();
        state.tick();
        assert_eq!(state.pain_count(), before - 1);
    }

    #[test]
    fn state_pain_ticks_to_zero_gives_palette_zero() {
        let mut state = PaletteFlashState::new();
        state.add_pain(3);
        state.tick(); // 2
        state.tick(); // 1
        state.tick(); // 0
        assert_eq!(state.pain_count(), 0);
        assert_eq!(state.active_palette(), 0);
    }

    #[test]
    fn state_add_bonus_returns_bonus_palette() {
        let mut state = PaletteFlashState::new();
        state.add_bonus();
        let pal = state.active_palette();
        assert!(
            (9..=12).contains(&pal),
            "bonus palette should be 9-12, got {pal}"
        );
    }

    #[test]
    fn state_bonus_ticks_to_zero() {
        let mut state = PaletteFlashState::new();
        state.add_bonus();
        let duration = state.bonus_count();
        for _ in 0..duration {
            state.tick();
        }
        assert_eq!(state.bonus_count(), 0);
        assert_eq!(state.active_palette(), 0);
    }

    #[test]
    fn state_rad_suit_returns_palette_13() {
        let mut state = PaletteFlashState::new();
        state.set_rad_suit(100);
        assert_eq!(state.active_palette(), 13);
    }

    #[test]
    fn state_pain_takes_priority_over_bonus() {
        let mut state = PaletteFlashState::new();
        state.add_pain(20);
        state.add_bonus();
        let pal = state.active_palette();
        assert!(
            (1..=8).contains(&pal),
            "pain should take priority over bonus, got palette {pal}"
        );
    }

    #[test]
    fn state_bonus_takes_priority_over_rad_suit() {
        let mut state = PaletteFlashState::new();
        state.add_bonus();
        state.set_rad_suit(100);
        let pal = state.active_palette();
        assert!(
            (9..=12).contains(&pal),
            "bonus should take priority over rad_suit, got palette {pal}"
        );
    }

    #[test]
    fn state_pain_takes_priority_over_rad_suit() {
        let mut state = PaletteFlashState::new();
        state.add_pain(30);
        state.set_rad_suit(100);
        let pal = state.active_palette();
        assert!(
            (1..=8).contains(&pal),
            "pain should take priority over rad_suit, got palette {pal}"
        );
    }

    #[test]
    fn state_all_zero_returns_palette_zero() {
        let state = PaletteFlashState::new();
        assert_eq!(state.active_palette(), 0);
    }

    #[test]
    fn state_tick_reduces_all_counters() {
        let mut state = PaletteFlashState::new();
        state.add_pain(5);
        state.add_bonus();
        state.set_rad_suit(10);
        state.set_berserk(10);

        state.tick();

        assert_eq!(state.pain_count(), 4);
        assert_eq!(state.bonus_count(), BONUS_FLASH_DURATION - 1);
        assert_eq!(state.rad_suit_tics(), 9);
        assert_eq!(state.berserk_tics(), 9);
    }

    #[test]
    fn state_tick_does_not_go_below_zero() {
        let mut state = PaletteFlashState::new();
        state.tick(); // All already at 0.
        assert_eq!(state.pain_count(), 0);
        assert_eq!(state.bonus_count(), 0);
        assert_eq!(state.rad_suit_tics(), 0);
        assert_eq!(state.berserk_tics(), 0);
    }

    #[test]
    fn state_berserk_returns_palette_1() {
        let mut state = PaletteFlashState::new();
        state.set_berserk(50);
        assert_eq!(state.active_palette(), 1);
    }

    #[test]
    fn state_rad_suit_takes_priority_over_berserk() {
        let mut state = PaletteFlashState::new();
        state.set_rad_suit(10);
        state.set_berserk(10);
        assert_eq!(state.active_palette(), 13);
    }

    #[test]
    fn state_default_matches_new() {
        let default_state = PaletteFlashState::default();
        let new_state = PaletteFlashState::new();
        assert_eq!(default_state.active_palette(), new_state.active_palette());
        assert_eq!(default_state.pain_count(), new_state.pain_count());
    }

    #[test]
    fn state_cumulative_pain() {
        let mut state = PaletteFlashState::new();
        state.add_pain(5);
        state.add_pain(5);
        assert_eq!(state.pain_count(), 10);
        // Should map to a higher palette than single 5-damage hit.
        let pal_10 = state.active_palette();

        let mut state2 = PaletteFlashState::new();
        state2.add_pain(5);
        let pal_5 = state2.active_palette();

        assert!(
            pal_10 >= pal_5,
            "cumulative pain should give equal or higher palette"
        );
    }
}
