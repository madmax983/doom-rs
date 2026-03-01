//! Weapon firing — maps WeaponType to attack parameters and fires.
//!
//! Port of Doom's `p_pspr.c` (simplified — no weapon bob, no raise/lower
//! animations, no flash states).
//!
//! Each weapon dispatches to either `p_line_attack` (hitscan) or
//! `p_radius_attack` (splash) from `combat.rs`.

use doom_map::Level;
use doom_types::{Bam, Fixed16_16};

use crate::combat::{MISSILERANGE, p_line_attack, p_radius_attack};
use crate::mobj::MobjHandle;
use crate::player::AmmoType;
use crate::state::GameState;

// ---------------------------------------------------------------------------
// Weapon stat table
// ---------------------------------------------------------------------------

/// Per-weapon static firing parameters.
#[allow(dead_code)]
struct WeaponInfo {
    /// Ammo type consumed, or `None` for melee weapons (Fist, Chainsaw).
    ammo_type: Option<AmmoType>,
    /// Units of ammo consumed per firing event.
    ammo_use: u32,
    /// Minimum damage roll (inclusive).
    damage_lo: i32,
    /// Maximum damage roll (inclusive).
    damage_hi: i32,
    /// Number of hitscan rays per shot (0 = radius attack, e.g. rocket).
    pellets: u8,
    /// Maximum hitscan range in fixed-point map units.
    range: Fixed16_16,
    /// Angular spread between pellets in BAM units (0 = no spread).
    spread: u32,
    /// True if weapon is melee (Fist, Chainsaw).
    is_melee: bool,
}

/// Weapon info table indexed by `WeaponType as usize`.
///
/// Entries correspond to: Fist(0), Pistol(1), Shotgun(2), Chaingun(3),
/// RocketLauncher(4), PlasmaRifle(5), Bfg(6), Chainsaw(7), SuperShotgun(8).
static WEAPON_INFO: [WeaponInfo; 9] = [
    // 0 — Fist
    WeaponInfo {
        ammo_type: None,
        ammo_use:  0,
        damage_lo: 10,
        damage_hi: 110,
        pellets:   1,
        range:     Fixed16_16(64 << 16),
        spread:    0,
        is_melee:  true,
    },
    // 1 — Pistol
    WeaponInfo {
        ammo_type: Some(AmmoType::Bullets),
        ammo_use:  1,
        damage_lo: 5,
        damage_hi: 15,
        pellets:   1,
        range:     MISSILERANGE,
        spread:    0,
        is_melee:  false,
    },
    // 2 — Shotgun
    WeaponInfo {
        ammo_type: Some(AmmoType::Shells),
        ammo_use:  1,
        damage_lo: 5,
        damage_hi: 15,
        pellets:   7,
        range:     MISSILERANGE,
        spread:    0x1400_0000, // ≈ 5.6° per pellet in BAM
        is_melee:  false,
    },
    // 3 — Chaingun
    WeaponInfo {
        ammo_type: Some(AmmoType::Bullets),
        ammo_use:  1,
        damage_lo: 5,
        damage_hi: 15,
        pellets:   1,
        range:     MISSILERANGE,
        spread:    0,
        is_melee:  false,
    },
    // 4 — RocketLauncher (pellets=0 → radius attack)
    WeaponInfo {
        ammo_type: Some(AmmoType::Rockets),
        ammo_use:  1,
        damage_lo: 80,
        damage_hi: 160,
        pellets:   0,
        range:     Fixed16_16::ZERO, // unused — radius attack
        spread:    0,
        is_melee:  false,
    },
    // 5 — PlasmaRifle
    WeaponInfo {
        ammo_type: Some(AmmoType::Cells),
        ammo_use:  1,
        damage_lo: 5,
        damage_hi: 40,
        pellets:   1,
        range:     MISSILERANGE,
        spread:    0,
        is_melee:  false,
    },
    // 6 — BFG 9000
    WeaponInfo {
        ammo_type: Some(AmmoType::Cells),
        ammo_use:  40,
        damage_lo: 100,
        damage_hi: 800,
        pellets:   1,
        range:     MISSILERANGE,
        spread:    0,
        is_melee:  false,
    },
    // 7 — Chainsaw
    WeaponInfo {
        ammo_type: None,
        ammo_use:  0,
        damage_lo: 10,
        damage_hi: 110,
        pellets:   1,
        range:     Fixed16_16(64 << 16),
        spread:    0,
        is_melee:  true,
    },
    // 8 — SuperShotgun
    WeaponInfo {
        ammo_type: Some(AmmoType::Shells),
        ammo_use:  2,
        damage_lo: 5,
        damage_hi: 15,
        pellets:   20,
        range:     MISSILERANGE,
        spread:    0x1400_0000, // same spread as regular shotgun
        is_melee:  false,
    },
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fire the player's current weapon once.
///
/// Called when `bt::BT_ATTACK` is set in a `TicCmd` and the player is alive.
///
/// # Ammo check
/// If the current weapon requires ammo and the player has insufficient ammo,
/// this function returns immediately without firing or consuming ammo.
///
/// # Rocket Launcher
/// When `pellets == 0`, fires `p_radius_attack` with a fixed blast radius of
/// 128 map units and `mid_damage = (damage_lo + damage_hi) / 2`.
///
/// # Hitscan weapons
/// Fires `pellets` separate rays via `p_line_attack`.  Each pellet's angle is
/// spread evenly around the actor's facing direction.
pub fn fire_weapon(gs: &mut GameState, level: Option<&Level>, handle: MobjHandle) {
    let weapon = gs.player.weapon;
    let info = &WEAPON_INFO[weapon as usize];

    // --- Ammo check ---
    if let Some(ammo_type) = info.ammo_type {
        let available = gs.player.ammo(ammo_type as usize);
        if available < info.ammo_use {
            return;
        }
        // We already verified available >= ammo_use above; this cannot fail.
        gs.player.use_ammo(ammo_type as usize, info.ammo_use);
    }

    // --- Gather source angle (copy out before mutable borrows) ---
    let base_angle: Bam = match gs.mobjslab.get(handle) {
        Some(mo) => mo.angle,
        None => return,
    };

    let pellets = info.pellets;
    let spread  = info.spread;
    let range   = info.range;

    // Deterministic pseudo-random damage using tic_num.
    let tic = gs.tic_num;
    let damage_range = (info.damage_hi - info.damage_lo + 1) as u32;
    let damage_lo    = info.damage_lo;

    // --- Rocket Launcher: radius attack ---
    if pellets == 0 {
        let mid_damage = (damage_lo + info.damage_hi) / 2;
        p_radius_attack(gs, handle, mid_damage, Fixed16_16(128 << 16), level);
        return;
    }

    // --- Hitscan: fire each pellet ---
    for i in 0..pellets {
        // Compute per-pellet angle.
        // For single-pellet weapons spread=0, so this is just base_angle.
        // For multi-pellet (shotgun): center the spread around base_angle.
        //   offset = spread * i - spread * (pellets - 1) / 2
        let shot_angle: Bam = if pellets == 1 || spread == 0 {
            base_angle
        } else {
            // Center the spread: pellet i fires at base_angle + spread*(i) - spread*(pellets-1)/2
            let positive_offset = spread.wrapping_mul(i as u32);
            let center_offset   = spread.wrapping_mul((pellets as u32).wrapping_sub(1)) / 2;
            Bam(base_angle
                .0
                .wrapping_add(positive_offset)
                .wrapping_sub(center_offset))
        };

        // Deterministic damage: vary by pellet index and tic_num.
        let damage = damage_lo + ((tic.wrapping_add(i as u32)) % damage_range) as i32;

        p_line_attack(gs, handle, shot_angle, range, damage, level);
    }
}

/// Returns `true` if the player's current weapon has sufficient ammo to fire.
///
/// Always returns `true` for melee weapons (Fist, Chainsaw) regardless of
/// ammo counts.
pub fn player_can_fire(gs: &GameState) -> bool {
    let weapon = gs.player.weapon;
    let info = &WEAPON_INFO[weapon as usize];
    match info.ammo_type {
        None => true,
        Some(ammo_type) => gs.player.ammo(ammo_type as usize) >= info.ammo_use,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind, flags};
    use crate::player::{PlayerState, WeaponType};
    use crate::state::GameState;
    use doom_types::{Bam, Fixed16_16};

    /// Build a minimal GameState with a live player Mobj at the origin.
    ///
    /// Mirrors the pattern used in `actions.rs` tests.
    fn make_game_state() -> GameState {
        let mut gs = GameState::new("test");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags  = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let handle = gs.mobjslab.alloc(mo);
        gs.player  = PlayerState::pistol_start(handle);
        gs
    }

    // -----------------------------------------------------------------------
    // Test 1: pistol consumes one bullet per shot
    // -----------------------------------------------------------------------

    #[test]
    fn pistol_consumes_one_clip_ammo() {
        let mut gs = make_game_state();
        // pistol_start gives 50 bullets; weapon defaults to Pistol.
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(
            after,
            before - 1,
            "pistol must consume exactly 1 bullet per shot"
        );
    }

    // -----------------------------------------------------------------------
    // Test 2: fist never consumes any ammo
    // -----------------------------------------------------------------------

    #[test]
    fn fist_never_consumes_ammo() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Fist;
        let before_bullets = gs.player.ammo(AmmoType::Bullets as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after_bullets = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(
            after_bullets, before_bullets,
            "fist must not consume any ammo"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: player_can_fire returns false when clip is empty
    // -----------------------------------------------------------------------

    #[test]
    fn player_can_fire_returns_false_when_no_ammo() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Pistol;
        // Drain all bullets by firing until empty, or use use_ammo for the known count.
        // pistol_start gives 50 bullets; drain exactly 50.
        let drained = gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        assert!(drained, "pre-condition: must drain 50 bullets successfully");
        assert_eq!(
            gs.player.ammo(AmmoType::Bullets as usize),
            0,
            "pre-condition: bullets must be 0"
        );
        assert!(
            !player_can_fire(&gs),
            "player_can_fire must return false with 0 bullets and Pistol"
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: player_can_fire always returns true for fist
    // -----------------------------------------------------------------------

    #[test]
    fn player_can_fire_returns_true_for_fist_always() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Fist;
        // Drain the bullets the pistol-start gives (50); ignore pools that are already 0.
        let _ = gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        assert!(
            player_can_fire(&gs),
            "fist must always be fireable regardless of ammo"
        );
    }

    // -----------------------------------------------------------------------
    // Test 5: shotgun consumes one shell per shot
    // -----------------------------------------------------------------------

    #[test]
    fn shotgun_consumes_one_shell() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Shotgun;
        // Give the player some shells.
        gs.player.give_ammo(AmmoType::Shells as usize, 10);
        let before = gs.player.ammo(AmmoType::Shells as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Shells as usize);
        assert_eq!(
            after,
            before - 1,
            "shotgun must consume exactly 1 shell per shot"
        );
    }

    // -----------------------------------------------------------------------
    // Test 6: BFG consumes 40 cells per shot
    // -----------------------------------------------------------------------

    #[test]
    fn bfg_consumes_40_cells() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Bfg;
        // Give enough cells to fire the BFG.
        gs.player.give_ammo(AmmoType::Cells as usize, 300);
        let before = gs.player.ammo(AmmoType::Cells as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Cells as usize);
        assert_eq!(
            after,
            before - 40,
            "BFG must consume exactly 40 cells per shot"
        );
    }

    // -----------------------------------------------------------------------
    // Additional: no-fire when insufficient ammo (ammo check)
    // -----------------------------------------------------------------------

    #[test]
    fn no_fire_when_insufficient_ammo() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Bfg;
        // Only 5 cells — not enough for BFG (needs 40).
        gs.player.give_ammo(AmmoType::Cells as usize, 5);
        let before = gs.player.ammo(AmmoType::Cells as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Cells as usize);
        assert_eq!(after, before, "no ammo must be consumed when insufficient");
    }

    // -----------------------------------------------------------------------
    // Additional: chainsaw never consumes ammo
    // -----------------------------------------------------------------------

    #[test]
    fn chainsaw_never_consumes_ammo() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Chainsaw;
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(after, before, "chainsaw must not consume any ammo");
    }

    // -----------------------------------------------------------------------
    // Additional: player_can_fire with chainsaw always true
    // -----------------------------------------------------------------------

    #[test]
    fn player_can_fire_chainsaw_always_true() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Chainsaw;
        let _ = gs.player.use_ammo(AmmoType::Bullets as usize, 200);
        assert!(
            player_can_fire(&gs),
            "chainsaw must always be fireable"
        );
    }
}
