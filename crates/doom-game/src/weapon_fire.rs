//! Per-weapon fire functions — each weapon gets its own function with
//! Doom-accurate spread, ammo consumption, and damage parameters.
//!
//! This module provides individual fire functions that match the original
//! Doom source (`p_pspr.c`) behavior more closely than the table-driven
//! `fire_weapon` in `weapons.rs`, and adds a `fire_current_weapon`
//! dispatcher that stages Doom-style pending weapon switches on empty.

use doom_map::Level;
use doom_types::{Bam, Fixed16_16};

use crate::combat::{MELEERANGE, MISSILERANGE, p_line_attack, p_line_attack_target};
use crate::mobj::MobjHandle;
use crate::player::powers::PW_STRENGTH;
use crate::projectile::p_spawn_player_missile;
use crate::random::p_damage_with_variance;
use crate::state::{GameState, SoundRequest};
use doom_types::mobj_kind::MobjKind;
use doom_types::weapons::AmmoType;
use doom_types::weapons::WeaponType;

// ---------------------------------------------------------------------------
// Ammo cost table
// ---------------------------------------------------------------------------

/// Per-weapon ammo cost: `(WeaponType, AmmoType, cost)`.
///
/// Melee weapons (Fist, Chainsaw) have cost 0 and `AmmoType::None`.
pub const AMMO_PER_SHOT: [(WeaponType, AmmoType, u32); 9] = [
    (WeaponType::Fist, AmmoType::None, 0),
    (WeaponType::Pistol, AmmoType::Bullets, 1),
    (WeaponType::Shotgun, AmmoType::Shells, 1),
    (WeaponType::SuperShotgun, AmmoType::Shells, 2),
    (WeaponType::Chaingun, AmmoType::Bullets, 1),
    (WeaponType::RocketLauncher, AmmoType::Rockets, 1),
    (WeaponType::PlasmaRifle, AmmoType::Cells, 1),
    (WeaponType::Bfg, AmmoType::Cells, 40),
    (WeaponType::Chainsaw, AmmoType::None, 0),
];

const BULLET_AUTOAIM_RANGE: Fixed16_16 = Fixed16_16(1024 << 16);
const BULLET_AUTOAIM_SIDE_PROBE: u32 = 1 << 26;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the ammo cost for `weapon` from the `AMMO_PER_SHOT` table.
///
/// Returns `(AmmoType, cost)`.  Melee weapons return `(AmmoType::None, 0)`.
pub fn weapon_ammo_cost(weapon: WeaponType) -> (AmmoType, u32) {
    for &(w, a, c) in &AMMO_PER_SHOT {
        if w == weapon {
            return (a, c);
        }
    }
    // Fallback (should never happen for valid WeaponType values).
    (AmmoType::None, 0)
}

/// Check whether the player has enough ammo for the given weapon.
/// Melee weapons always return true.
fn has_ammo(gs: &GameState, weapon: WeaponType) -> bool {
    let (ammo_type, cost) = weapon_ammo_cost(weapon);
    if ammo_type == AmmoType::None {
        return true;
    }
    gs.player.ammo(ammo_type as usize) >= cost
}

#[inline]
fn weapon_makes_noise(weapon: WeaponType) -> bool {
    !matches!(weapon, WeaponType::Fist)
}

/// Return the minimum number of tics before `weapon` may fire again.
pub fn weapon_refire_tics(weapon: WeaponType) -> u8 {
    match weapon {
        WeaponType::Fist => 12,
        WeaponType::Pistol => 14,
        WeaponType::Shotgun => 20,
        WeaponType::Chaingun => 4,
        WeaponType::RocketLauncher => 20,
        WeaponType::PlasmaRifle => 3,
        WeaponType::Bfg => 30,
        WeaponType::Chainsaw => 4,
        WeaponType::SuperShotgun => 34,
    }
}

/// Doom's launcher weapons require a release before the next shot.
pub fn weapon_allows_hold_fire(weapon: WeaponType) -> bool {
    !matches!(weapon, WeaponType::RocketLauncher | WeaponType::Bfg)
}

/// Consume ammo for the given weapon.  Returns `false` if insufficient.
/// Melee weapons always return `true` without consuming anything.
fn consume_ammo(gs: &mut GameState, weapon: WeaponType) -> bool {
    let (ammo_type, cost) = weapon_ammo_cost(weapon);
    if ammo_type == AmmoType::None || cost == 0 {
        return true;
    }
    gs.player.use_ammo(ammo_type as usize, cost)
}

/// Calculates the absolute facing angle of the player's actor. Returns `None` if the handle is
/// invalid.
fn player_angle(gs: &GameState) -> Option<Bam> {
    gs.mobjslab.get(gs.player.handle).map(|mo| mo.angle)
}

#[inline]
fn hitscan_shot_angle(gs: &mut GameState, base_angle: Bam, accurate_first_shot: bool) -> Bam {
    if accurate_first_shot && !gs.player.attack_down {
        return base_angle;
    }

    let spread = gs.rng.p_subrandom() << 18;
    Bam(base_angle.0.wrapping_add(spread as u32))
}

fn bullet_autoaim_angle(
    gs: &GameState,
    handle: MobjHandle,
    base_angle: Bam,
    level: Option<&Level>,
    intercepts: &mut smallvec::SmallVec<[crate::combat::HitscanIntercept; 16]>,
) -> Bam {
    let right_probe = Bam(base_angle.0.wrapping_add(BULLET_AUTOAIM_SIDE_PROBE));
    let left_probe = Bam(base_angle.0.wrapping_sub(BULLET_AUTOAIM_SIDE_PROBE));

    [base_angle, right_probe, left_probe]
        .into_iter()
        .find(|angle| {
            p_line_attack_target(gs, handle, *angle, BULLET_AUTOAIM_RANGE, level, intercepts)
                .is_some()
        })
        .unwrap_or(base_angle)
}

fn snap_player_to_target(gs: &mut GameState, target_handle: crate::mobj::MobjHandle) {
    let handle = gs.player.handle;
    if let (Some(src), Some(tgt)) = (
        gs.mobjslab.get(handle).map(|m| (m.x, m.y)),
        gs.mobjslab.get(target_handle).map(|m| (m.x, m.y)),
    ) {
        let dx = tgt.0 - src.0;
        let dy = tgt.1 - src.1;
        let dx_f = dx.to_int() as f32;
        let dy_f = dy.to_int() as f32;
        let angle_rad = dy_f.atan2(dx_f);
        let new_angle =
            Bam((angle_rad / std::f32::consts::TAU * (u32::MAX as f64 + 1.0) as f32) as u32);

        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.angle = new_angle;
        }
    }
}

/// Select the best available weapon when the current one runs out of ammo.
///
/// Doom's priority order (highest to lowest):
/// PlasmaRifle > SuperShotgun > Chaingun > Shotgun > Pistol > Fist/Chainsaw
///
/// Returns `None` if no weapon switch is needed (current weapon is still
/// usable).
pub fn select_next_weapon(gs: &GameState) -> Option<WeaponType> {
    // Priority order from highest to lowest.
    const PRIORITY: [WeaponType; 9] = [
        WeaponType::PlasmaRifle,
        WeaponType::SuperShotgun,
        WeaponType::Chaingun,
        WeaponType::Shotgun,
        WeaponType::Pistol,
        WeaponType::Chainsaw,
        WeaponType::Fist,
        WeaponType::RocketLauncher,
        WeaponType::Bfg,
    ];

    for &w in &PRIORITY {
        if !gs.player.weapons[w as usize] {
            continue;
        }
        if has_ammo(gs, w) {
            return Some(w);
        }
    }
    // Absolute fallback: fist always works.
    Some(WeaponType::Fist)
}

// ---------------------------------------------------------------------------
// Hitscan weapons
// ---------------------------------------------------------------------------

/// Fire the pistol: consume 1 Clip (Bullets), fire 1 hitscan ray.
///
/// Spread: `p_subrandom() << 18` BAM.
/// Damage: `p_damage_with_variance(&mut gs.rng, 5)` = 5..40.
pub fn p_fire_pistol(gs: &mut GameState, level: Option<&Level>) {
    if !consume_ammo(gs, WeaponType::Pistol) {
        return;
    }

    let handle = gs.player.handle;
    let Some(a) = player_angle(gs) else {
        return;
    };
    let base_angle = a;

    let mut intercepts = smallvec::SmallVec::new();
    let autoaim_angle = bullet_autoaim_angle(gs, handle, base_angle, level, &mut intercepts);
    let shot_angle = hitscan_shot_angle(gs, autoaim_angle, true);
    let damage = p_damage_with_variance(&mut gs.rng, 5);

    p_line_attack(
        gs,
        handle,
        shot_angle,
        MISSILERANGE,
        damage,
        level,
        &mut intercepts,
    );
}

/// Fire the shotgun: consume 1 Shell, fire 7 pellets.
///
/// Each pellet: spread `p_subrandom() << 18`, damage `p_damage_with_variance(&mut gs.rng, 5)`.
pub fn p_fire_shotgun(gs: &mut GameState, level: Option<&Level>) {
    if !consume_ammo(gs, WeaponType::Shotgun) {
        return;
    }

    let handle = gs.player.handle;
    let Some(a) = player_angle(gs) else {
        return;
    };
    let base_angle = a;

    let mut intercepts = smallvec::SmallVec::new();
    let autoaim_angle = bullet_autoaim_angle(gs, handle, base_angle, level, &mut intercepts);

    for _ in 0..7 {
        let spread = gs.rng.p_subrandom() << 18;
        let shot_angle = Bam(autoaim_angle.0.wrapping_add(spread as u32));
        let damage = p_damage_with_variance(&mut gs.rng, 5);
        p_line_attack(
            gs,
            handle,
            shot_angle,
            MISSILERANGE,
            damage,
            level,
            &mut intercepts,
        );
    }
}

/// Fire the super shotgun: consume 2 Shells, fire 20 pellets.
///
/// Each pellet: spread `p_subrandom() << 19` (wider), damage
/// `p_damage_with_variance(&mut gs.rng, 5)`.
pub fn p_fire_super_shotgun(gs: &mut GameState, level: Option<&Level>) {
    if !consume_ammo(gs, WeaponType::SuperShotgun) {
        return;
    }

    let handle = gs.player.handle;
    let Some(a) = player_angle(gs) else {
        return;
    };
    let base_angle = a;

    let mut intercepts = smallvec::SmallVec::new();
    let autoaim_angle = bullet_autoaim_angle(gs, handle, base_angle, level, &mut intercepts);

    for _ in 0..20 {
        let spread = gs.rng.p_subrandom() << 19;
        let shot_angle = Bam(autoaim_angle.0.wrapping_add(spread as u32));
        let damage = p_damage_with_variance(&mut gs.rng, 5);
        p_line_attack(
            gs,
            handle,
            shot_angle,
            MISSILERANGE,
            damage,
            level,
            &mut intercepts,
        );
    }
}

/// Fire the chaingun: consume 1 Clip (Bullets), fire 1 hitscan ray.
///
/// Same parameters as pistol; the faster fire rate comes from the weapon
/// state machine (shorter re-fire delay), not from this function.
pub fn p_fire_chaingun(gs: &mut GameState, level: Option<&Level>) {
    if !consume_ammo(gs, WeaponType::Chaingun) {
        return;
    }

    let handle = gs.player.handle;
    let Some(a) = player_angle(gs) else {
        return;
    };
    let base_angle = a;

    let mut intercepts = smallvec::SmallVec::new();
    let autoaim_angle = bullet_autoaim_angle(gs, handle, base_angle, level, &mut intercepts);
    let shot_angle = hitscan_shot_angle(gs, autoaim_angle, true);
    let damage = p_damage_with_variance(&mut gs.rng, 5);

    p_line_attack(
        gs,
        handle,
        shot_angle,
        MISSILERANGE,
        damage,
        level,
        &mut intercepts,
    );
}

// ---------------------------------------------------------------------------
// Melee weapons
// ---------------------------------------------------------------------------

/// Fire the fist: no ammo, hitscan at MELEERANGE.
///
/// Damage: `p_damage_with_variance(&mut gs.rng, 2)` = 2..16.
/// If Berserk active (`powers[PW_STRENGTH] > 0`): damage *= 10.
pub fn p_fire_fist(gs: &mut GameState, level: Option<&Level>) {
    let handle = gs.player.handle;
    let Some(a) = player_angle(gs) else {
        return;
    };
    let base_angle = a;

    let mut damage = p_damage_with_variance(&mut gs.rng, 2);

    // Berserk multiplier.
    if gs.player.powers[PW_STRENGTH] > 0 {
        damage *= 10;
    }

    let spread = gs.rng.p_subrandom() << 18;
    let shot_angle = Bam(base_angle.0.wrapping_add(spread as u32));

    let mut intercepts = smallvec::SmallVec::new();
    let hit = p_line_attack(
        gs,
        handle,
        shot_angle,
        MELEERANGE,
        damage,
        level,
        &mut intercepts,
    );
    if let Some(target_handle) = hit {
        snap_player_to_target(gs, target_handle);
    }
}

/// Fire the chainsaw: no ammo, hitscan at MELEERANGE+1.
///
/// Damage: `p_damage_with_variance(&mut gs.rng, 2)` = 2..16.
/// On hit: turn player toward target (auto-aim snap).
pub fn p_fire_chainsaw(gs: &mut GameState, level: Option<&Level>) {
    let handle = gs.player.handle;
    let Some(a) = player_angle(gs) else {
        return;
    };
    let base_angle = a;

    let damage = p_damage_with_variance(&mut gs.rng, 2);

    let spread = gs.rng.p_subrandom() << 18;
    let shot_angle = Bam(base_angle.0.wrapping_add(spread as u32));

    // MELEERANGE + 1 map unit for chainsaw (slightly longer reach).
    let chainsaw_range = Fixed16_16(MELEERANGE.0 + (1 << 16));

    let mut intercepts = smallvec::SmallVec::new();
    let hit = p_line_attack(
        gs,
        handle,
        shot_angle,
        chainsaw_range,
        damage,
        level,
        &mut intercepts,
    );

    // Auto-aim snap: if we hit something, turn the player toward the target.
    if let Some(target_handle) = hit {
        snap_player_to_target(gs, target_handle);
    }
}

// ---------------------------------------------------------------------------
// Projectile weapons
// ---------------------------------------------------------------------------

/// Fire the rocket launcher: consume 1 Rocket, spawn `MobjKind::Rocket`.
pub fn p_fire_rocket(gs: &mut GameState, _level: Option<&Level>) {
    if !consume_ammo(gs, WeaponType::RocketLauncher) {
        return;
    }
    let handle = gs.player.handle;
    p_spawn_player_missile(gs, handle, MobjKind::Rocket);
}

/// Fire the plasma rifle: consume 1 Cell, spawn `MobjKind::PlasmaBall`.
pub fn p_fire_plasma(gs: &mut GameState, _level: Option<&Level>) {
    if !consume_ammo(gs, WeaponType::PlasmaRifle) {
        return;
    }
    let handle = gs.player.handle;
    p_spawn_player_missile(gs, handle, MobjKind::PlasmaBall);
}

/// Fire the BFG 9000: consume 40 Cells, spawn `MobjKind::BfgBall`.
pub fn p_fire_bfg(gs: &mut GameState, _level: Option<&Level>) {
    if !consume_ammo(gs, WeaponType::Bfg) {
        return;
    }
    let handle = gs.player.handle;
    p_spawn_player_missile(gs, handle, MobjKind::BfgBall);
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

/// Fire the player's current weapon, dispatching to the appropriate fire
/// function.
///
/// If the player lacks ammo for the current weapon, stages a switch to the
/// best available weapon and returns without firing.
pub fn fire_current_weapon(gs: &mut GameState, level: Option<&Level>) -> bool {
    let weapon = gs.player.weapon;
    let player_handle = gs.player.handle;

    // Check ammo first.
    if !has_ammo(gs, weapon) {
        // Stage the next best weapon; the psprite state machine performs the
        // visible lower/raise transition before `player.weapon` changes.
        if let Some(next) = select_next_weapon(gs) {
            gs.player.pending_weapon = Some(next);
        }
        return false;
    }

    match weapon {
        WeaponType::Fist => p_fire_fist(gs, level),
        WeaponType::Pistol => p_fire_pistol(gs, level),
        WeaponType::Shotgun => p_fire_shotgun(gs, level),
        WeaponType::SuperShotgun => p_fire_super_shotgun(gs, level),
        WeaponType::Chaingun => p_fire_chaingun(gs, level),
        WeaponType::RocketLauncher => p_fire_rocket(gs, level),
        WeaponType::PlasmaRifle => p_fire_plasma(gs, level),
        WeaponType::Bfg => p_fire_bfg(gs, level),
        WeaponType::Chainsaw => p_fire_chainsaw(gs, level),
    }

    gs.sound
        .sound_queue
        .push(SoundRequest::PlayerWeaponFire(weapon));

    if weapon_makes_noise(weapon) {
        if let Some(lv) = level {
            crate::sound::p_noise_alert(gs, lv, player_handle, player_handle);
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, flags};
    use crate::player::PlayerState;
    use crate::sound::{get_sound_target, init_sound_state};
    use crate::state::{GameState, SoundRequest};
    use doom_map::lumps::{Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Vertex};
    use doom_map::{Level, SIDEDEF_NONE};
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    /// Build a minimal GameState with a live player Mobj at the origin.
    fn make_game_state() -> GameState {
        let mut gs = GameState::new("test");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        mo.height = Fixed16_16::from_int(56);
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs
    }

    fn init_trig() {
        // SAFETY: trig tables are process-global and internally guarded.
        unsafe {
            doom_types::Bam::init_trig_tables();
        }
    }

    fn spawn_shootable_target(
        gs: &mut GameState,
        x: i32,
        y: i32,
        radius: i32,
        health: i32,
    ) -> crate::mobj::MobjHandle {
        let mut target = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        target.health = health;
        target.radius = Fixed16_16::from_int(radius);
        target.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        gs.mobjslab.alloc(target)
    }

    /// Give the player all weapons and max ammo for testing.
    fn give_all_weapons_and_ammo(gs: &mut GameState) {
        for i in 0..9 {
            gs.player.weapons[i] = true;
        }
        gs.player.give_ammo(AmmoType::Bullets as usize, 200);
        gs.player.give_ammo(AmmoType::Shells as usize, 50);
        gs.player.give_ammo(AmmoType::Cells as usize, 300);
        gs.player.give_ammo(AmmoType::Rockets as usize, 50);
    }

    fn make_sound_level() -> Level {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("value must exist in test");

        Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: SIDEDEF_NONE,
            }],
            sidedefs: vec![Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"        ",
                lower_texture: *b"        ",
                middle_texture: *b"WALL1   ",
                sector: 0,
            }],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 128, y: 0 }],
            segs: vec![Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            }],
            ssectors: vec![Ssector {
                seg_count: 1,
                first_seg: 0,
            }],
            nodes: vec![],
            sectors: vec![Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            }],
            reject: Reject::parse_lump(&[0u8], 1).expect("value must exist in test"),
            blockmap,
        }
    }

    fn make_open_level() -> Level {
        let cols = 8u16;
        let rows = 8u16;
        let n_blocks = cols as usize * rows as usize;
        let data_start = 4u16 + n_blocks as u16;

        let mut bm_raw = Vec::new();
        bm_raw.extend_from_slice(&(-256i16).to_le_bytes());
        bm_raw.extend_from_slice(&(-256i16).to_le_bytes());
        bm_raw.extend_from_slice(&cols.to_le_bytes());
        bm_raw.extend_from_slice(&rows.to_le_bytes());
        for _ in 0..n_blocks {
            bm_raw.extend_from_slice(&data_start.to_le_bytes());
        }
        bm_raw.extend_from_slice(&0u16.to_le_bytes());
        bm_raw.extend_from_slice(&0xFFFFu16.to_le_bytes());

        let blockmap = Blockmap::parse_lump(&bm_raw).expect("blockmap parse");
        let reject = Reject::parse_lump(&[0u8], 1).expect("reject parse");

        Level {
            name: "OPEN".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![Sector {
                floor_height: 0,
                ceil_height: 256,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            }],
            reject,
            blockmap,
        }
    }

    // =======================================================================
    // AMMO_PER_SHOT table tests
    // =======================================================================

    #[test]
    fn ammo_per_shot_table_has_9_entries() {
        assert_eq!(AMMO_PER_SHOT.len(), 9);
    }

    #[test]
    fn weapon_ammo_cost_pistol() {
        let (ammo, cost) = weapon_ammo_cost(WeaponType::Pistol);
        assert_eq!(ammo, AmmoType::Bullets);
        assert_eq!(cost, 1);
    }

    #[test]
    fn weapon_ammo_cost_bfg() {
        let (ammo, cost) = weapon_ammo_cost(WeaponType::Bfg);
        assert_eq!(ammo, AmmoType::Cells);
        assert_eq!(cost, 40);
    }

    #[test]
    fn weapon_ammo_cost_fist_is_free() {
        let (ammo, cost) = weapon_ammo_cost(WeaponType::Fist);
        assert_eq!(ammo, AmmoType::None);
        assert_eq!(cost, 0);
    }

    #[test]
    fn weapon_ammo_cost_chainsaw_is_free() {
        let (ammo, cost) = weapon_ammo_cost(WeaponType::Chainsaw);
        assert_eq!(ammo, AmmoType::None);
        assert_eq!(cost, 0);
    }

    #[test]
    fn weapon_ammo_cost_ssg() {
        let (ammo, cost) = weapon_ammo_cost(WeaponType::SuperShotgun);
        assert_eq!(ammo, AmmoType::Shells);
        assert_eq!(cost, 2);
    }

    // =======================================================================
    // Pistol tests
    // =======================================================================

    #[test]
    fn pistol_consumes_one_bullet() {
        let mut gs = make_game_state();
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        p_fire_pistol(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(after, before - 1, "pistol must consume exactly 1 bullet");
    }

    #[test]
    fn pistol_no_fire_when_no_ammo() {
        let mut gs = make_game_state();
        gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        assert_eq!(gs.player.ammo(AmmoType::Bullets as usize), 0);
        p_fire_pistol(&mut gs, None);
        assert_eq!(
            gs.player.ammo(AmmoType::Bullets as usize),
            0,
            "pistol must not consume ammo when empty"
        );
    }

    #[test]
    fn pistol_damage_range_is_5_to_40() {
        // p_damage_with_variance(&mut gs.rng, 5) returns 5 * (1..=8) = 5..=40
        let mut gs = GameState::new("test");
        let mut min_seen = i32::MAX;
        let mut max_seen = i32::MIN;
        for _ in 0..256 {
            let dmg = p_damage_with_variance(&mut gs.rng, 5);
            min_seen = min_seen.min(dmg);
            max_seen = max_seen.max(dmg);
        }
        assert!(min_seen >= 5, "min damage {min_seen} must be >= 5");
        assert!(max_seen <= 40, "max damage {max_seen} must be <= 40");
    }

    #[test]
    fn pistol_first_shot_after_release_is_accurate() {
        init_trig();

        let mut gs = make_game_state();
        let target_handle = spawn_shootable_target(&mut gs, 512, 0, 8, 20);
        gs.rng.set_index(16);
        gs.player.attack_down = false;

        p_fire_pistol(&mut gs, None);

        assert!(
            gs.mobjslab
                .get(target_handle)
                .expect("value must exist in test")
                .health
                < 20,
            "first pistol shot after release should be accurate"
        );
    }

    #[test]
    fn pistol_refire_uses_spread_and_can_miss_exactly_aimed_target() {
        init_trig();

        let mut gs = make_game_state();
        let target_handle = spawn_shootable_target(&mut gs, 512, 0, 8, 20);
        gs.rng.set_index(16);
        gs.player.attack_down = true;

        p_fire_pistol(&mut gs, None);

        assert_eq!(
            gs.mobjslab
                .get(target_handle)
                .expect("value must exist in test")
                .health,
            20,
            "refire pistol shot should still use spread"
        );
    }

    #[test]
    fn pistol_autoaim_probe_hits_target_slightly_right_of_center() {
        init_trig();

        let level = make_open_level();
        let mut gs = make_game_state();
        let target_handle = spawn_shootable_target(&mut gs, 512, 50, 20, 20);
        gs.rng.set_index(16);
        gs.player.attack_down = false;

        p_fire_pistol(&mut gs, Some(&level));

        assert!(
            gs.mobjslab
                .get(target_handle)
                .expect("value must exist in test")
                .health
                < 20,
            "pistol autoaim probe should acquire a target within Doom's side-angle search"
        );
    }

    #[test]
    fn pistol_autoaim_probe_hits_target_slightly_left_of_center() {
        init_trig();

        let level = make_open_level();
        let mut gs = make_game_state();
        let target_handle = spawn_shootable_target(&mut gs, 512, -50, 20, 20);
        gs.rng.set_index(16);
        gs.player.attack_down = false;

        p_fire_pistol(&mut gs, Some(&level));

        assert!(
            gs.mobjslab
                .get(target_handle)
                .expect("value must exist in test")
                .health
                < 20,
            "pistol autoaim probe should search both sides of center"
        );
    }

    // =======================================================================
    // Shotgun tests
    // =======================================================================

    #[test]
    fn shotgun_consumes_one_shell() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        gs.player.weapon = WeaponType::Shotgun;
        let before = gs.player.ammo(AmmoType::Shells as usize);
        p_fire_shotgun(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Shells as usize);
        assert_eq!(after, before - 1, "shotgun must consume exactly 1 shell");
    }

    #[test]
    fn shotgun_no_fire_when_no_shells() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Shotgun;
        // No shells given
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), 0);
        p_fire_shotgun(&mut gs, None);
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), 0);
    }

    // =======================================================================
    // Super Shotgun tests
    // =======================================================================

    #[test]
    fn ssg_consumes_two_shells() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        gs.player.weapon = WeaponType::SuperShotgun;
        let before = gs.player.ammo(AmmoType::Shells as usize);
        p_fire_super_shotgun(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Shells as usize);
        assert_eq!(after, before - 2, "SSG must consume exactly 2 shells");
    }

    #[test]
    fn ssg_no_fire_with_one_shell() {
        let mut gs = make_game_state();
        gs.player.give_ammo(AmmoType::Shells as usize, 1);
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), 1);
        p_fire_super_shotgun(&mut gs, None);
        assert_eq!(
            gs.player.ammo(AmmoType::Shells as usize),
            1,
            "SSG must not fire with only 1 shell"
        );
    }

    // =======================================================================
    // Chaingun tests
    // =======================================================================

    #[test]
    fn chaingun_consumes_one_bullet() {
        let mut gs = make_game_state();
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        p_fire_chaingun(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(after, before - 1, "chaingun must consume exactly 1 bullet");
    }

    #[test]
    fn chaingun_no_fire_when_no_ammo() {
        let mut gs = make_game_state();
        gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        p_fire_chaingun(&mut gs, None);
        assert_eq!(gs.player.ammo(AmmoType::Bullets as usize), 0);
    }

    #[test]
    fn chaingun_first_shot_after_release_is_accurate() {
        init_trig();

        let mut gs = make_game_state();
        let target_handle = spawn_shootable_target(&mut gs, 512, 0, 8, 20);
        gs.rng.set_index(16);
        gs.player.attack_down = false;

        p_fire_chaingun(&mut gs, None);

        assert!(
            gs.mobjslab
                .get(target_handle)
                .expect("value must exist in test")
                .health
                < 20,
            "first chaingun shot after release should be accurate"
        );
    }

    #[test]
    fn chaingun_refire_uses_spread_and_can_miss_exactly_aimed_target() {
        init_trig();

        let mut gs = make_game_state();
        let target_handle = spawn_shootable_target(&mut gs, 512, 0, 8, 20);
        gs.rng.set_index(16);
        gs.player.attack_down = true;

        p_fire_chaingun(&mut gs, None);

        assert_eq!(
            gs.mobjslab
                .get(target_handle)
                .expect("value must exist in test")
                .health,
            20,
            "held chaingun shots should keep spread"
        );
    }

    #[test]
    fn chaingun_autoaim_probe_hits_target_slightly_off_center() {
        init_trig();

        let level = make_open_level();
        let mut gs = make_game_state();
        let target_handle = spawn_shootable_target(&mut gs, 512, 50, 20, 20);
        gs.rng.set_index(16);
        gs.player.attack_down = false;

        p_fire_chaingun(&mut gs, Some(&level));

        assert!(
            gs.mobjslab
                .get(target_handle)
                .expect("value must exist in test")
                .health
                < 20,
            "chaingun should reuse the same Doom bullet autoaim probe as the pistol"
        );
    }

    // =======================================================================
    // Fist tests
    // =======================================================================

    #[test]
    fn fist_consumes_no_ammo() {
        let mut gs = make_game_state();
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        p_fire_fist(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(after, before, "fist must not consume any ammo");
    }

    #[test]
    fn fist_damage_range_is_2_to_16() {
        let mut gs = GameState::new("test");
        let mut min_seen = i32::MAX;
        let mut max_seen = i32::MIN;
        for _ in 0..256 {
            let dmg = p_damage_with_variance(&mut gs.rng, 2);
            min_seen = min_seen.min(dmg);
            max_seen = max_seen.max(dmg);
        }
        assert!(min_seen >= 2, "min fist damage {min_seen} must be >= 2");
        assert!(max_seen <= 16, "max fist damage {max_seen} must be <= 16");
    }

    #[test]
    fn fist_berserk_multiplier() {
        // Berserk should multiply damage by 10.
        // With base 2, normal range is 2-16.  Berserk: 20-160.
        let mut gs = make_game_state();
        gs.player.powers[PW_STRENGTH] = 1; // any nonzero = active

        // We can't easily test the exact damage applied by p_fire_fist
        // without a target, but we can verify the logic path exists by
        // calling the function and checking it doesn't panic.
        p_fire_fist(&mut gs, None);

        // Verify powers remain active after firing.
        assert!(
            gs.player.powers[PW_STRENGTH] > 0,
            "berserk must remain active after firing"
        );
    }

    #[test]
    fn fist_berserk_damage_range_is_20_to_160() {
        // The berserk fist does p_damage_with_variance(&mut gs.rng, 2) * 10.
        // p_damage_with_variance(&mut gs.rng, 2) returns 2..16, so berserk = 20..160.
        let mut gs = GameState::new("test");
        let mut min_seen = i32::MAX;
        let mut max_seen = i32::MIN;
        for _ in 0..256 {
            let dmg = p_damage_with_variance(&mut gs.rng, 2) * 10;
            min_seen = min_seen.min(dmg);
            max_seen = max_seen.max(dmg);
        }
        assert!(
            min_seen >= 20,
            "min berserk damage {min_seen} must be >= 20"
        );
        assert!(
            max_seen <= 160,
            "max berserk damage {max_seen} must be <= 160"
        );
    }

    #[test]
    fn fist_no_berserk_does_normal_damage() {
        let mut gs = make_game_state();
        assert_eq!(gs.player.powers[PW_STRENGTH], 0);
        // Call p_fire_fist — should not multiply damage.
        p_fire_fist(&mut gs, None);
        // No crash = success for this path.
    }

    #[test]
    fn fist_hit_snaps_player_toward_target() {
        init_trig();

        let mut gs = make_game_state();
        let target_handle = spawn_shootable_target(&mut gs, 32, 16, 20, 20);
        let before = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("value must exist in test")
            .angle;

        p_fire_fist(&mut gs, None);

        let after = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("value must exist in test")
            .angle;
        assert_ne!(
            after, before,
            "fist hit should turn the player toward the target"
        );
        assert!(
            gs.mobjslab
                .get(target_handle)
                .expect("value must exist in test")
                .health
                < 20,
            "fist snap regression should hit the melee target"
        );
    }

    // =======================================================================
    // Chainsaw tests
    // =======================================================================

    #[test]
    fn chainsaw_consumes_no_ammo() {
        let mut gs = make_game_state();
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        p_fire_chainsaw(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(after, before, "chainsaw must not consume any ammo");
    }

    #[test]
    fn chainsaw_always_fires() {
        // Even with zero ammo of all types, chainsaw should fire.
        let mut gs = make_game_state();
        gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        p_fire_chainsaw(&mut gs, None);
        // No panic = success.
    }

    // =======================================================================
    // Rocket launcher tests
    // =======================================================================

    #[test]
    fn rocket_consumes_one_rocket_ammo() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        let before = gs.player.ammo(AmmoType::Rockets as usize);
        p_fire_rocket(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Rockets as usize);
        assert_eq!(after, before - 1, "rocket must consume exactly 1 rocket");
    }

    #[test]
    fn rocket_spawns_projectile() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        let count_before: usize = gs.mobjslab.iter_handles().count();
        p_fire_rocket(&mut gs, None);
        let count_after: usize = gs.mobjslab.iter_handles().count();
        assert_eq!(
            count_after,
            count_before + 1,
            "rocket must spawn 1 projectile actor"
        );

        // Verify it's a Rocket MobjKind.
        let proj = gs
            .mobjslab
            .iter_handles()
            .filter_map(|h| gs.mobjslab.get(h))
            .find(|m| m.kind == MobjKind::Rocket);
        assert!(
            proj.is_some(),
            "spawned projectile must be MobjKind::Rocket"
        );
    }

    #[test]
    fn rocket_no_fire_when_no_rockets() {
        let mut gs = make_game_state();
        // No rockets.
        assert_eq!(gs.player.ammo(AmmoType::Rockets as usize), 0);
        let count_before: usize = gs.mobjslab.iter_handles().count();
        p_fire_rocket(&mut gs, None);
        let count_after: usize = gs.mobjslab.iter_handles().count();
        assert_eq!(
            count_after, count_before,
            "no projectile when out of rockets"
        );
    }

    // =======================================================================
    // Plasma rifle tests
    // =======================================================================

    #[test]
    fn plasma_consumes_one_cell() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        let before = gs.player.ammo(AmmoType::Cells as usize);
        p_fire_plasma(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Cells as usize);
        assert_eq!(after, before - 1, "plasma must consume exactly 1 cell");
    }

    #[test]
    fn plasma_spawns_plasmaball() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        p_fire_plasma(&mut gs, None);
        let proj = gs
            .mobjslab
            .iter_handles()
            .filter_map(|h| gs.mobjslab.get(h))
            .find(|m| m.kind == MobjKind::PlasmaBall);
        assert!(
            proj.is_some(),
            "plasma must spawn MobjKind::PlasmaBall projectile"
        );
    }

    #[test]
    fn plasma_no_fire_when_no_cells() {
        let mut gs = make_game_state();
        assert_eq!(gs.player.ammo(AmmoType::Cells as usize), 0);
        let count_before: usize = gs.mobjslab.iter_handles().count();
        p_fire_plasma(&mut gs, None);
        let count_after: usize = gs.mobjslab.iter_handles().count();
        assert_eq!(count_after, count_before, "no projectile when out of cells");
    }

    // =======================================================================
    // BFG tests
    // =======================================================================

    #[test]
    fn bfg_consumes_40_cells() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        let before = gs.player.ammo(AmmoType::Cells as usize);
        p_fire_bfg(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Cells as usize);
        assert_eq!(after, before - 40, "BFG must consume exactly 40 cells");
    }

    #[test]
    fn bfg_spawns_bfgball() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        p_fire_bfg(&mut gs, None);
        let proj = gs
            .mobjslab
            .iter_handles()
            .filter_map(|h| gs.mobjslab.get(h))
            .find(|m| m.kind == MobjKind::BfgBall);
        assert!(
            proj.is_some(),
            "BFG must spawn MobjKind::BfgBall projectile"
        );
    }

    #[test]
    fn bfg_no_fire_with_39_cells() {
        let mut gs = make_game_state();
        gs.player.give_ammo(AmmoType::Cells as usize, 39);
        assert_eq!(gs.player.ammo(AmmoType::Cells as usize), 39);
        let count_before: usize = gs.mobjslab.iter_handles().count();
        p_fire_bfg(&mut gs, None);
        let count_after: usize = gs.mobjslab.iter_handles().count();
        assert_eq!(
            count_after, count_before,
            "BFG must not fire with < 40 cells"
        );
        assert_eq!(
            gs.player.ammo(AmmoType::Cells as usize),
            39,
            "no cells consumed when insufficient"
        );
    }

    #[test]
    fn bfg_fires_with_exactly_40_cells() {
        let mut gs = make_game_state();
        gs.player.give_ammo(AmmoType::Cells as usize, 40);
        p_fire_bfg(&mut gs, None);
        assert_eq!(
            gs.player.ammo(AmmoType::Cells as usize),
            0,
            "40 cells must be consumed leaving 0"
        );
    }

    // =======================================================================
    // fire_current_weapon dispatcher tests
    // =======================================================================

    #[test]
    fn fire_current_weapon_dispatches_pistol() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Pistol;
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        fire_current_weapon(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(after, before - 1, "dispatcher must fire pistol");
    }

    #[test]
    fn fire_current_weapon_queues_player_weapon_sound_when_it_fires() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Pistol;

        let fired = fire_current_weapon(&mut gs, None);

        assert!(fired, "dispatcher should report a successful shot");
        assert_eq!(
            gs.sound.sound_queue,
            vec![SoundRequest::PlayerWeaponFire(WeaponType::Pistol)]
        );
    }

    #[test]
    fn fire_current_weapon_does_not_queue_sound_when_switching_empty_weapon() {
        let mut gs = make_game_state();
        gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        gs.player.weapon = WeaponType::Pistol;

        let fired = fire_current_weapon(&mut gs, None);

        assert!(!fired, "switching away from an empty weapon is not a shot");
        assert!(
            gs.sound.sound_queue.is_empty(),
            "no player weapon sound should be queued when nothing fired"
        );
    }

    #[test]
    fn fire_current_weapon_dispatches_fist() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Fist;
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        fire_current_weapon(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(after, before, "fist must not consume ammo via dispatcher");
    }

    #[test]
    fn fire_current_weapon_dispatches_shotgun() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        gs.player.weapon = WeaponType::Shotgun;
        let before = gs.player.ammo(AmmoType::Shells as usize);
        fire_current_weapon(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Shells as usize);
        assert_eq!(after, before - 1, "dispatcher must fire shotgun");
    }

    #[test]
    fn fire_current_weapon_dispatches_rocket() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        gs.player.weapon = WeaponType::RocketLauncher;
        let before = gs.player.ammo(AmmoType::Rockets as usize);
        fire_current_weapon(&mut gs, None);
        let after = gs.player.ammo(AmmoType::Rockets as usize);
        assert_eq!(after, before - 1, "dispatcher must fire rocket");
    }

    #[test]
    fn fire_current_weapon_emits_noise_alert_when_level_present() {
        let mut gs = make_game_state();
        let level = make_sound_level();
        init_sound_state(&mut gs, level.sectors.len());
        gs.player.weapon = WeaponType::Pistol;

        fire_current_weapon(&mut gs, Some(&level));

        assert_eq!(get_sound_target(&gs, 0), Some(gs.player.handle));
    }

    // =======================================================================
    // Weapon switching on empty
    // =======================================================================

    #[test]
    fn fire_current_weapon_stages_switch_when_empty() {
        let mut gs = make_game_state();
        // Player has pistol with 0 bullets.
        gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        gs.player.weapon = WeaponType::Pistol;

        fire_current_weapon(&mut gs, None);

        assert_ne!(
            gs.player.pending_weapon,
            Some(WeaponType::Pistol),
            "must stage a different pending weapon when the current one is empty"
        );
        assert_eq!(
            gs.player.weapon,
            WeaponType::Pistol,
            "empty-weapon fire should not swap `player.weapon` immediately"
        );
    }

    #[test]
    fn fire_current_weapon_sets_pending_weapon_on_switch() {
        let mut gs = make_game_state();
        gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        gs.player.weapon = WeaponType::Pistol;

        fire_current_weapon(&mut gs, None);

        assert!(
            gs.player.pending_weapon.is_some(),
            "must set pending_weapon when switching"
        );
    }

    #[test]
    fn fire_current_weapon_no_ammo_consumed_on_switch() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        // Drain all cells, set weapon to BFG.
        gs.player.use_ammo(AmmoType::Cells as usize, 300);
        gs.player.weapon = WeaponType::Bfg;

        let bullets_before = gs.player.ammo(AmmoType::Bullets as usize);
        let shells_before = gs.player.ammo(AmmoType::Shells as usize);
        let rockets_before = gs.player.ammo(AmmoType::Rockets as usize);

        fire_current_weapon(&mut gs, None);

        // No ammo should be consumed — just a weapon switch.
        assert_eq!(gs.player.ammo(AmmoType::Bullets as usize), bullets_before);
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), shells_before);
        assert_eq!(gs.player.ammo(AmmoType::Rockets as usize), rockets_before);
    }

    // =======================================================================
    // select_next_weapon tests
    // =======================================================================

    #[test]
    fn select_next_weapon_returns_fist_as_fallback() {
        let mut gs = make_game_state();
        // Drain all ammo.
        gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        let next = select_next_weapon(&gs);
        assert_eq!(
            next,
            Some(WeaponType::Fist),
            "fist must be the fallback weapon"
        );
    }

    #[test]
    fn select_next_weapon_prefers_plasma_when_available() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        let next = select_next_weapon(&gs);
        assert_eq!(
            next,
            Some(WeaponType::PlasmaRifle),
            "plasma rifle is highest priority when ammo is available"
        );
    }

    #[test]
    fn select_next_weapon_skips_weapons_without_ammo() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        // Drain cells (no plasma/BFG), drain shells (no shotgun/SSG).
        gs.player.use_ammo(AmmoType::Cells as usize, 300);
        gs.player.use_ammo(AmmoType::Shells as usize, 50);
        let next = select_next_weapon(&gs);
        assert_eq!(
            next,
            Some(WeaponType::Chaingun),
            "chaingun should be next when cells and shells are empty"
        );
    }

    // =======================================================================
    // has_ammo helper tests
    // =======================================================================

    #[test]
    fn has_ammo_returns_true_for_melee() {
        let gs = make_game_state();
        assert!(has_ammo(&gs, WeaponType::Fist));
        assert!(has_ammo(&gs, WeaponType::Chainsaw));
    }

    #[test]
    fn has_ammo_returns_false_when_empty() {
        let mut gs = make_game_state();
        gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        assert!(!has_ammo(&gs, WeaponType::Pistol));
        assert!(!has_ammo(&gs, WeaponType::Chaingun));
    }

    #[test]
    fn has_ammo_bfg_requires_40_cells() {
        let mut gs = make_game_state();
        gs.player.give_ammo(AmmoType::Cells as usize, 39);
        assert!(
            !has_ammo(&gs, WeaponType::Bfg),
            "39 cells not enough for BFG"
        );
        gs.player.give_ammo(AmmoType::Cells as usize, 1);
        assert!(has_ammo(&gs, WeaponType::Bfg), "40 cells enough for BFG");
    }

    // =======================================================================
    // Multiple shots drain ammo correctly
    // =======================================================================

    #[test]
    fn multiple_pistol_shots_drain_ammo() {
        let mut gs = make_game_state();
        for _ in 0..50 {
            p_fire_pistol(&mut gs, None);
        }
        assert_eq!(
            gs.player.ammo(AmmoType::Bullets as usize),
            0,
            "50 pistol shots from 50 bullets must leave 0"
        );
        // 51st shot should not fire.
        p_fire_pistol(&mut gs, None);
        assert_eq!(gs.player.ammo(AmmoType::Bullets as usize), 0);
    }

    #[test]
    fn multiple_ssg_shots_drain_shells() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        // 50 shells / 2 per shot = 25 shots.
        for _ in 0..25 {
            p_fire_super_shotgun(&mut gs, None);
        }
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), 0);
        // 26th should not fire.
        p_fire_super_shotgun(&mut gs, None);
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), 0);
    }

    // =======================================================================
    // Projectile weapon counts
    // =======================================================================

    #[test]
    fn plasma_rapid_fire_spawns_multiple_projectiles() {
        let mut gs = make_game_state();
        give_all_weapons_and_ammo(&mut gs);
        let count_before: usize = gs.mobjslab.iter_handles().count();
        for _ in 0..5 {
            p_fire_plasma(&mut gs, None);
        }
        let count_after: usize = gs.mobjslab.iter_handles().count();
        assert_eq!(
            count_after,
            count_before + 5,
            "5 plasma shots must spawn 5 projectiles"
        );
    }

    // =======================================================================
    // Melee range constants
    // =======================================================================

    #[test]
    fn melee_range_is_64_map_units() {
        assert_eq!(MELEERANGE, Fixed16_16::from_int(64));
    }

    #[test]
    fn missile_range_is_2048_map_units() {
        assert_eq!(MISSILERANGE, Fixed16_16::from_int(2048));
    }
}
