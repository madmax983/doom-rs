//! Projectile spawning.
//!
//! Port of Doom's projectile spawn logic from `p_mobj.c`:
//! - `P_SpawnMissile` — spawn a projectile from source aimed at dest, with the
//!   exact fixed-point aim (`FixedMul(speed, finecosine/finesine)`) and the
//!   `P_CheckMissileSpawn` half-momentum nudge.
//! - `P_SpawnPlayerMissile` — spawn a projectile in the player's facing direction.
//!
//! Projectile MOVEMENT and COLLISION live in `tic.rs`: a missile is advanced by
//! its momentum every tic inside `P_MobjThinker` (`p_xy_movement_missile` +
//! `PIT_CheckThing` + `p_explode_missile`, then `p_z_movement_missile`), exactly
//! as vanilla does — not by a separate projectile pass.

use doom_types::{Bam, Fixed16_16};

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

    // Extract source position.
    let (sx, sy, sz) = {
        let mo = gs.mobjslab.get(source)?;
        (mo.x, mo.y, mo.z)
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

    // Spawn position (vanilla `P_SpawnMissile`): `source->z + 4*8*FRACUNIT`
    // (a fixed 32 units above the shooter's feet), NOT the shooter's mid-height.
    let spawn_z = sz + Fixed16_16::from_int(32);

    // Aim exactly as vanilla does, in fixed-point: the BAM angle from source to
    // dest, then `momx/momy = FixedMul(speed, finecosine/finesine[angle])`.
    // Using the fixed-point trig tables (not f32 with an integer-truncated
    // magnitude) is essential for demo sync — a quantized momentum drifts the
    // projectile off vanilla's path and makes it strike its target several tics
    // early or late. The target is never MF_SHADOW here, so no `P_SubRandom`
    // fuzz draw is taken.
    let an = crate::geom::r_point_to_angle2(sx.raw(), sy.raw(), dx.raw(), dy.raw());
    let angle = Bam(an);
    let speed_raw = info.speed.raw();
    let momx = Fixed16_16::from_raw(crate::geom::fixed_mul(speed_raw, crate::geom::fine_cosine(an)));
    let momy = Fixed16_16::from_raw(crate::geom::fixed_mul(speed_raw, crate::geom::fine_sine(an)));

    // Vertical aim (vanilla): `dist = P_AproxDistance(dx,dy) / speed; if (dist<1)
    // dist=1; momz = (dest->z - source->z) / dist`. Note this uses the shooter's
    // feet (`sz`), not the spawn z.
    let dist = crate::geom::p_aprox_distance((dx - sx).raw(), (dy - sy).raw());
    let dist_tics = (dist / speed_raw).max(1);
    let momz = Fixed16_16::from_raw((dz - sz).raw() / dist_tics);

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
        // P_CheckMissileSpawn (p_mobj.c) also nudges the missile forward by HALF
        // its momentum ("move a little forward so an angle can be computed if it
        // immediately explodes"). This half-step is essential for demo sync: on
        // the spawn tic vanilla applies this nudge AND then the thinker's full
        // `P_XYMovement` step, so a monster-fired missile is 1.5 steps along its
        // path by the end of its spawn tic — without the nudge every projectile
        // trails vanilla by exactly half a step for its whole flight, striking
        // its target a step off. Vanilla additionally `P_TryMove`s here and
        // explodes on a blocked nudge; that point-blank collision is instead
        // resolved by the missile's first thinker step this same tic.
        mo.x += Fixed16_16::from_raw(momx.raw() >> 1);
        mo.y += Fixed16_16::from_raw(momy.raw() >> 1);
        mo.z += Fixed16_16::from_raw(momz.raw() >> 1);
    }

    Some(handle)
}

// ---------------------------------------------------------------------------
// p_spawn_player_missile — spawn from player facing direction
// ---------------------------------------------------------------------------

/// Spawn a projectile from the player in the direction they're facing.
///
/// Faithful port of vanilla `P_SpawnPlayerMissile` (`p_mobj.c:1027`):
///
/// 1. **Autoaim.** Probe `P_AimLineAttack` at the player's facing angle, then
///    (if no target) `+1<<26`, then `-1<<26`, over `16*64*FRACUNIT`. The angle
///    that acquired a target becomes the missile's horizontal angle; its
///    returned `slope` becomes the vertical aim. If none acquire a target the
///    angle reverts to the player's facing and slope is 0.
/// 2. **Spawn z** is `source->z + 4*8*FRACUNIT` (a fixed 32 units above the
///    shooter's feet), *not* mid-height.
/// 3. `P_SpawnMobj` draws `lastlook = P_Random() % MAXPLAYERS` for every mobj.
/// 4. `momx/momy = FixedMul(speed, finecosine/finesine[an])`, `momz =
///    FixedMul(speed, slope)`.
/// 5. `P_CheckMissileSpawn` draws `tics -= P_Random()&3` (min 1) and nudges the
///    missile forward by half its momentum.
///
/// The two `P_Random` draws (steps 3 and 5) advance the shared playsim RNG
/// exactly as vanilla does; omitting them drops two draws on every rocket /
/// plasma / BFG shot and desyncs the whole stream from the first shot onward.
pub fn p_spawn_player_missile(
    gs: &mut GameState,
    source: MobjHandle,
    kind: MobjKind,
    level: Option<&doom_map::Level>,
) -> Option<MobjHandle> {
    let info = projectile_info(kind)?;

    // Extract source position, feet z, and facing angle.
    let (sx, sy, sz, base_angle) = {
        let mo = gs.mobjslab.get(source)?;
        (mo.x, mo.y, mo.z, mo.angle)
    };

    // --- Autoaim (draws NO RNG), exactly as vanilla P_SpawnPlayerMissile. ---
    const AUTOAIM_RANGE: Fixed16_16 = Fixed16_16(16 * 64 << 16);
    const SIDE_PROBE: u32 = 1 << 26;

    let mut an = base_angle.0;
    let mut aim = crate::combat::p_aim_line_attack(gs, source, Bam(an), AUTOAIM_RANGE, level);
    if aim.linetarget.is_none() {
        an = base_angle.0.wrapping_add(SIDE_PROBE);
        aim = crate::combat::p_aim_line_attack(gs, source, Bam(an), AUTOAIM_RANGE, level);
        if aim.linetarget.is_none() {
            // Vanilla does `an += 1<<26` then `an -= 2<<26`, i.e. base - 1<<26.
            an = base_angle.0.wrapping_add(SIDE_PROBE).wrapping_sub(2 * SIDE_PROBE);
            aim = crate::combat::p_aim_line_attack(gs, source, Bam(an), AUTOAIM_RANGE, level);
        }
        if aim.linetarget.is_none() {
            an = base_angle.0;
        }
    }
    let slope = aim.slope;
    let angle = Bam(an);

    // z = source->z + 4*8*FRACUNIT.
    let spawn_z = sz + Fixed16_16::from_int(32);

    // P_SpawnMobj: `mobj->lastlook = P_Random() % MAXPLAYERS`.
    let _lastlook = (gs.p_random() as u32) % 4;

    let speed_raw = info.speed.raw();
    let momx = Fixed16_16::from_raw(crate::geom::fixed_mul(speed_raw, crate::geom::fine_cosine(an)));
    let momy = Fixed16_16::from_raw(crate::geom::fixed_mul(speed_raw, crate::geom::fine_sine(an)));
    let momz = Fixed16_16::from_raw(crate::geom::fixed_mul(speed_raw, slope));

    let mut proj = Mobj::new(kind, sx, sy, angle);
    crate::spawn::apply_mobjinfo_defaults(&mut proj);
    proj.z = spawn_z;
    proj.radius = info.radius;
    proj.height = info.height;
    proj.health = 1000;
    proj.flags = info.flags;
    proj.momx = momx;
    proj.momy = momy;
    proj.momz = momz;
    proj.target = source;

    let handle = gs.mobjslab.alloc(proj);

    // P_CheckMissileSpawn: randomize the initial animation phase and nudge the
    // missile forward by half its momentum.
    let r = (gs.p_random() & 3) as i16;
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.tics -= r;
        if mo.tics < 1 {
            mo.tics = 1;
        }
        mo.x += Fixed16_16::from_raw(momx.raw() >> 1);
        mo.y += Fixed16_16::from_raw(momy.raw() >> 1);
        mo.z += Fixed16_16::from_raw(momz.raw() >> 1);
    }

    Some(handle)
}

// Missile MOVEMENT and COLLISION are no longer handled here. Vanilla advances
// each missile inside `P_MobjThinker` (swept `P_XYMovement` + `PIT_CheckThing`
// + `P_ExplodeMissile`, then `P_ZMovement`), so doom-rs drives missiles from
// `tick_mobj` in `tic.rs` alongside every other actor — see
// `p_xy_movement_missile` / `p_z_movement_missile` / `p_explode_missile` there.
// The old custom `p_move_projectiles` stepper (a single unswept momentum add
// with a z-less AABB actor test) was retired: it moved missiles a second time
// per tic and struck targets several tics off vanilla.

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

        let proj_h = p_spawn_player_missile(&mut gs, source, MobjKind::Rocket, None);
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
    fn player_missile_draws_two_p_randoms() {
        // Vanilla P_SpawnPlayerMissile draws exactly two P_Random values on the
        // shared playsim stream: the `P_SpawnMobj` `lastlook = P_Random() %
        // MAXPLAYERS` and the `P_CheckMissileSpawn` `tics -= P_Random()&3`. The
        // old stub drew none, dropping two draws on every rocket/plasma/BFG shot
        // and desyncing the RNG from the first shot (DEMO1 rndindex @ tic 1673).
        let mut gs = make_game_state();
        let source = gs.player.handle;

        let before = gs.rng.index();
        let proj_h = p_spawn_player_missile(&mut gs, source, MobjKind::Rocket, None);
        assert!(proj_h.is_some());
        let advanced = (gs.rng.index() + 256 - before) & 255;
        assert_eq!(
            advanced, 2,
            "player missile spawn must advance the P_Random index by exactly 2"
        );
    }

    #[test]
    fn player_missile_returns_none_for_non_projectile() {
        let mut gs = make_game_state();
        let source = gs.player.handle;

        let result = p_spawn_player_missile(&mut gs, source, MobjKind::Demon, None);
        assert!(result.is_none());
    }

    #[test]
    fn player_missile_spawns_at_chest_height() {
        let mut gs = make_game_state();
        let source = gs.player.handle;

        let proj_h = p_spawn_player_missile(&mut gs, source, MobjKind::PlasmaBall, None)
            .expect("value must exist in test");
        let proj = gs.mobjslab.get(proj_h).expect("value must exist in test");
        // Vanilla P_SpawnPlayerMissile spawns at source->z + 4*8*FRACUNIT.
        // Player z=0, so spawn z = 32. (momz is 0 with no autoaim target, so the
        // half-step nudge leaves z unchanged.)
        assert_eq!(proj.z, Fixed16_16::from_int(32));
    }

    #[test]
    fn player_missile_initializes_visible_spawn_state() {
        let mut gs = make_game_state();
        let source = gs.player.handle;

        let proj_h = p_spawn_player_missile(&mut gs, source, MobjKind::Rocket, None)
            .expect("value must exist in test");
        let proj = gs.mobjslab.get(proj_h).expect("value must exist in test");
        let info = &MOBJINFO[MobjKind::Rocket as usize];

        assert_eq!(info.spawn_state, StateNum(ids::S_ROCKET));
        assert_eq!(proj.state, info.spawn_state);
        assert_eq!(proj.tics, STATES[info.spawn_state.0 as usize].tics);
    }

    // -----------------------------------------------------------------------
    // p_spawn_missile momentum (vanilla fixed-point) regression tests
    // -----------------------------------------------------------------------

    /// Demo-sync regression: vanilla `P_SpawnMissile` aims with the fixed-point
    /// trig tables — `momx/momy = FixedMul(speed, finecosine/finesine[angle])`
    /// — so the momentum magnitude equals the projectile's speed. The old f32
    /// path truncated each component to a whole map unit, quantizing the aim and
    /// shrinking the magnitude (e.g. speed 10 -> (-4,8), |v|≈8.9), which drifted
    /// the projectile off vanilla's path and made it strike its target several
    /// tics early or late.
    #[test]
    fn spawn_missile_momentum_is_full_speed_fixed_point() {
        // SAFETY: trig tables are process-global and internally guarded.
        unsafe {
            doom_types::Bam::init_trig_tables();
        }
        let mut gs = make_game_state();
        let source = gs.player.handle;
        // Target off-axis so both momentum components are non-trivial.
        let target = spawn_target(&mut gs, 300, 700, 60);

        let proj_h = p_spawn_missile(&mut gs, source, target, MobjKind::ImpFireball)
            .expect("value must exist in test");
        let proj = gs.mobjslab.get(proj_h).expect("value must exist in test");

        // |momentum| must equal the imp fireball's speed (10 units/tic) within
        // fixed-point trig rounding — never the integer-truncated magnitude
        // (~8.9) the f32 path produced. Measure in RAW fixed-point (the whole
        // point of the fix is that the fractional bits are preserved).
        let mx = proj.momx.raw() as i64;
        let my = proj.momy.raw() as i64;
        let mag = ((mx * mx + my * my) as f64).sqrt();
        let speed_raw = Fixed16_16::from_int(10).raw() as f64; // 655360
        assert!(
            (mag - speed_raw).abs() < speed_raw * 0.01,
            "|momentum| = {mag} must be ~{speed_raw} (speed 10), got momx={mx} momy={my}"
        );
        // At least one component must carry a fractional part (proof the aim is
        // no longer quantized to whole units).
        assert!(
            proj.momx.raw() % 65536 != 0 || proj.momy.raw() % 65536 != 0,
            "vanilla fixed-point aim must produce fractional momentum, got \
             momx={:#x} momy={:#x}",
            proj.momx.raw(),
            proj.momy.raw()
        );
    }

    /// Demo-sync regression: vanilla `P_CheckMissileSpawn` nudges a freshly
    /// spawned missile forward by HALF its momentum (`x += momx>>1`) so it has a
    /// valid position/angle if it explodes immediately. Without this half-step
    /// every projectile trails vanilla by exactly half a step for its whole
    /// flight. Spawn position must therefore equal the shooter origin plus the
    /// half-momentum nudge (spawn z = source.z + 32, per vanilla).
    #[test]
    fn spawn_missile_applies_checkmissilespawn_half_step_nudge() {
        unsafe {
            doom_types::Bam::init_trig_tables();
        }
        let mut gs = make_game_state();
        let source = gs.player.handle;
        let (sx, sy, sz) = {
            let mo = gs.mobjslab.get(source).expect("source exists");
            (mo.x, mo.y, mo.z)
        };
        let target = spawn_target(&mut gs, 300, 700, 60);

        let proj_h = p_spawn_missile(&mut gs, source, target, MobjKind::ImpFireball)
            .expect("value must exist in test");
        let proj = gs.mobjslab.get(proj_h).expect("value must exist in test");

        // The spawn origin is the shooter position plus the half-momentum nudge
        // (`P_CheckMissileSpawn`), derived from the missile's own momentum.
        let expected_x = sx + Fixed16_16::from_raw(proj.momx.raw() >> 1);
        let expected_y = sy + Fixed16_16::from_raw(proj.momy.raw() >> 1);
        let expected_z = sz + Fixed16_16::from_int(32) + Fixed16_16::from_raw(proj.momz.raw() >> 1);
        assert_eq!(proj.x, expected_x, "spawn x must include the half-momentum nudge");
        assert_eq!(proj.y, expected_y, "spawn y must include the half-momentum nudge");
        assert_eq!(
            proj.z, expected_z,
            "spawn z must be source.z + 32 plus the half-momz nudge"
        );
    }

    #[test]
    fn spawn_missile_z_is_thirtytwo_above_source_before_nudge() {
        unsafe {
            doom_types::Bam::init_trig_tables();
        }
        let mut gs = make_game_state();
        let source = gs.player.handle;
        // Aim horizontally (target at same z) so momz == 0 and there is no z nudge.
        let target = spawn_target(&mut gs, 500, 0, 60);
        let proj_h = p_spawn_missile(&mut gs, source, target, MobjKind::Rocket)
            .expect("value must exist in test");
        let proj = gs.mobjslab.get(proj_h).expect("value must exist in test");
        // Source (player) z=0; vanilla spawn z = source.z + 4*8*FRACUNIT = 32.
        assert_eq!(proj.momz, Fixed16_16::ZERO, "level shot has no vertical momentum");
        assert_eq!(proj.z, Fixed16_16::from_int(32), "spawn z must be source.z + 32");
    }

    // -----------------------------------------------------------------------
    // Removed: the old `p_move_projectiles` movement tests. Missile movement is
    // now covered by `tic.rs` (`missile_*` tests), which exercise the vanilla
    // `P_XYMovement`/`PIT_CheckThing`/`P_ExplodeMissile` path.
    // -----------------------------------------------------------------------

}
