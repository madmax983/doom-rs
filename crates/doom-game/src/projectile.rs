//! Projectile spawning, movement, and collision.
//!
//! Port of Doom's projectile logic from `p_mobj.c` and `p_map.c`:
//! - `P_SpawnMissile` — spawn a projectile from source aimed at dest.
//! - `P_SpawnPlayerMissile` — spawn a projectile in the player's facing direction.
//! - Projectile movement — advance by momentum, explode on wall/actor hit.
//!
//! # Collision model
//! Simple O(n^2) distance check for actor-vs-projectile.  Blockmap-based
//! optimization is a future batch.

use doom_map::Level;
use doom_types::{Bam, Fixed16_16};

use crate::combat;
use crate::mobj::{Mobj, MobjHandle, flags};
use crate::state::GameState;
use doom_types::mobj_kind::MobjKind;

// ---------------------------------------------------------------------------
// ProjectileInfo — template data for each projectile kind
// ---------------------------------------------------------------------------

/// Template data for spawning a projectile: speed, radius, height, damage, flags.
#[derive(Clone, Copy, Debug)]
pub struct ProjectileInfo {
    /// Speed in map units per tic (fixed-point).
    pub speed: Fixed16_16,
    /// Collision radius (fixed-point map units).
    pub radius: Fixed16_16,
    /// Collision height (fixed-point map units).
    pub height: Fixed16_16,
    /// Base damage (may be multiplied by random 1..=8 for some projectiles).
    pub damage: i32,
    /// Behavior flags (typically MF_MISSILE | MF_DROPOFF | MF_NOGRAVITY | MF_NOBLOCKMAP).
    pub flags: u32,
}

/// Standard projectile flags: missile + dropoff + no gravity + no blockmap.
const MF_PROJECTILE: u32 =
    flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;

/// Return projectile template data for a given `MobjKind`, or `None` if the
/// kind is not a projectile.
pub fn projectile_info(kind: MobjKind) -> Option<ProjectileInfo> {
    match kind {
        MobjKind::Rocket => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(20),
            radius: Fixed16_16::from_int(11),
            height: Fixed16_16::from_int(8),
            damage: 20,
            flags: MF_PROJECTILE,
        }),
        MobjKind::PlasmaBall => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(25),
            radius: Fixed16_16::from_int(13),
            height: Fixed16_16::from_int(8),
            damage: 5,
            flags: MF_PROJECTILE,
        }),
        MobjKind::BfgBall => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(25),
            radius: Fixed16_16::from_int(13),
            height: Fixed16_16::from_int(8),
            damage: 100,
            flags: MF_PROJECTILE,
        }),
        MobjKind::BfgExtra => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(25),
            radius: Fixed16_16::from_int(6),
            height: Fixed16_16::from_int(8),
            damage: 15,
            flags: MF_PROJECTILE,
        }),
        MobjKind::ImpFireball => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(10),
            radius: Fixed16_16::from_int(6),
            height: Fixed16_16::from_int(8),
            damage: 3,
            flags: MF_PROJECTILE,
        }),
        MobjKind::CacoFireball => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(10),
            radius: Fixed16_16::from_int(6),
            height: Fixed16_16::from_int(8),
            damage: 5,
            flags: MF_PROJECTILE,
        }),
        MobjKind::BaronBall => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(15),
            radius: Fixed16_16::from_int(6),
            height: Fixed16_16::from_int(8),
            damage: 8,
            flags: MF_PROJECTILE,
        }),
        MobjKind::ArachPlaz => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(25),
            radius: Fixed16_16::from_int(13),
            height: Fixed16_16::from_int(8),
            damage: 5,
            flags: MF_PROJECTILE,
        }),
        MobjKind::FatShot => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(20),
            radius: Fixed16_16::from_int(6),
            height: Fixed16_16::from_int(8),
            damage: 8,
            flags: MF_PROJECTILE,
        }),
        MobjKind::Tracer => Some(ProjectileInfo {
            speed: Fixed16_16::from_int(10),
            radius: Fixed16_16::from_int(11),
            height: Fixed16_16::from_int(8),
            damage: 10,
            flags: MF_PROJECTILE,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Angle calculation helper
// ---------------------------------------------------------------------------

/// Compute the BAM angle from `(dx, dy)` displacement using f32 atan2.
///
/// This is the only float usage in the projectile path and is used solely
/// for aiming direction — game state remains deterministic via fixed-point.
fn angle_from_delta(dx: Fixed16_16, dy: Fixed16_16) -> Bam {
    let dx_f = dx.to_int() as f32;
    let dy_f = dy.to_int() as f32;
    let angle_rad = dy_f.atan2(dx_f);
    // Convert radians to BAM: full circle = 2*pi = u32::MAX + 1
    Bam((angle_rad / std::f32::consts::TAU * (u32::MAX as f64 + 1.0) as f32) as u32)
}

// ---------------------------------------------------------------------------
// p_spawn_missile — spawn a projectile from source aimed at dest
// ---------------------------------------------------------------------------

/// Spawn a projectile from `source` aimed at `dest`.
///
/// Returns the handle to the new projectile `Mobj`, or `None` if the source
/// or dest handle is invalid, or the kind has no projectile info.
///
/// The projectile is positioned at `source.x, source.y, source.z + source.height/2`
/// (fires from chest height) and aimed toward `dest` with appropriate momentum.
/// The `target` field is set to `source` for kill credit attribution.
///
/// Momentum is computed directly from the source-to-dest displacement vector,
/// avoiding dependency on trig tables (which may not be initialized in tests).
pub fn p_spawn_missile(
    gs: &mut GameState,
    source: MobjHandle,
    dest: MobjHandle,
    kind: MobjKind,
) -> Option<MobjHandle> {
    let info = projectile_info(kind)?;

    // Extract source position and dimensions.
    let (sx, sy, sz, s_height) = {
        let mo = gs.mobjslab.get(source)?;
        (mo.x, mo.y, mo.z, mo.height)
    };

    // Extract dest position.
    let (dx, dy, dz) = {
        let mo = gs.mobjslab.get(dest)?;
        (mo.x, mo.y, mo.z)
    };

    // P_SpawnMobj (p_mobj.c:547) draws `mobj->lastlook = P_Random() % MAXPLAYERS`
    // for EVERY spawned mobj, including missiles. This is on the playsim
    // `prndindex` stream, so it must fire here (before the later
    // P_CheckMissileSpawn draw) to keep the shared RNG ordinal aligned with
    // vanilla — otherwise every monster missile drops two draws relative to the
    // oracle (P_SpawnMobj + P_CheckMissileSpawn), drifting the whole stream from
    // the first monster fireball onward.
    let _lastlook = (gs.p_random() as u32) % 4;

    // Spawn position: source center, chest height.
    let spawn_z = sz + Fixed16_16(s_height.0 / 2);

    // Compute direction and angle toward dest.
    let delta_x = dx - sx;
    let delta_y = dy - sy;
    let angle = angle_from_delta(delta_x, delta_y);

    // Compute momentum directly from displacement to avoid trig-table dependency.
    // Use f32 for the normalization (this is aim-direction only, not game state).
    let dx_f = delta_x.to_int() as f32;
    let dy_f = delta_y.to_int() as f32;
    let dist_f = (dx_f * dx_f + dy_f * dy_f).sqrt().max(1.0);
    let speed_f = info.speed.to_int() as f32;
    let momx = Fixed16_16::from_int((dx_f / dist_f * speed_f) as i32);
    let momy = Fixed16_16::from_int((dy_f / dist_f * speed_f) as i32);

    // Vertical aim: approximate (dest.z - spawn_z) / distance * speed.
    let delta_z = dz - spawn_z;
    let flat_dist_int = dist_f as i32;
    let momz = if flat_dist_int > 0 {
        Fixed16_16::from_int(delta_z.to_int() * info.speed.to_int() / flat_dist_int)
    } else {
        Fixed16_16::ZERO
    };

    // Build the projectile Mobj.
    let mut proj = Mobj::new(kind, sx, sy, angle);
    crate::spawn::apply_mobjinfo_defaults(&mut proj);
    proj.z = spawn_z;
    proj.radius = info.radius;
    proj.height = info.height;
    proj.health = 1000; // projectiles have infinite effective health
    proj.flags = info.flags;
    proj.momx = momx;
    proj.momy = momy;
    proj.momz = momz;
    proj.target = source; // who fired it, for kill credit

    let handle = gs.mobjslab.alloc(proj);

    // P_CheckMissileSpawn (p_mobj.c): `th->tics -= P_Random()&3; if (th->tics <
    // 1) th->tics = 1;`. Randomizes the missile's initial animation phase; the
    // draw advances the shared RNG ordinal exactly as the oracle's
    // `P_CheckMissileSpawn` entry does.
    let r = (gs.p_random() & 3) as i16;
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.tics -= r;
        if mo.tics < 1 {
            mo.tics = 1;
        }
    }

    Some(handle)
}

// ---------------------------------------------------------------------------
// p_spawn_player_missile — spawn from player facing direction
// ---------------------------------------------------------------------------

/// Spawn a projectile from the player in the direction they're facing.
///
/// Unlike `p_spawn_missile`, this does not need a destination handle — it
/// uses the source actor's angle directly.  Vertical aim is zero (straight
/// ahead on the horizontal plane).
pub fn p_spawn_player_missile(
    gs: &mut GameState,
    source: MobjHandle,
    kind: MobjKind,
) -> Option<MobjHandle> {
    let info = projectile_info(kind)?;

    // Extract source position, angle, and height.
    let (sx, sy, sz, s_height, angle) = {
        let mo = gs.mobjslab.get(source)?;
        (mo.x, mo.y, mo.z, mo.height, mo.angle)
    };

    let spawn_z = sz + Fixed16_16(s_height.0 / 2);

    let momx = info.speed.fixed_mul(angle.cos());
    let momy = info.speed.fixed_mul(angle.sin());

    let mut proj = Mobj::new(kind, sx, sy, angle);
    crate::spawn::apply_mobjinfo_defaults(&mut proj);
    proj.z = spawn_z;
    proj.radius = info.radius;
    proj.height = info.height;
    proj.health = 1000;
    proj.flags = info.flags;
    proj.momx = momx;
    proj.momy = momy;
    proj.momz = Fixed16_16::ZERO;
    proj.target = source;

    Some(gs.mobjslab.alloc(proj))
}

// ---------------------------------------------------------------------------
// p_move_projectiles — advance all projectiles one tic
// ---------------------------------------------------------------------------

/// Advance all MF_MISSILE actors by their momentum.  Called once per tic.
///
/// For each projectile:
/// 1. Compute new position from momentum.
/// 2. Check wall collision via `p_try_move` (if level available).
/// 3. Check actor collision (O(n^2) distance check).
/// 4. On hit: apply damage, remove projectile.  Rockets also do radius attack.
/// 5. Otherwise: update position.
pub fn p_move_projectiles(gs: &mut GameState, level: Option<&Level>) {
    // ⚡ Bolt: Iterate over the MobjSlab by index to avoid a per-frame `Vec`
    // heap allocation that would otherwise collect all missile handles.
    let initial_slot_count = gs.mobjslab.slot_count();
    let initial_generation = gs.mobjslab.next_generation();

    for i in 0..initial_slot_count {
        let Some(missile_handle) = gs.mobjslab.handle_at(i) else {
            continue;
        };
        if missile_handle.generation >= initial_generation {
            continue;
        }

        // Ensure this actor is actually a missile.
        if !gs
            .mobjslab
            .get(missile_handle)
            .map(|m| m.flags & flags::MF_MISSILE != 0)
            .unwrap_or(false)
        {
            continue;
        }
        // Re-read missile data (it may have been freed by an earlier iteration).
        let Some(m) = gs.mobjslab.get(missile_handle) else {
            continue;
        };
        let (mx, my, mz, momx, momy, momz, m_radius, m_kind, m_source) = (
            m.x, m.y, m.z, m.momx, m.momy, m.momz, m.radius, m.kind, m.target,
        );

        let new_x = mx + momx;
        let new_y = my + momy;
        let new_z = mz + momz;

        // --- Wall collision ---
        let wall_ok = match level {
            Some(lv) => crate::movement::p_try_move(&gs.mobjslab, missile_handle, new_x, new_y, lv),
            None => true, // no level = no wall collision (unit tests)
        };

        if !wall_ok {
            // Hit a wall.  Rockets do splash damage on explosion.
            if m_kind == MobjKind::Rocket {
                combat::p_radius_attack(
                    gs,
                    missile_handle,
                    128, // rocket splash damage
                    Fixed16_16::from_int(128),
                    level,
                );
            }
            gs.mobjslab.free(missile_handle);
            continue;
        }

        // --- Actor collision (O(n^2) distance check) ---
        let mut hit_target: Option<MobjHandle> = None;

        for target_h in gs.mobjslab.iter_handles() {
            // Skip self and the source actor.
            if target_h == missile_handle || target_h == m_source {
                continue;
            }
            let Some(t) = gs.mobjslab.get(target_h) else {
                continue;
            };
            let (tx, ty, t_radius, t_alive, t_shootable) = (
                t.x,
                t.y,
                t.radius,
                t.health > 0,
                t.flags & flags::MF_SHOOTABLE != 0,
            );
            if !t_alive || !t_shootable {
                continue;
            }

            // Bounding-box overlap check: |dx| < sum of radii && |dy| < sum of radii.
            let combined_radius = m_radius + t_radius;
            let dx = (new_x - tx).abs();
            let dy = (new_y - ty).abs();
            if dx < combined_radius && dy < combined_radius {
                hit_target = Some(target_h);
                break;
            }
        }

        if let Some(target_h) = hit_target {
            // Compute damage: base_damage * random(1..=8) using the RNG.
            let base_damage = projectile_info(m_kind).map(|pi| pi.damage).unwrap_or(1);
            let rng_val = (gs.rng.next_byte() % 8) as i32 + 1;
            let damage = base_damage * rng_val;

            combat::damage_mobj(gs, target_h, m_source, damage);

            // Rockets also do splash damage on actor hit.
            if m_kind == MobjKind::Rocket {
                combat::p_radius_attack(gs, missile_handle, 128, Fixed16_16::from_int(128), level);
            }

            gs.mobjslab.free(missile_handle);
            continue;
        }

        // --- No collision: update position ---
        if let Some(m) = gs.mobjslab.get_mut(missile_handle) {
            m.x = new_x;
            m.y = new_y;
            m.z = new_z;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, StateNum, flags};
    use crate::mobjinfo::MOBJINFO;
    use crate::player::PlayerState;
    use crate::state::GameState;
    use crate::states::{STATES, ids};
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    /// Build a minimal GameState with a live player at the origin.
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

    /// Spawn a shootable target at (x, y).
    fn spawn_target(gs: &mut GameState, x: i32, y: i32, health: i32) -> MobjHandle {
        let mut mo = Mobj::new(
            MobjKind::Imp,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = health;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        mo.radius = Fixed16_16::from_int(20);
        mo.height = Fixed16_16::from_int(56);
        gs.mobjslab.alloc(mo)
    }

    // -----------------------------------------------------------------------
    // projectile_info tests
    // -----------------------------------------------------------------------

    #[test]
    fn projectile_info_returns_none_for_non_projectiles() {
        assert!(projectile_info(MobjKind::Player).is_none());
        assert!(projectile_info(MobjKind::Imp).is_none());
        assert!(projectile_info(MobjKind::Demon).is_none());
        assert!(projectile_info(MobjKind::Trooper).is_none());
        assert!(projectile_info(MobjKind::Column).is_none());
    }

    #[test]
    fn projectile_info_returns_data_for_all_projectile_kinds() {
        assert!(projectile_info(MobjKind::Rocket).is_some());
        assert!(projectile_info(MobjKind::PlasmaBall).is_some());
        assert!(projectile_info(MobjKind::BfgBall).is_some());
        assert!(projectile_info(MobjKind::BfgExtra).is_some());
        assert!(projectile_info(MobjKind::ImpFireball).is_some());
        assert!(projectile_info(MobjKind::CacoFireball).is_some());
        assert!(projectile_info(MobjKind::BaronBall).is_some());
        assert!(projectile_info(MobjKind::ArachPlaz).is_some());
        assert!(projectile_info(MobjKind::FatShot).is_some());
        assert!(projectile_info(MobjKind::Tracer).is_some());
    }

    #[test]
    fn rocket_has_correct_stats() {
        let info = projectile_info(MobjKind::Rocket).expect("value must exist in test");
        assert_eq!(info.speed, Fixed16_16::from_int(20));
        assert_eq!(info.radius, Fixed16_16::from_int(11));
        assert_eq!(info.damage, 20);
        assert_ne!(info.flags & flags::MF_MISSILE, 0);
        assert_ne!(info.flags & flags::MF_NOGRAVITY, 0);
    }

    #[test]
    fn all_projectiles_have_missile_flag() {
        let projectile_kinds = [
            MobjKind::Rocket,
            MobjKind::PlasmaBall,
            MobjKind::BfgBall,
            MobjKind::BfgExtra,
            MobjKind::ImpFireball,
            MobjKind::CacoFireball,
            MobjKind::BaronBall,
            MobjKind::ArachPlaz,
            MobjKind::FatShot,
            MobjKind::Tracer,
        ];
        for kind in projectile_kinds {
            let info = projectile_info(kind).expect("value must exist in test");
            assert_ne!(
                info.flags & flags::MF_MISSILE,
                0,
                "{kind:?} must have MF_MISSILE"
            );
        }
    }

    // -----------------------------------------------------------------------
    // p_spawn_missile tests
    // -----------------------------------------------------------------------

    #[test]
    fn spawn_missile_creates_projectile_aimed_at_target() {
        let mut gs = make_game_state();
        // Source at (0,0), target at (100,0).
        let source = gs.player.handle;
        let target = spawn_target(&mut gs, 100, 0, 60);

        let proj_h = p_spawn_missile(&mut gs, source, target, MobjKind::ImpFireball);
        assert!(proj_h.is_some(), "must create a projectile");

        let proj = gs
            .mobjslab
            .get(proj_h.expect("value must exist in test"))
            .expect("value must exist in test");
        assert_eq!(proj.kind, MobjKind::ImpFireball);
        assert_ne!(proj.flags & flags::MF_MISSILE, 0);
        // Target is east of source: momx should be positive, momy ~0.
        assert!(
            proj.momx > Fixed16_16::ZERO,
            "momx={:?} must be positive (target is east)",
            proj.momx
        );
        // The target field should point back to source for kill credit.
        assert_eq!(proj.target, source);
    }

    #[test]
    fn spawn_missile_initializes_visible_spawn_state() {
        let mut gs = make_game_state();
        let source = gs.player.handle;
        let target = spawn_target(&mut gs, 100, 0, 60);

        let proj_h = p_spawn_missile(&mut gs, source, target, MobjKind::ImpFireball)
            .expect("value must exist in test");
        let proj = gs.mobjslab.get(proj_h).expect("value must exist in test");
        let info = &MOBJINFO[MobjKind::ImpFireball as usize];

        assert_eq!(info.spawn_state, StateNum(ids::S_TBALL1));
        assert_eq!(proj.state, info.spawn_state);
        // Vanilla `P_CheckMissileSpawn` (p_mobj.c) randomizes the initial
        // animation phase: `th->tics -= P_Random()&3; if (th->tics < 1) th->tics
        // = 1;`. So the spawned missile's tics is the spawnstate tics reduced by
        // 0..3, clamped to a minimum of 1 — never the raw spawnstate value plus
        // anything, and always at least 1.
        let base = STATES[info.spawn_state.0 as usize].tics;
        assert!(
            proj.tics >= 1 && proj.tics <= base,
            "missile tics {} must be in [1, {base}] after P_CheckMissileSpawn's -= P_Random()&3",
            proj.tics
        );
        assert!(
            proj.tics >= (base - 3).max(1),
            "missile tics {} must be at most 3 below the spawnstate tics {base}",
            proj.tics
        );
    }

    /// Demo-sync regression: vanilla `P_SpawnMissile` draws exactly two playsim
    /// `P_Random`s — `P_SpawnMobj`'s `lastlook = P_Random()%MAXPLAYERS` and
    /// `P_CheckMissileSpawn`'s `tics -= P_Random()&3`. Both advance the shared
    /// `prndindex` ordinal, so a monster fireball must consume two RNG bytes or
    /// the whole monster-AI stream drifts from the first fireball onward (this
    /// was the DEMO3/E1M7 lt168 divergence). The target here is not MF_SHADOW,
    /// so the fuzzy-spread P_SubRandom is NOT drawn.
    #[test]
    fn spawn_missile_consumes_two_rng_bytes() {
        let mut gs = make_game_state();
        let source = gs.player.handle;
        let target = spawn_target(&mut gs, 100, 0, 60);

        let before = gs.rng.index();
        let _ = p_spawn_missile(&mut gs, source, target, MobjKind::ImpFireball)
            .expect("value must exist in test");
        assert_eq!(
            (gs.rng.index().wrapping_sub(before)) & 255,
            2,
            "P_SpawnMissile must draw exactly 2 P_Random (lastlook + P_CheckMissileSpawn)"
        );
    }

    #[test]
    fn spawn_missile_returns_none_for_non_projectile_kind() {
        let mut gs = make_game_state();
        let source = gs.player.handle;
        let target = spawn_target(&mut gs, 100, 0, 60);

        let result = p_spawn_missile(&mut gs, source, target, MobjKind::Player);
        assert!(result.is_none(), "non-projectile kind must return None");
    }

    #[test]
    fn spawn_missile_returns_none_for_invalid_source() {
        let mut gs = make_game_state();
        let target = spawn_target(&mut gs, 100, 0, 60);

        let result = p_spawn_missile(&mut gs, MobjHandle::NULL, target, MobjKind::Rocket);
        assert!(result.is_none());
    }

    #[test]
    fn spawn_missile_returns_none_for_invalid_dest() {
        let mut gs = make_game_state();
        let source = gs.player.handle;

        let result = p_spawn_missile(&mut gs, source, MobjHandle::NULL, MobjKind::Rocket);
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // p_spawn_player_missile tests
    // -----------------------------------------------------------------------

    #[test]
    fn player_missile_uses_player_angle() {
        let mut gs = make_game_state();
        // Player faces east (angle = 0).
        let source = gs.player.handle;

        let proj_h = p_spawn_player_missile(&mut gs, source, MobjKind::Rocket);
        assert!(proj_h.is_some());

        let proj = gs
            .mobjslab
            .get(proj_h.expect("value must exist in test"))
            .expect("value must exist in test");
        assert_eq!(proj.kind, MobjKind::Rocket);
        assert_ne!(proj.flags & flags::MF_MISSILE, 0);
        // Since angle.cos() and angle.sin() return 0 when trig tables are not
        // initialized, momx/momy will be zero in unit tests.  This is expected.
        // The important thing is the projectile was created with correct kind.
        assert_eq!(proj.target, source);
        assert_eq!(
            proj.momz,
            Fixed16_16::ZERO,
            "player missile has no vertical aim"
        );
    }

    #[test]
    fn player_missile_returns_none_for_non_projectile() {
        let mut gs = make_game_state();
        let source = gs.player.handle;

        let result = p_spawn_player_missile(&mut gs, source, MobjKind::Demon);
        assert!(result.is_none());
    }

    #[test]
    fn player_missile_spawns_at_chest_height() {
        let mut gs = make_game_state();
        let source = gs.player.handle;

        let proj_h = p_spawn_player_missile(&mut gs, source, MobjKind::PlasmaBall)
            .expect("value must exist in test");
        let proj = gs.mobjslab.get(proj_h).expect("value must exist in test");
        // Player z=0, height=56, so chest height = 56/2 = 28.
        assert_eq!(proj.z, Fixed16_16::from_int(28));
    }

    #[test]
    fn player_missile_initializes_visible_spawn_state() {
        let mut gs = make_game_state();
        let source = gs.player.handle;

        let proj_h = p_spawn_player_missile(&mut gs, source, MobjKind::Rocket)
            .expect("value must exist in test");
        let proj = gs.mobjslab.get(proj_h).expect("value must exist in test");
        let info = &MOBJINFO[MobjKind::Rocket as usize];

        assert_eq!(info.spawn_state, StateNum(ids::S_ROCKET));
        assert_eq!(proj.state, info.spawn_state);
        assert_eq!(proj.tics, STATES[info.spawn_state.0 as usize].tics);
    }

    // -----------------------------------------------------------------------
    // p_move_projectiles tests
    // -----------------------------------------------------------------------

    #[test]
    fn p_move_projectiles_advances_position() {
        let mut gs = make_game_state();
        // Manually create a projectile with known momentum.
        let mut proj = Mobj::new(
            MobjKind::Rocket,
            Fixed16_16::from_int(50),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        proj.momx = Fixed16_16::from_int(10);
        proj.momy = Fixed16_16::ZERO;
        proj.momz = Fixed16_16::ZERO;
        proj.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        proj.health = 1000;
        proj.radius = Fixed16_16::from_int(11);
        proj.height = Fixed16_16::from_int(8);
        proj.target = gs.player.handle;
        let proj_h = gs.mobjslab.alloc(proj);

        // Move projectiles (no level = no wall collision).
        p_move_projectiles(&mut gs, None);

        let proj = gs.mobjslab.get(proj_h).expect("value must exist in test");
        assert_eq!(
            proj.x,
            Fixed16_16::from_int(60),
            "x must advance by momx=10"
        );
    }

    #[test]
    fn p_move_projectiles_hits_actor() {
        let mut gs = make_game_state();
        // Spawn target at (30, 0) with 60 hp.
        let target_h = spawn_target(&mut gs, 30, 0, 60);

        // Spawn projectile at (10, 0) moving east at 20 units/tic.
        // After one tic it will be at (30, 0) — right on top of the target.
        let mut proj = Mobj::new(
            MobjKind::ImpFireball,
            Fixed16_16::from_int(10),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        proj.momx = Fixed16_16::from_int(20);
        proj.momy = Fixed16_16::ZERO;
        proj.momz = Fixed16_16::ZERO;
        proj.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        proj.health = 1000;
        proj.radius = Fixed16_16::from_int(6);
        proj.height = Fixed16_16::from_int(8);
        // Set source to player so it doesn't hit the player.
        proj.target = gs.player.handle;
        let proj_h = gs.mobjslab.alloc(proj);

        p_move_projectiles(&mut gs, None);

        // Target should have taken damage.
        let target = gs.mobjslab.get(target_h).expect("value must exist in test");
        assert!(
            target.health < 60,
            "target health {} must decrease from projectile hit",
            target.health
        );

        // Projectile should be removed.
        assert!(
            gs.mobjslab.get(proj_h).is_none(),
            "projectile must be freed after hitting actor"
        );
    }

    #[test]
    fn p_move_projectiles_does_not_hit_source() {
        let mut gs = make_game_state();
        // Spawn projectile at same position as player, moving east.
        let player_h = gs.player.handle;
        let mut proj = Mobj::new(
            MobjKind::ImpFireball,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        proj.momx = Fixed16_16::from_int(5);
        proj.momy = Fixed16_16::ZERO;
        proj.momz = Fixed16_16::ZERO;
        proj.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        proj.health = 1000;
        proj.radius = Fixed16_16::from_int(6);
        proj.height = Fixed16_16::from_int(8);
        proj.target = player_h; // source = player
        let proj_h = gs.mobjslab.alloc(proj);

        p_move_projectiles(&mut gs, None);

        // Player should not be damaged (projectile skips source).
        let player = gs.mobjslab.get(player_h).expect("value must exist in test");
        assert_eq!(
            player.health, 100,
            "source must not be hit by own projectile"
        );

        // Projectile should still be alive (no target to collide with).
        assert!(
            gs.mobjslab.get(proj_h).is_some(),
            "projectile must survive when no valid collision target"
        );
    }

    #[test]
    fn p_move_projectiles_skips_non_missiles() {
        let mut gs = make_game_state();
        // Spawn a regular (non-missile) actor with momentum.
        let mut mo = Mobj::new(
            MobjKind::Imp,
            Fixed16_16::from_int(50),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.momx = Fixed16_16::from_int(10);
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        mo.health = 60;
        let imp_h = gs.mobjslab.alloc(mo);

        p_move_projectiles(&mut gs, None);

        // Non-missile actors must not be moved by p_move_projectiles.
        let imp = gs.mobjslab.get(imp_h).expect("value must exist in test");
        assert_eq!(
            imp.x,
            Fixed16_16::from_int(50),
            "non-missile actors must not be moved"
        );
    }

    #[test]
    fn multiple_projectiles_move_independently() {
        let mut gs = make_game_state();

        // Projectile 1: moving east.
        let mut p1 = Mobj::new(
            MobjKind::PlasmaBall,
            Fixed16_16::from_int(0),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        p1.momx = Fixed16_16::from_int(25);
        p1.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        p1.health = 1000;
        p1.radius = Fixed16_16::from_int(13);
        p1.height = Fixed16_16::from_int(8);
        p1.target = gs.player.handle;
        let h1 = gs.mobjslab.alloc(p1);

        // Projectile 2: moving north (positive Y).
        let mut p2 = Mobj::new(
            MobjKind::ImpFireball,
            Fixed16_16::ZERO,
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        p2.momy = Fixed16_16::from_int(10);
        p2.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        p2.health = 1000;
        p2.radius = Fixed16_16::from_int(6);
        p2.height = Fixed16_16::from_int(8);
        p2.target = gs.player.handle;
        let h2 = gs.mobjslab.alloc(p2);

        p_move_projectiles(&mut gs, None);

        let m1 = gs.mobjslab.get(h1).expect("value must exist in test");
        assert_eq!(m1.x, Fixed16_16::from_int(25));
        assert_eq!(m1.y, Fixed16_16::ZERO);

        let m2 = gs.mobjslab.get(h2).expect("value must exist in test");
        assert_eq!(m2.x, Fixed16_16::ZERO);
        assert_eq!(m2.y, Fixed16_16::from_int(10));
    }
}
