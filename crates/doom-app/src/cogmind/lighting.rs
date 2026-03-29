//! Lighting module for cogmind-mode rendering.
//!
//! Pure functions for sector flicker, player radial light, hazard glow blending,
//! and entity light boosting. All functions are deterministic and side-effect-free.

use super::glyphs::Rgb;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum radius (in tile cells) for the player's radial light bonus.
pub const PLAYER_LIGHT_RADIUS: i32 = 8;

/// Light level added per cell of proximity within the radial light radius.
pub const LIGHT_PER_CELL: i32 = 15;

/// Sector special values that produce flickering light effects.
pub const FLICKER_SPECIALS: [u16; 6] = [1, 2, 3, 12, 13, 17];

// ---------------------------------------------------------------------------
// Cosmetic PRNG
// ---------------------------------------------------------------------------

/// Xorshift32 cosmetic PRNG. NOT for gameplay determinism -- only for visual
/// effects like flicker offsets that don't affect game state.
#[must_use]
pub fn cosm_rand(state: u32) -> u32 {
    let mut s = state;
    // Ensure non-zero input (xorshift32 has a zero fixed point).
    if s == 0 {
        s = 1;
    }
    s ^= s << 13;
    s ^= s >> 17;
    s ^= s << 5;
    s
}

// ---------------------------------------------------------------------------
// Flicker offset
// ---------------------------------------------------------------------------

/// Compute a brightness delta for flickering sectors.
///
/// Returns 0 for non-flicker sector specials. For flicker specials (1, 2, 3,
/// 12, 13, 17), returns a value in the range `[-40, 40]` seeded from
/// `(sector_idx * 2654435761) ^ tic` so that each sector flickers
/// independently.
#[must_use]
pub fn flicker_offset(sector_special: u16, sector_idx: usize, tic: u32) -> i16 {
    if !FLICKER_SPECIALS.contains(&sector_special) {
        return 0;
    }

    // Knuth multiplicative hash on sector_idx, XOR with tic.
    let seed = (sector_idx as u32).wrapping_mul(2_654_435_761) ^ tic;
    let rand_val = cosm_rand(seed);

    // Map to [-40, 40]: take low bits mod 81, then shift down by 40.
    ((rand_val % 81) as i16) - 40
}

// ---------------------------------------------------------------------------
// Player radial bonus
// ---------------------------------------------------------------------------

/// Compute a light bonus based on Manhattan distance from the player.
///
/// Tiles at the player's position get `PLAYER_LIGHT_RADIUS * LIGHT_PER_CELL`
/// bonus. Each cell further away reduces the bonus by `LIGHT_PER_CELL`.
/// Tiles beyond `PLAYER_LIGHT_RADIUS` get 0.
#[must_use]
pub fn player_radial_bonus(player_tx: i32, player_ty: i32, tile_tx: i32, tile_ty: i32) -> i32 {
    let dist = (player_tx - tile_tx).abs() + (player_ty - tile_ty).abs();
    if dist > PLAYER_LIGHT_RADIUS {
        return 0;
    }
    (PLAYER_LIGHT_RADIUS - dist) * LIGHT_PER_CELL
}

// ---------------------------------------------------------------------------
// Hazard glow blend
// ---------------------------------------------------------------------------

/// Blend a base color with a hazard glow: 80% base + 20% glow.
#[must_use]
pub fn blend_hazard_glow(base: Rgb, glow: Rgb) -> Rgb {
    let blend = |b: u8, g: u8| -> u8 {
        let val = (u16::from(b) * 4 + u16::from(g)) / 5;
        val as u8
    };
    (
        blend(base.0, glow.0),
        blend(base.1, glow.1),
        blend(base.2, glow.2),
    )
}

// ---------------------------------------------------------------------------
// Effective light
// ---------------------------------------------------------------------------

/// Compute the final light level for a tile, stacking sector base light,
/// flicker offset, and player radial bonus. Clamps result to `[0, 255]`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn effective_light(
    sector_light: u8,
    sector_special: u16,
    sector_idx: usize,
    tic: u32,
    player_tx: i32,
    player_ty: i32,
    tile_tx: i32,
    tile_ty: i32,
) -> u8 {
    let base = i32::from(sector_light);
    let flicker = i32::from(flicker_offset(sector_special, sector_idx, tic));
    let radial = player_radial_bonus(player_tx, player_ty, tile_tx, tile_ty);

    let total = base + flicker + radial;
    total.clamp(0, 255) as u8
}

// ---------------------------------------------------------------------------
// Entity light boost
// ---------------------------------------------------------------------------

/// Boost light level for entities in dark sectors so they remain visible.
///
/// If `light < 80`, bumps the value to `min(light + 60, 120)`.
#[must_use]
pub fn entity_light_boost(light: u8) -> u8 {
    if light < 80 {
        (light as u16 + 60).min(120) as u8
    } else {
        light
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- cosm_rand --

    #[test]
    fn cosm_rand_deterministic() {
        let a = cosm_rand(42);
        let b = cosm_rand(42);
        assert_eq!(a, b, "same seed must produce same result");
    }

    #[test]
    fn cosm_rand_different_seeds_differ() {
        let a = cosm_rand(1);
        let b = cosm_rand(2);
        assert_ne!(a, b, "different seeds should produce different results");
    }

    #[test]
    fn cosm_rand_zero_seed_works() {
        // Zero is a fixed point for raw xorshift; our impl guards against it.
        let val = cosm_rand(0);
        assert_ne!(val, 0, "zero seed should produce nonzero output");
    }

    #[test]
    fn cosm_rand_nonzero_output() {
        // Xorshift32 should never produce 0 from a nonzero seed.
        for seed in [1, 100, 0xDEAD_BEEF, u32::MAX] {
            assert_ne!(cosm_rand(seed), 0);
        }
    }

    // -- flicker_offset --

    #[test]
    fn flicker_offset_zero_for_non_flicker() {
        // Special 0 (normal sector) should never flicker.
        for tic in 0..100 {
            assert_eq!(flicker_offset(0, 5, tic), 0);
        }
        // Special 4 (lava damage, not flicker).
        assert_eq!(flicker_offset(4, 0, 0), 0);
        assert_eq!(flicker_offset(99, 0, 0), 0);
    }

    #[test]
    fn flicker_offset_nonzero_for_flicker_specials() {
        // Over many tics, at least some should be nonzero.
        for &special in &FLICKER_SPECIALS {
            let any_nonzero = (0..200).any(|tic| flicker_offset(special, 7, tic) != 0);
            assert!(
                any_nonzero,
                "special {special} should produce nonzero flicker at some tic"
            );
        }
    }

    #[test]
    fn flicker_offset_stays_in_range() {
        for &special in &FLICKER_SPECIALS {
            for tic in 0..500 {
                let offset = flicker_offset(special, 42, tic);
                assert!(
                    (-40..=40).contains(&offset),
                    "offset {offset} out of range for special={special} tic={tic}"
                );
            }
        }
    }

    #[test]
    fn flicker_sectors_vary_independently() {
        // Two different sector indices at the same tic should generally differ.
        let mut differ_count = 0;
        for tic in 0..100 {
            let a = flicker_offset(1, 0, tic);
            let b = flicker_offset(1, 1, tic);
            if a != b {
                differ_count += 1;
            }
        }
        assert!(
            differ_count > 50,
            "sectors should flicker independently (differed {differ_count}/100 tics)"
        );
    }

    // -- player_radial_bonus --

    #[test]
    fn radial_bonus_at_player_position() {
        let bonus = player_radial_bonus(5, 5, 5, 5);
        assert_eq!(bonus, PLAYER_LIGHT_RADIUS * LIGHT_PER_CELL);
    }

    #[test]
    fn radial_bonus_adjacent() {
        let bonus = player_radial_bonus(5, 5, 6, 5); // dist = 1
        assert_eq!(bonus, (PLAYER_LIGHT_RADIUS - 1) * LIGHT_PER_CELL);
    }

    #[test]
    fn radial_bonus_at_edge() {
        // Manhattan distance exactly PLAYER_LIGHT_RADIUS → bonus = 0.
        let bonus = player_radial_bonus(0, 0, PLAYER_LIGHT_RADIUS, 0);
        assert_eq!(bonus, 0);
    }

    #[test]
    fn radial_bonus_beyond_radius() {
        let bonus = player_radial_bonus(0, 0, PLAYER_LIGHT_RADIUS + 1, 0);
        assert_eq!(bonus, 0);
    }

    #[test]
    fn radial_bonus_diagonal() {
        // (4,4) from player at (0,0) → Manhattan dist = 8 = PLAYER_LIGHT_RADIUS → 0.
        let bonus = player_radial_bonus(0, 0, 4, 4);
        assert_eq!(bonus, 0);

        // (3,4) → dist 7 → bonus = 1 * 15 = 15.
        let bonus2 = player_radial_bonus(0, 0, 3, 4);
        assert_eq!(bonus2, LIGHT_PER_CELL);
    }

    #[test]
    fn radial_bonus_negative_coords() {
        // Works with negative coordinates.
        let bonus = player_radial_bonus(-5, -5, -5, -5);
        assert_eq!(bonus, PLAYER_LIGHT_RADIUS * LIGHT_PER_CELL);
    }

    // -- blend_hazard_glow --

    #[test]
    fn blend_hazard_glow_math() {
        // 80% of (100, 200, 50) + 20% of (255, 0, 100)
        // R: (100*4 + 255) / 5 = 655/5 = 131
        // G: (200*4 + 0) / 5 = 800/5 = 160
        // B: (50*4 + 100) / 5 = 300/5 = 60
        let result = blend_hazard_glow((100, 200, 50), (255, 0, 100));
        assert_eq!(result, (131, 160, 60));
    }

    #[test]
    fn blend_hazard_glow_same_color() {
        let result = blend_hazard_glow((120, 120, 120), (120, 120, 120));
        assert_eq!(result, (120, 120, 120));
    }

    #[test]
    fn blend_hazard_glow_black_base() {
        // 80% black + 20% green = (0, 51, 0)
        let result = blend_hazard_glow((0, 0, 0), (0, 255, 0));
        assert_eq!(result, (0, 51, 0));
    }

    #[test]
    fn blend_hazard_glow_black_glow() {
        // 80% white + 20% black = (204, 204, 204)
        let result = blend_hazard_glow((255, 255, 255), (0, 0, 0));
        assert_eq!(result, (204, 204, 204));
    }

    // -- effective_light --

    #[test]
    fn effective_light_basic_no_flicker() {
        // Non-flicker sector, player far away.
        let light = effective_light(128, 0, 0, 0, 100, 100, 0, 0);
        assert_eq!(light, 128);
    }

    #[test]
    fn effective_light_clamped_high() {
        // High sector light + player at same position → could exceed 255.
        let light = effective_light(255, 0, 0, 0, 5, 5, 5, 5);
        assert_eq!(light, 255);
    }

    #[test]
    fn effective_light_clamped_low() {
        // Very dark sector with negative flicker → should clamp to 0.
        // Use a flicker special to get potential negative offset.
        // We test many tics: at least one should produce 0 if sector_light is low.
        let any_zero = (0..500).any(|tic| effective_light(5, 1, 42, tic, 100, 100, 0, 0) == 0);
        // With sector_light=5, flicker can go to -40, so 5 + (-40) = -35 → 0.
        assert!(
            any_zero,
            "dark sector + flicker should clamp to 0 sometimes"
        );
    }

    #[test]
    fn effective_light_player_proximity_adds() {
        // Player adjacent to tile in non-flicker sector.
        let far = effective_light(100, 0, 0, 0, 50, 50, 0, 0);
        let near = effective_light(100, 0, 0, 0, 5, 5, 6, 5);
        assert!(near > far, "proximity should add light");
        assert_eq!(
            near,
            100 + ((PLAYER_LIGHT_RADIUS - 1) * LIGHT_PER_CELL) as u8
        );
    }

    // -- entity_light_boost --

    #[test]
    fn entity_light_boost_dark_sector() {
        // light=20 < 80 → 20+60=80, min(80,120)=80
        assert_eq!(entity_light_boost(20), 80);
    }

    #[test]
    fn entity_light_boost_very_dark() {
        // light=0 → 0+60=60, min(60,120)=60
        assert_eq!(entity_light_boost(0), 60);
    }

    #[test]
    fn entity_light_boost_near_threshold() {
        // light=79 → 79+60=139, min(139,120)=120
        assert_eq!(entity_light_boost(79), 120);
    }

    #[test]
    fn entity_light_boost_at_threshold() {
        // light=80 → no boost
        assert_eq!(entity_light_boost(80), 80);
    }

    #[test]
    fn entity_light_boost_bright_sector() {
        // light=200 → no boost
        assert_eq!(entity_light_boost(200), 200);
    }

    #[test]
    fn entity_light_boost_max_light() {
        assert_eq!(entity_light_boost(255), 255);
    }
}
