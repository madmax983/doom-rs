//! Combat randomness utilities built on top of `GameState::p_random`.
//!
//! All functions here consume RNG bytes from the deterministic `DoomRng` inside
//! `GameState`, ensuring identical results across network peers and demo
//! playback.

use crate::state::GameState;

/// Doom's original 256-entry pseudo-random number table (from `m_random.c`).
///
/// The sequence is deterministic and identical on all network peers, making
/// it safe to use inside the game simulation.  `DoomRng::next_byte()` returns
/// successive bytes from this table, wrapping at index 255.
pub static RNG_TABLE: [u8; 256] = [
    0, 8, 109, 220, 222, 241, 149, 107, 75, 248, 254, 140, 16, 66, 74, 21, 211, 47, 80, 242, 154,
    27, 205, 253, 197, 224, 120, 244, 122, 173, 177, 144, 96, 255, 183, 114, 170, 72, 91, 148, 88,
    197, 243, 40, 204, 114, 237, 236, 64, 226, 104, 152, 112, 94, 237, 158, 106, 236, 168, 185,
    254, 241, 107, 208, 190, 68, 200, 60, 239, 88, 107, 55, 197, 236, 132, 150, 12, 217, 103, 177,
    166, 166, 130, 130, 172, 170, 247, 234, 60, 82, 18, 100, 250, 225, 58, 170, 26, 109, 59, 251,
    120, 125, 172, 100, 59, 181, 121, 228, 191, 130, 63, 185, 151, 189, 79, 92, 38, 30, 94, 51,
    216, 211, 165, 203, 28, 200, 216, 219, 104, 108, 175, 186, 241, 217, 45, 20, 219, 163, 43, 55,
    197, 228, 237, 51, 79, 73, 145, 181, 180, 97, 237, 232, 241, 161, 166, 174, 28, 116, 190, 174,
    16, 64, 44, 126, 161, 229, 141, 81, 89, 169, 253, 226, 116, 147, 143, 224, 11, 223, 175, 137,
    65, 68, 66, 239, 166, 196, 206, 241, 26, 189, 221, 244, 12, 201, 79, 167, 243, 246, 200, 240,
    205, 24, 113, 79, 221, 253, 2, 79, 199, 200, 40, 167, 170, 93, 69, 80, 84, 112, 210, 8, 137,
    33, 208, 185, 218, 130, 110, 91, 162, 236, 52, 249, 228, 252, 154, 232, 12, 128, 252, 183, 41,
    108, 195, 135, 183, 46, 64, 183, 231, 238, 36, 90, 201, 139, 254, 33,
];

/// Deterministic Doom RNG.
///
/// Wraps an index into `RNG_TABLE`, advancing by 1 each call (mod 256).
/// The index is part of `GameState` and is included in snapshots.
#[derive(Clone, Debug, Default)]
pub struct DoomRng {
    /// Current position in `RNG_TABLE` (always in `0..256`).
    index: u32,
}

impl DoomRng {
    /// Create a new RNG at index 0 (start of table).
    pub fn new() -> Self {
        Self { index: 0 }
    }

    /// Return the next random byte and advance the index.
    #[inline]
    pub fn next_byte(&mut self) -> u8 {
        let val = RNG_TABLE[(self.index & 255) as usize];
        self.index = (self.index + 1) & 255;
        val
    }

    /// Current table index (for snapshot / serialization).
    #[inline]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Restore to a specific index (for snapshot restore).
    #[inline]
    pub fn set_index(&mut self, index: u32) {
        self.index = index & 255;
    }
}

// ---------------------------------------------------------------------------
// Damage variance
// ---------------------------------------------------------------------------

/// Apply Doom's standard damage multiplier to a base damage value.
///
/// Formula: `base_damage * ((p_random() % 8) + 1)`.
/// The result is in `[base_damage * 1, base_damage * 8]`.
///
/// Port of the inline `damage *= ...` patterns found throughout `p_map.c`,
/// `p_inter.c`, and weapon code.
pub fn p_damage_with_variance(gs: &mut GameState, base_damage: i32) -> i32 {
    if base_damage == 0 {
        return 0;
    }
    let multiplier = (gs.p_random() as i32 % 8) + 1;
    base_damage * multiplier
}

// ---------------------------------------------------------------------------
// Random tic offset
// ---------------------------------------------------------------------------

/// Add a random 0-3 tic offset to `base_tics`.
///
/// Used by monster see/chase states to desynchronize animations of monsters
/// that spawn at the same time.  Not applied to attack/pain/death states.
///
/// Port of the `P_Random()&3` patterns in `P_SetMobjState`.
pub fn randomize_tics(gs: &mut GameState, base_tics: i32) -> i32 {
    base_tics + (gs.p_random() as i32 & 3)
}

// ---------------------------------------------------------------------------
// Random chance check
// ---------------------------------------------------------------------------

/// Return `true` if `p_random() < threshold`.
///
/// Threshold 0 always returns `false`; threshold 255 returns `true` for all
/// table entries except the single entry with value 255 (which returns
/// `false` for threshold 255).
///
/// Used for pain chance, dodge chance, monster infighting probability, etc.
pub fn p_random_chance(gs: &mut GameState, threshold: u8) -> bool {
    gs.p_random() < threshold
}

// ---------------------------------------------------------------------------
// Missile angle spread
// ---------------------------------------------------------------------------

/// Compute a random angle spread for inaccurate monster projectiles.
///
/// Returns `p_subrandom() << 20` — a BAM-compatible angle delta suitable for
/// adding to a monster's aim angle to simulate weapon inaccuracy.
///
/// Used by zombiemen, shotgun guys, and other hitscan monsters.
pub fn p_missile_angle_spread(gs: &mut GameState) -> i32 {
    gs.p_subrandom() << 20
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;

    // -----------------------------------------------------------------------
    // p_random basics
    // -----------------------------------------------------------------------

    #[test]
    fn p_random_returns_first_table_entry() {
        let mut gs = GameState::new("test");
        let val = gs.p_random();
        assert_eq!(val, RNG_TABLE[0], "first p_random must return RNG_TABLE[0]");
    }

    #[test]
    fn p_random_advances_rng_index() {
        let mut gs = GameState::new("test");
        gs.p_random();
        assert_eq!(
            gs.rng.index(),
            1,
            "rng index must be 1 after one p_random call"
        );
    }

    #[test]
    fn p_random_wraps_at_256() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            gs.p_random();
        }
        assert_eq!(
            gs.rng.index(),
            0,
            "rng index must wrap to 0 after 256 p_random calls"
        );
        // Next call must return RNG_TABLE[0] again.
        let val = gs.p_random();
        assert_eq!(val, RNG_TABLE[0]);
    }

    #[test]
    fn p_random_is_deterministic() {
        let mut gs1 = GameState::new("test");
        let mut gs2 = GameState::new("test");
        let seq1: Vec<u8> = (0..50).map(|_| gs1.p_random()).collect();
        let seq2: Vec<u8> = (0..50).map(|_| gs2.p_random()).collect();
        assert_eq!(
            seq1, seq2,
            "two fresh GameStates must produce identical sequences"
        );
    }

    #[test]
    fn multiple_p_random_calls_produce_different_values() {
        let mut gs = GameState::new("test");
        let a = gs.p_random();
        let b = gs.p_random();
        // RNG_TABLE[0]=0, RNG_TABLE[1]=8 — they differ.
        assert_ne!(
            a, b,
            "consecutive p_random calls should produce different values"
        );
    }

    // -----------------------------------------------------------------------
    // p_random_range
    // -----------------------------------------------------------------------

    #[test]
    fn p_random_range_returns_value_within_bounds() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let val = gs.p_random_range(10, 20);
            assert!(val >= 10, "p_random_range value {val} must be >= 10");
            assert!(val <= 20, "p_random_range value {val} must be <= 20");
        }
    }

    #[test]
    fn p_random_range_min_equals_max_returns_that_value() {
        let mut gs = GameState::new("test");
        for _ in 0..10 {
            let val = gs.p_random_range(42, 42);
            assert_eq!(val, 42, "min==max must always return that value");
        }
    }

    // -----------------------------------------------------------------------
    // p_subrandom
    // -----------------------------------------------------------------------

    #[test]
    fn p_subrandom_range_is_minus255_to_255() {
        let mut gs = GameState::new("test");
        for _ in 0..512 {
            let val = gs.p_subrandom();
            assert!(
                (-255..=255).contains(&val),
                "p_subrandom value {val} must be in [-255, 255]"
            );
        }
    }

    #[test]
    fn p_subrandom_is_symmetric_around_zero_on_average() {
        let mut gs = GameState::new("test");
        let sum: i64 = (0..1024).map(|_| gs.p_subrandom() as i64).sum();
        // With 1024 samples, the absolute average should be reasonably small.
        // We allow generous bounds since the table is small and will cycle.
        let avg = sum.abs() as f64 / 1024.0;
        assert!(
            avg < 50.0,
            "p_subrandom average magnitude {avg} should be small, indicating symmetry"
        );
    }

    // -----------------------------------------------------------------------
    // p_damage_with_variance
    // -----------------------------------------------------------------------

    #[test]
    fn damage_variance_applies_multiplier_1_to_8() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let dmg = p_damage_with_variance(&mut gs, 10);
            assert!(dmg >= 10, "damage {dmg} must be >= base 10");
            assert!(dmg <= 80, "damage {dmg} must be <= base*8 = 80");
            assert_eq!(dmg % 10, 0, "damage {dmg} must be a multiple of base 10");
        }
    }

    #[test]
    fn damage_variance_with_base_zero_returns_zero() {
        let mut gs = GameState::new("test");
        let dmg = p_damage_with_variance(&mut gs, 0);
        assert_eq!(dmg, 0, "base 0 must return 0");
    }

    #[test]
    fn damage_variance_with_base_10_returns_10_to_80() {
        let mut gs = GameState::new("test");
        let mut min_seen = i32::MAX;
        let mut max_seen = i32::MIN;
        for _ in 0..256 {
            let dmg = p_damage_with_variance(&mut gs, 10);
            min_seen = min_seen.min(dmg);
            max_seen = max_seen.max(dmg);
        }
        assert!(min_seen >= 10, "min damage {min_seen} must be >= 10");
        assert!(max_seen <= 80, "max damage {max_seen} must be <= 80");
    }

    // -----------------------------------------------------------------------
    // Pain chance (tested via damage_mobj — see combat.rs tests)
    // We test the core p_random_chance helper here.
    // -----------------------------------------------------------------------

    #[test]
    fn p_random_chance_threshold_0_returns_false() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            assert!(
                !p_random_chance(&mut gs, 0),
                "threshold 0 must always return false"
            );
        }
    }

    #[test]
    fn p_random_chance_threshold_255_returns_true_for_most_values() {
        let mut gs = GameState::new("test");
        let true_count: usize = (0..256).filter(|_| p_random_chance(&mut gs, 255)).count();
        // Only entries with value 255 in the table return false.
        // There is 1 entry with value 255 in the table (index 33).
        assert!(
            true_count >= 250,
            "threshold 255 should return true for most values, got {true_count}/256"
        );
    }

    // -----------------------------------------------------------------------
    // randomize_tics
    // -----------------------------------------------------------------------

    #[test]
    fn randomize_tics_adds_0_to_3_to_base() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let result = randomize_tics(&mut gs, 10);
            assert!(result >= 10, "result {result} must be >= base 10");
            assert!(result <= 13, "result {result} must be <= base+3 = 13");
        }
    }

    #[test]
    fn randomize_tics_with_base_0_returns_0_to_3() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let result = randomize_tics(&mut gs, 0);
            assert!(result >= 0, "result {result} must be >= 0");
            assert!(result <= 3, "result {result} must be <= 3");
        }
    }

    // -----------------------------------------------------------------------
    // p_missile_angle_spread
    // -----------------------------------------------------------------------

    #[test]
    fn missile_angle_spread_returns_bam_compatible_values() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let spread = p_missile_angle_spread(&mut gs);
            // p_subrandom is [-255, 255], shifted left 20 bits.
            let max_mag = 255i32 << 20;
            assert!(
                spread.abs() <= max_mag,
                "spread {spread} magnitude must be <= {max_mag}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // RNG determinism across save/load
    // -----------------------------------------------------------------------

    #[test]
    fn rng_state_is_deterministic_across_save_load() {
        let mut gs = GameState::new("test");
        // Consume some random bytes.
        for _ in 0..77 {
            gs.p_random();
        }
        // Save the RNG index.
        let saved_index = gs.rng.index();
        // Generate a sequence.
        let seq1: Vec<u8> = (0..20).map(|_| gs.p_random()).collect();

        // Simulate restore: create fresh state, set index to saved value.
        let mut gs2 = GameState::new("test");
        gs2.rng.set_index(saved_index);
        let seq2: Vec<u8> = (0..20).map(|_| gs2.p_random()).collect();

        assert_eq!(
            seq1, seq2,
            "RNG must replay identically after restoring saved index"
        );
    }
}
