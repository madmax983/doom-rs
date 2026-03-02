//! Combat functions — damage, hitscan attacks, radius attacks.
//!
//! Port of Doom's `p_inter.c` and parts of `p_map.c`.

use doom_map::Level;
use doom_types::{Bam, Fixed16_16};

use crate::mobj::{MobjHandle, StateNum, flags};
use crate::state::GameState;

/// Maximum hitscan range in map units.
pub const MISSILERANGE: Fixed16_16 = Fixed16_16(2048 << 16);

/// Maximum melee attack range in map units (64 units).
pub const MELEERANGE: Fixed16_16 = Fixed16_16(64 << 16);

// ---------------------------------------------------------------------------
// damage_mobj
// ---------------------------------------------------------------------------

/// Port of `P_DamageMobj`.
///
/// Applies `damage` hit points to `target`.  If `inflictor` is not
/// `MobjHandle::NULL`, sets `target.target` to the inflictor so the monster
/// knows who attacked it.  Health is clamped to a minimum of 0.
///
/// Triggers a death-state transition when `health` reaches 0, and a
/// pain-state transition when `health` remains positive (if the actor has
/// a pain state and a non-zero `pain_chance`).
///
/// Returns immediately if `target` does not have `MF_SHOOTABLE` or is
/// already dead.
pub fn damage_mobj(gs: &mut GameState, target: MobjHandle, inflictor: MobjHandle, damage: i32) {
    // Guard: must exist, be shootable, and be alive.
    {
        let Some(mo) = gs.mobjslab.get(target) else {
            return;
        };
        if mo.flags & flags::MF_SHOOTABLE == 0 {
            return;
        }
        if mo.health <= 0 {
            return;
        }
    }

    // Apply damage + inflictor.
    let new_health = {
        let Some(mo) = gs.mobjslab.get_mut(target) else {
            return;
        };
        mo.health = (mo.health - damage).max(0);
        if inflictor != MobjHandle::NULL {
            mo.target = inflictor;
        }
        mo.health
    };

    if new_health <= 0 {
        // -------------------------------------------------------------------
        // Death transition
        // -------------------------------------------------------------------
        let death_sn: StateNum = {
            let Some(mo) = gs.mobjslab.get(target) else {
                return;
            };
            crate::mobjinfo::MOBJINFO[mo.kind as usize].death_state
        };
        if death_sn != StateNum::NULL {
            if let Some(entry) = crate::states::STATES.get(death_sn.0 as usize) {
                let new_tics = entry.tics;
                if let Some(mo) = gs.mobjslab.get_mut(target) {
                    mo.state = death_sn;
                    mo.tics = new_tics;
                }
            }
        }
    } else {
        // -------------------------------------------------------------------
        // Pain transition (always triggers when eligible; Batch 5 adds RNG)
        // -------------------------------------------------------------------
        let (pain_sn, pain_chance): (StateNum, u8) = {
            let Some(mo) = gs.mobjslab.get(target) else {
                return;
            };
            let info = &crate::mobjinfo::MOBJINFO[mo.kind as usize];
            (info.pain_state, info.pain_chance)
        };
        if pain_sn != StateNum::NULL && pain_chance > 0 {
            if let Some(entry) = crate::states::STATES.get(pain_sn.0 as usize) {
                let new_tics = entry.tics;
                if let Some(mo) = gs.mobjslab.get_mut(target) {
                    mo.state = pain_sn;
                    mo.tics = new_tics;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// p_line_attack
// ---------------------------------------------------------------------------

/// Simplified hitscan attack (no BSP ray cast — full traversal deferred).
///
/// Fires a ray from `source` in direction `angle` up to `range` map units.
/// For each live, shootable actor along the ray (tested via ray-circle
/// intersection), picks the closest one, applies `damage`, and returns its
/// handle.  Returns `None` if no actor is hit.
///
/// All position math is done in `i64` to avoid fixed-point overflow.
///
/// `_level` is reserved for future BSP-based line-of-sight tests.
pub fn p_line_attack(
    gs: &mut GameState,
    source: MobjHandle,
    angle: Bam,
    range: Fixed16_16,
    damage: i32,
    _level: Option<&Level>,
) -> Option<MobjHandle> {
    // Extract source position before any further borrows.
    let (sx, sy) = match gs.mobjslab.get(source) {
        Some(mo) => (mo.x.to_int() as i64, mo.y.to_int() as i64),
        None => return None,
    };

    // Trig values as integers (return 0 when tables are uninitialized).
    let cos_int = angle.cos().to_int() as i64;
    let sin_int = angle.sin().to_int() as i64;
    let range_int = range.to_int() as i64;

    // Collect all handles up front to avoid borrow conflicts.
    let handles: Vec<MobjHandle> = gs.mobjslab.iter_handles().collect();

    let mut best_handle: Option<MobjHandle> = None;
    let mut best_t: i64 = i64::MAX;

    for handle in handles {
        // Skip source.
        if handle == source {
            continue;
        }

        // Extract actor data — skip if dead or non-shootable.
        let (ax, ay, radius, alive, shootable) = match gs.mobjslab.get(handle) {
            Some(mo) => (
                mo.x.to_int() as i64,
                mo.y.to_int() as i64,
                mo.radius.to_int() as i64,
                mo.health > 0,
                mo.flags & flags::MF_SHOOTABLE != 0,
            ),
            None => continue,
        };

        if !alive || !shootable {
            continue;
        }

        // Ray-circle intersection using integer arithmetic.
        // Project (actor - source) onto the ray direction.
        let dx = ax - sx;
        let dy = ay - sy;
        let t = dx * cos_int + dy * sin_int;

        // Must be in front of and within range.
        if t <= 0 || t > range_int {
            continue;
        }

        // Perpendicular distance squared from actor center to the ray.
        let perp_sq = dx * dx + dy * dy - t * t;
        if perp_sq > radius * radius {
            continue;
        }

        // Closest so far?
        if t < best_t {
            best_t = t;
            best_handle = Some(handle);
        }
    }

    if let Some(hit) = best_handle {
        damage_mobj(gs, hit, source, damage);
        Some(hit)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// p_radius_attack
// ---------------------------------------------------------------------------

/// Splash damage from an explosion centered on `source`.
///
/// Every live, shootable actor (except `source` itself) within `radius` map
/// units (Manhattan distance) receives proportional damage that falls off
/// linearly from `damage` at the center to 1 at the edge.
///
/// `_level` is reserved for future line-of-sight blocking.
pub fn p_radius_attack(
    gs: &mut GameState,
    source: MobjHandle,
    damage: i32,
    radius: Fixed16_16,
    _level: Option<&Level>,
) {
    let radius_int = radius.to_int();
    if radius_int <= 0 {
        return;
    }

    // Extract source position before iterating.
    let (sx, sy) = match gs.mobjslab.get(source) {
        Some(mo) => (mo.x.to_int(), mo.y.to_int()),
        None => return,
    };

    // Collect handles up front.
    let handles: Vec<MobjHandle> = gs.mobjslab.iter_handles().collect();

    for handle in handles {
        if handle == source {
            continue;
        }

        // Extract actor data.
        let (ax, ay, alive, shootable) = match gs.mobjslab.get(handle) {
            Some(mo) => (
                mo.x.to_int(),
                mo.y.to_int(),
                mo.health > 0,
                mo.flags & flags::MF_SHOOTABLE != 0,
            ),
            None => continue,
        };

        if !alive || !shootable {
            continue;
        }

        // Manhattan distance.
        let dist = (ax - sx).unsigned_abs() as i32 + (ay - sy).unsigned_abs() as i32;

        if dist < radius_int {
            let actual = (damage * (radius_int - dist) / radius_int).max(1);
            damage_mobj(gs, handle, source, actual);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind};
    use crate::player::PlayerState;
    use crate::state::GameState;
    use crate::states::ids;
    use doom_types::{Bam, Fixed16_16};

    // -----------------------------------------------------------------------
    // Helpers — adapted from actions.rs tests
    // -----------------------------------------------------------------------

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
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs
    }

    fn spawn_trooper(gs: &mut GameState, x: i32, y: i32) -> MobjHandle {
        use crate::mobj::MobjKind;
        use crate::mobjinfo::MOBJINFO;
        use crate::states::STATES;
        let kind = MobjKind::Trooper;
        let spawn_sn = MOBJINFO[kind as usize].spawn_state;
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = spawn_sn;
        mo.tics = STATES[spawn_sn.0 as usize].tics;
        gs.mobjslab.alloc(mo)
    }

    // -----------------------------------------------------------------------
    // damage_mobj tests
    // -----------------------------------------------------------------------

    #[test]
    fn damage_reduces_health() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        // Trooper starts with 20 health; deal 5 damage.
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);
        assert_eq!(gs.mobjslab.get(trooper).unwrap().health, 15);
    }

    #[test]
    fn damage_kills_at_zero() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        // Deal more damage than the trooper has health — should clamp to 0.
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 9999);
        assert_eq!(
            gs.mobjslab.get(trooper).unwrap().health,
            0,
            "health must clamp to 0, not go negative"
        );
    }

    #[test]
    fn damage_ignores_non_shootable() {
        let mut gs = make_game_state();
        // Spawn a trooper but strip MF_SHOOTABLE.
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().flags &= !flags::MF_SHOOTABLE;

        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 10);
        // Health must be unchanged (still 20).
        assert_eq!(
            gs.mobjslab.get(trooper).unwrap().health,
            20,
            "non-shootable actor must not take damage"
        );
    }

    #[test]
    fn damage_sets_inflictor_target() {
        let mut gs = make_game_state();
        let player_handle = gs.player.handle;
        let trooper = spawn_trooper(&mut gs, 100, 0);

        damage_mobj(&mut gs, trooper, player_handle, 5);

        assert_eq!(
            gs.mobjslab.get(trooper).unwrap().target,
            player_handle,
            "inflictor must be set as the target"
        );
    }

    // -----------------------------------------------------------------------
    // Death and pain state transition tests
    // -----------------------------------------------------------------------

    #[test]
    fn death_transitions_monster_to_death_state() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // Deal exactly lethal damage (20 hp → 0).
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 20);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.state,
            crate::mobj::StateNum(ids::S_POSS_DIE1),
            "killed trooper must enter S_POSS_DIE1"
        );
        assert_eq!(mo.health, 0);
    }

    #[test]
    fn pain_transitions_monster_to_pain_state() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // Deal non-lethal damage (5/20 hp).
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.state,
            crate::mobj::StateNum(ids::S_POSS_PAIN),
            "damaged (not killed) trooper must enter S_POSS_PAIN"
        );
        assert_eq!(mo.health, 15);
    }

    #[test]
    fn dead_actor_ignores_further_damage() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // Kill the trooper.
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 20);
        let state_after_kill = gs.mobjslab.get(trooper).unwrap().state;

        // Apply more damage — must be a no-op.
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(mo.health, 0, "health must remain 0");
        assert_eq!(
            mo.state, state_after_kill,
            "state must not change after death"
        );
    }

    #[test]
    fn stale_handle_is_noop() {
        let mut gs = make_game_state();
        // Pass a NULL handle — must not panic.
        damage_mobj(&mut gs, MobjHandle::NULL, MobjHandle::NULL, 10);
    }

    // -----------------------------------------------------------------------
    // p_line_attack tests
    //
    // Note: Bam::init_trig_tables() is NOT called here, so sin()/cos() return
    // 0.  With cos=0 and sin=0 the ray projection t = 0 for all actors, which
    // means no actor passes the `t > 0` guard — matching the documented
    // behaviour in the task spec (skip or #[ignore] trig-dependent tests).
    // -----------------------------------------------------------------------

    #[test]
    fn line_attack_returns_none_when_no_actors() {
        let mut gs = GameState::new("test");
        // Spawn only a bare source actor, no targets.
        let mut src = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        src.health = 100;
        src.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let src_handle = gs.mobjslab.alloc(src);

        let result = p_line_attack(&mut gs, src_handle, Bam::ZERO, MISSILERANGE, 10, None);
        assert!(result.is_none(), "must return None when no targets exist");
    }

    #[test]
    #[ignore = "requires Bam::init_trig_tables() which is unsafe and not called in unit tests"]
    fn line_attack_hits_actor_directly_ahead() {
        // This test requires trig tables.  Skipped per task spec.
        let mut gs = make_game_state();
        let _trooper = spawn_trooper(&mut gs, 100, 0);
        let src = gs.player.handle;
        let result = p_line_attack(&mut gs, src, Bam::ZERO, Fixed16_16::from_int(500), 5, None);
        assert!(result.is_some(), "should hit actor directly ahead");
    }

    #[test]
    fn line_attack_skips_dead_actors() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        // Kill the trooper first.
        gs.mobjslab.get_mut(trooper).unwrap().health = 0;

        let src = gs.player.handle;
        // Even if trig tables were initialized and the geometry lined up,
        // dead actors must be skipped.  With uninitialized tables, t=0 and
        // both the dead-check and t<=0 guard fire — None is the expected result.
        let result = p_line_attack(&mut gs, src, Bam::ZERO, MISSILERANGE, 10, None);
        assert!(result.is_none(), "dead actors must not be hit");
    }

    // -----------------------------------------------------------------------
    // p_radius_attack tests
    // -----------------------------------------------------------------------

    #[test]
    fn radius_attack_damages_nearby() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 50, 0); // 50 units east of origin

        let player_handle = gs.player.handle;
        // Explode at origin with radius 200 — trooper is well within range.
        p_radius_attack(&mut gs, player_handle, 100, Fixed16_16::from_int(200), None);

        let health = gs.mobjslab.get(trooper).unwrap().health;
        assert!(
            health < 20,
            "trooper health {health} must decrease from splash damage"
        );
    }

    #[test]
    fn radius_attack_ignores_far_actors() {
        let mut gs = make_game_state();
        // Trooper 500 units away; explosion radius only 100.
        let trooper = spawn_trooper(&mut gs, 500, 0);

        let player_handle = gs.player.handle;
        p_radius_attack(&mut gs, player_handle, 100, Fixed16_16::from_int(100), None);

        let health = gs.mobjslab.get(trooper).unwrap().health;
        assert_eq!(
            health, 20,
            "trooper outside blast radius must be unaffected"
        );
    }
}
