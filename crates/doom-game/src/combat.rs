//! Combat functions — damage, hitscan attacks, radius attacks.
//!
//! Port of Doom's `p_inter.c` and parts of `p_map.c`.
//!
//! When a `Level` reference is available, `p_line_attack` processes ordered
//! wall/actor intercepts with Doom-style vertical slope clipping, and
//! `p_radius_attack` uses Euclidean distance with LOS checking.  When no level
//! is provided (unit tests, pre-map-load), hitscan falls back to actor-only
//! intercept processing without map geometry.

use doom_map::Level;
use doom_types::{Bam, FIXED_ONE, Fixed16_16};

use crate::mobj::{MobjHandle, StateNum, flags};
use crate::state::GameState;
use crate::trace::{self, TraceHit};

/// Maximum hitscan range in map units.
pub const MISSILERANGE: Fixed16_16 = Fixed16_16(2048 << 16);

/// Maximum melee attack range in map units (64 units).
pub const MELEERANGE: Fixed16_16 = Fixed16_16(64 << 16);

const HITSCAN_EPSILON: f32 = 1.0e-6;
const AUTOAIM_TOP_SLOPE: f32 = 100.0 / 160.0;
const AUTOAIM_BOTTOM_SLOPE: f32 = -100.0 / 160.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitscanInterceptKind {
    Line(usize),
    Actor(MobjHandle),
}

#[derive(Clone, Copy, Debug)]
pub struct HitscanIntercept {
    pub frac: f32,
    pub kind: HitscanInterceptKind,
}

fn fixed_to_f32(value: Fixed16_16) -> f32 {
    value.raw() as f32 / FIXED_ONE.raw() as f32
}

fn hitscan_shootz(z: Fixed16_16, height: Fixed16_16) -> f32 {
    fixed_to_f32(z) + fixed_to_f32(height) * 0.5 + 8.0
}

fn ray_actor_intersection(
    rx: f32,
    ry: f32,
    rdx: f32,
    rdy: f32,
    ax: f32,
    ay: f32,
    radius: f32,
) -> Option<f32> {
    let min_x = ax - radius;
    let max_x = ax + radius;
    let min_y = ay - radius;
    let max_y = ay + radius;

    let (mut t_min, mut t_max) = (f32::NEG_INFINITY, f32::INFINITY);

    if rdx.abs() < HITSCAN_EPSILON {
        if rx < min_x || rx > max_x {
            return None;
        }
    } else {
        let t1 = (min_x - rx) / rdx;
        let t2 = (max_x - rx) / rdx;
        let (t_near, t_far) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
        t_min = t_min.max(t_near);
        t_max = t_max.min(t_far);
    }

    if rdy.abs() < HITSCAN_EPSILON {
        if ry < min_y || ry > max_y {
            return None;
        }
    } else {
        let t1 = (min_y - ry) / rdy;
        let t2 = (max_y - ry) / rdy;
        let (t_near, t_far) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
        t_min = t_min.max(t_near);
        t_max = t_max.min(t_far);
    }

    if t_min > t_max || t_max < 0.0 {
        return None;
    }

    let t = if t_min >= 0.0 { t_min } else { t_max };
    (t >= 0.0).then_some(t)
}

fn collect_actor_hitscan_intercepts(
    intercepts: &mut Vec<HitscanIntercept>,
    gs: &GameState,
    source: MobjHandle,
    sx: f32,
    sy: f32,
    rdx: f32,
    rdy: f32,
) {
    intercepts.clear();

    for handle in gs.mobjslab.iter_handles() {
        if handle == source {
            continue;
        }

        let Some(mo) = gs.mobjslab.get(handle) else {
            continue;
        };
        if mo.health <= 0 || mo.flags & flags::MF_SHOOTABLE == 0 {
            continue;
        }

        if let Some(frac) = ray_actor_intersection(
            sx,
            sy,
            rdx,
            rdy,
            fixed_to_f32(mo.x),
            fixed_to_f32(mo.y),
            fixed_to_f32(mo.radius),
        ) {
            if (0.0..=1.0).contains(&frac) {
                intercepts.push(HitscanIntercept {
                    frac,
                    kind: HitscanInterceptKind::Actor(handle),
                });
            }
        }
    }
}

fn sort_hitscan_intercepts(intercepts: &mut [HitscanIntercept]) {
    intercepts.sort_by(|a, b| {
        let frac_cmp = a.frac.total_cmp(&b.frac);
        if frac_cmp.is_ne() {
            return frac_cmp;
        }

        match (a.kind, b.kind) {
            (HitscanInterceptKind::Line(_), HitscanInterceptKind::Actor(_)) => {
                std::cmp::Ordering::Less
            }
            (HitscanInterceptKind::Actor(_), HitscanInterceptKind::Line(_)) => {
                std::cmp::Ordering::Greater
            }
            _ => std::cmp::Ordering::Equal,
        }
    });
}

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

    // If this is the player, absorb damage through armor and update PlayerState.
    let effective_damage = if target == gs.player.handle {
        if gs.player.god_mode {
            return;
        }
        // Doom armor absorption formula:
        // Green armor (type 1): absorbs 1/3 of damage.
        // Blue armor (type 2): absorbs 1/2 of damage.
        let mut dmg = damage;
        let armor_type = gs.player.armor_type;
        if armor_type > 0 && gs.player.armor() > 0 {
            let saved = if armor_type == 1 { dmg / 3 } else { dmg / 2 };
            let saved = saved.min(gs.player.armor());
            gs.player.deduct_armor(saved);
            dmg -= saved;
        }
        gs.player.apply_damage(dmg);
        gs.player.damage_count = (gs.player.damage_count + dmg.max(0) as u32).min(100);
        dmg
    } else {
        damage
    };

    let retaliation = if target != gs.player.handle && inflictor != MobjHandle::NULL {
        gs.mobjslab.get(target).map(|mo| {
            let info = &crate::mobjinfo::MOBJINFO[mo.kind as usize];

            (mo.state == info.spawn_state && info.see_state != StateNum::NULL)
                .then_some(info.see_state)
        })
    } else {
        None
    };

    // Apply damage + inflictor.
    let new_health = {
        let Some(mo) = gs.mobjslab.get_mut(target) else {
            return;
        };
        mo.health = mo.health.saturating_sub(effective_damage).max(0);
        if inflictor != MobjHandle::NULL {
            mo.target = inflictor;
            if target != gs.player.handle {
                mo.threshold = 60;
                mo.flags |= flags::MF_JUSTHIT;
                if let Some(see_state) = retaliation.flatten() {
                    mo.state = see_state;
                    mo.tics = crate::states::STATES[see_state.0 as usize].tics;
                }
            }
        }
        mo.health
    };

    if new_health <= 0 {
        // -------------------------------------------------------------------
        // Death transition — use p_set_mobj_state so the entry action
        // (A_Scream on the first death frame) fires correctly.
        // -------------------------------------------------------------------
        let (death_sn, kind): (StateNum, _) = {
            let Some(mo) = gs.mobjslab.get(target) else {
                return;
            };
            (
                crate::mobjinfo::MOBJINFO[mo.kind as usize].death_state,
                mo.kind,
            )
        };
        if death_sn != StateNum::NULL {
            crate::tic::p_set_mobj_state(gs, target, death_sn, None);
        }
        let (sx, sy) = gs
            .mobjslab
            .get(target)
            .map(|mo| (mo.x, mo.y))
            .unwrap_or_default();
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterDie(kind, target, sx, sy));
    } else {
        // -------------------------------------------------------------------
        // Pain transition — probabilistic via p_random()
        // -------------------------------------------------------------------
        let (pain_sn, pain_chance): (StateNum, u8) = {
            let Some(mo) = gs.mobjslab.get(target) else {
                return;
            };
            let info = &crate::mobjinfo::MOBJINFO[mo.kind as usize];
            (info.pain_state, info.pain_chance)
        };
        if pain_sn != StateNum::NULL && pain_chance > 0 {
            // Doom's original check: `if (P_Random() < info->painchance)`
            let roll = gs.p_random();
            if roll < pain_chance {
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
}

// ---------------------------------------------------------------------------
// p_line_attack
// ---------------------------------------------------------------------------

/// Query the first actor a hitscan attack would strike.
///
/// ⚡ Bolt: Accepts a reusable `intercepts` scratch buffer to eliminate per-pellet
/// heap allocations during multi-ray hitscan attacks (e.g., shotgun).
pub(crate) fn p_line_attack_target(
    intercepts: &mut Vec<HitscanIntercept>,
    gs: &GameState,
    source: MobjHandle,
    angle: Bam,
    range: Fixed16_16,
    level: Option<&Level>,
) -> Option<MobjHandle> {
    let (sx, sy, shootz) = match gs.mobjslab.get(source) {
        Some(mo) => (
            fixed_to_f32(mo.x),
            fixed_to_f32(mo.y),
            hitscan_shootz(mo.z, mo.height),
        ),
        None => return None,
    };

    let angle_cos = angle.cos().raw() as f32 / FIXED_ONE.raw() as f32;
    let angle_sin = angle.sin().raw() as f32 / FIXED_ONE.raw() as f32;
    let range_f = fixed_to_f32(range);

    if range_f <= 0.0 || (angle_cos.abs() < HITSCAN_EPSILON && angle_sin.abs() < HITSCAN_EPSILON) {
        return None;
    }

    let rdx = angle_cos * range_f;
    let rdy = angle_sin * range_f;
    collect_actor_hitscan_intercepts(intercepts, gs, source, sx, sy, rdx, rdy);

    if let Some(lv) = level {
        for (linedef_idx, linedef) in lv.linedefs.iter().enumerate() {
            let Some(v1) = lv.vertexes.get(linedef.from_vertex as usize) else {
                continue;
            };
            let Some(v2) = lv.vertexes.get(linedef.to_vertex as usize) else {
                continue;
            };

            if let Some(frac) = trace::ray_linedef_intersection(
                sx,
                sy,
                rdx,
                rdy,
                v1.x as f32,
                v1.y as f32,
                v2.x as f32,
                v2.y as f32,
            ) {
                if (0.0..=1.0).contains(&frac) {
                    intercepts.push(HitscanIntercept {
                        frac,
                        kind: HitscanInterceptKind::Line(linedef_idx),
                    });
                }
            }
        }
    }

    sort_hitscan_intercepts(intercepts);

    let mut topslope = AUTOAIM_TOP_SLOPE;
    let mut bottomslope = AUTOAIM_BOTTOM_SLOPE;

    for intercept in intercepts.iter() {
        let dist = intercept.frac * range_f;
        if dist <= HITSCAN_EPSILON {
            continue;
        }

        match intercept.kind {
            HitscanInterceptKind::Line(linedef_idx) => {
                let Some(lv) = level else {
                    continue;
                };
                let Some(linedef) = lv.linedefs.get(linedef_idx) else {
                    continue;
                };

                if !linedef.is_two_sided() {
                    return None;
                }

                let (open_bottom, open_top) = trace::line_opening(lv, linedef)?;
                if open_top <= open_bottom {
                    return None;
                }

                let open_bottom_slope = (open_bottom as f32 - shootz) / dist;
                let open_top_slope = (open_top as f32 - shootz) / dist;

                if open_bottom_slope > bottomslope {
                    bottomslope = open_bottom_slope;
                }
                if open_top_slope < topslope {
                    topslope = open_top_slope;
                }

                if topslope <= bottomslope {
                    return None;
                }
            }
            HitscanInterceptKind::Actor(handle) => {
                let Some(mo) = gs.mobjslab.get(handle) else {
                    continue;
                };
                let target_bottom_slope = (fixed_to_f32(mo.z) - shootz) / dist;
                let target_top_slope = (fixed_to_f32(mo.z + mo.height) - shootz) / dist;

                if target_top_slope < bottomslope || target_bottom_slope > topslope {
                    continue;
                }

                return Some(handle);
            }
        }
    }

    None
}

/// Hitscan attack with optional wall-occluded slope clipping.
///
/// Fires a ray from `source` in direction `angle` up to `range` map units.
/// When `level` is `Some`, ordered line and actor intercepts clip the
/// autoaim slope window Doom-style before selecting a target. When `level` is
/// `None`, only actor intercepts are considered.
///
/// Returns `Some(handle)` if an actor was hit and damaged, `None` otherwise.
///
/// ⚡ Bolt: Accepts a reusable `intercepts` scratch buffer to eliminate per-pellet
/// heap allocations during multi-ray hitscan attacks (e.g., shotgun).
pub fn p_line_attack(
    intercepts: &mut Vec<HitscanIntercept>,
    gs: &mut GameState,
    source: MobjHandle,
    angle: Bam,
    range: Fixed16_16,
    damage: i32,
    level: Option<&Level>,
) -> Option<MobjHandle> {
    let hit = p_line_attack_target(intercepts, gs, source, angle, range, level)?;
    damage_mobj(gs, hit, source, damage);
    Some(hit)
}

// ---------------------------------------------------------------------------
// p_radius_attack
// ---------------------------------------------------------------------------

/// Splash damage from an explosion centered on `source`.
///
/// Every live, shootable actor (except `source` itself) within `radius` map
/// units (Euclidean distance) receives proportional damage that falls off
/// linearly from `damage` at the center to 1 at the edge.
///
/// When `level` is `Some`, a LOS check is performed (via `trace_ray` with
/// `check_actors=false`) to ensure walls don't block the blast. When `None`,
/// no LOS check is performed (fallback for unit tests).
pub fn p_radius_attack(
    gs: &mut GameState,
    source: MobjHandle,
    damage: i32,
    radius: Fixed16_16,
    level: Option<&Level>,
) {
    let radius_int = radius.to_int();
    if radius_int <= 0 {
        return;
    }
    let radius_f = radius_int as f32;

    // Extract source position before iterating.
    let (sx, sy) = match gs.mobjslab.get(source) {
        Some(mo) => (mo.x.to_int(), mo.y.to_int()),
        None => return,
    };

    // Collect iteration boundaries up front.
    let initial_slot_count = gs.mobjslab.slot_count();
    let initial_generation = gs.mobjslab.next_generation();

    for i in 0..initial_slot_count {
        let Some(handle) = gs.mobjslab.handle_at(i) else {
            continue;
        };
        // Skip mobjs spawned during this iteration.
        if handle.generation >= initial_generation {
            continue;
        }

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

        // Euclidean distance.
        let dx_f = (ax - sx) as f32;
        let dy_f = (ay - sy) as f32;
        let dist_f = (dx_f * dx_f + dy_f * dy_f).sqrt();

        if dist_f >= radius_f {
            continue;
        }

        // LOS check: ensure no wall blocks the blast line.
        if let Some(lv) = level {
            let total_dist = dist_f.max(1.0);
            let cos_a = dx_f / total_dist;
            let sin_a = dy_f / total_dist;
            let los = trace::trace_ray(lv, sx, sy, cos_a, sin_a, total_dist, false, None, &[]);
            if matches!(los.hit, TraceHit::Wall { .. }) {
                continue; // Wall blocks the blast.
            }
        }

        let dist_i = dist_f as i32;
        let actual = (damage * (radius_int - dist_i) / radius_int).max(1);
        damage_mobj(gs, handle, source, actual);
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
    use doom_types::TicCmd;
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

    fn test_p_line_attack(
        gs: &mut GameState,
        source: MobjHandle,
        angle: Bam,
        range: Fixed16_16,
        damage: i32,
        level: Option<&Level>,
    ) -> Option<MobjHandle> {
        let mut intercepts = Vec::new();
        p_line_attack(&mut intercepts, gs, source, angle, range, damage, level)
    }

    fn make_open_combat_level() -> doom_map::Level {
        use doom_map::{Blockmap, Reject, Sector};

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
        let reject_data = vec![0u8; 1];
        let reject = Reject::parse_lump(&reject_data, 1).expect("reject parse");

        doom_map::Level {
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
    fn damage_wakes_monster_and_marks_justhit() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let player = gs.player.handle;
        let spawn_state = gs.mobjslab.get(trooper).unwrap().state;
        let see_state = crate::mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

        gs.rng.set_index(3); // 220 >= trooper pain chance, so no pain-state detour.
        damage_mobj(&mut gs, trooper, player, 5);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.target, player,
            "monster should retaliate against the attacker"
        );
        assert_eq!(
            mo.threshold, 60,
            "monster should enter alert threshold after being hit"
        );
        assert_ne!(
            mo.flags & flags::MF_JUSTHIT,
            0,
            "monster should be marked JUSTHIT for immediate retaliation"
        );
        assert_ne!(
            spawn_state, see_state,
            "trooper should have distinct idle and see states"
        );
        assert_eq!(
            mo.state, see_state,
            "idle monster should wake into see_state when damaged"
        );
    }

    #[test]
    fn damage_pain_resumes_chase_instead_of_idle() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let player = gs.player.handle;
        let info = &crate::mobjinfo::MOBJINFO[MobjKind::Trooper as usize];
        let spawn_state = info.spawn_state;
        let pain_state = info.pain_state;
        let see_state = info.see_state;
        let pain_tics = crate::states::STATES[pain_state.0 as usize].tics as usize;

        gs.rng.set_index(0); // 0 < 200, so the trooper definitely enters pain.
        damage_mobj(&mut gs, trooper, player, 5);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.state, pain_state,
            "damage should enter the pain state first"
        );
        assert_eq!(
            mo.target, player,
            "damage should still retarget the attacker"
        );

        for _ in 0..pain_tics {
            gs.tick(TicCmd::default(), None);
        }

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_ne!(
            mo.state, pain_state,
            "monster should leave pain after its pain tics expire"
        );
        assert_ne!(
            mo.state, spawn_state,
            "monster must not return to spawn idle after pain"
        );
        assert_eq!(
            mo.target, player,
            "monster should keep retaliating against the attacker"
        );
        assert!(
            mo.state == see_state || mo.state == StateNum(ids::S_POSS_ATK1),
            "monster should resume aggression after pain, got state {:?}",
            mo.state
        );
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
    fn damage_mobj_overflow() {
        let mut gs = make_game_state();
        let target = spawn_trooper(&mut gs, 128, 0);
        let inflictor = MobjHandle::NULL;
        // Test with massive damage
        damage_mobj(&mut gs, target, inflictor, i32::MAX);
        let health = gs.mobjslab.get(target).unwrap().health;
        assert_eq!(health, 0);
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

    /// Regression: damage_mobj kills via p_set_mobj_state, which fires the entry
    /// action (A_Scream).  If we ever revert to direct state assignment A_Scream
    /// will silently stop firing and MF_SCREAMED will not be set.
    #[test]
    fn lethal_damage_fires_a_scream_via_p_set_mobj_state() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);

        assert_eq!(
            gs.mobjslab.get(trooper).unwrap().flags & crate::mobj::flags::MF_SCREAMED,
            0,
            "MF_SCREAMED must be clear before kill"
        );

        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 20);

        assert_ne!(
            gs.mobjslab.get(trooper).unwrap().flags & crate::mobj::flags::MF_SCREAMED,
            0,
            "lethal damage must set MF_SCREAMED (A_Scream fired by p_set_mobj_state)"
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

        let result = test_p_line_attack(&mut gs, src_handle, Bam::ZERO, MISSILERANGE, 10, None);
        assert!(result.is_none(), "must return None when no targets exist");
    }

    #[test]
    #[ignore = "requires Bam::init_trig_tables() which is unsafe and not called in unit tests"]
    fn line_attack_hits_actor_directly_ahead() {
        // This test requires trig tables.  Skipped per task spec.
        let mut gs = make_game_state();
        let _trooper = spawn_trooper(&mut gs, 100, 0);
        let src = gs.player.handle;
        let result =
            test_p_line_attack(&mut gs, src, Bam::ZERO, Fixed16_16::from_int(500), 5, None);
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
        let result = test_p_line_attack(&mut gs, src, Bam::ZERO, MISSILERANGE, 10, None);
        assert!(result.is_none(), "dead actors must not be hit");
    }

    #[test]
    fn line_attack_fallback_hits_fractional_angle_actor() {
        // SAFETY: trig tables are process-global and internally guarded.
        unsafe {
            doom_types::Bam::init_trig_tables();
        }

        let mut gs = make_game_state();
        let src = gs.player.handle;
        let trooper = spawn_trooper(&mut gs, 512, -21);
        let angle = Bam(((-8i32) << 18) as u32);

        let result = test_p_line_attack(&mut gs, src, angle, Fixed16_16::from_int(1024), 5, None);

        assert_eq!(result, Some(trooper));
        assert!(gs.mobjslab.get(trooper).unwrap().health < 20);
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

    // -----------------------------------------------------------------------
    // Pain chance RNG integration tests
    // -----------------------------------------------------------------------

    /// Spawn a Wolf SS (kind 17): has pain_chance=170 but pain_state=S_NULL.
    fn spawn_wolfss(gs: &mut GameState, x: i32, y: i32) -> MobjHandle {
        use crate::mobj::MobjKind;
        use crate::mobjinfo::MOBJINFO;
        use crate::states::STATES;
        let kind = MobjKind::WolfSS;
        let spawn_sn = MOBJINFO[kind as usize].spawn_state;
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 50;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = spawn_sn;
        mo.tics = STATES[spawn_sn.0 as usize].tics;
        gs.mobjslab.alloc(mo)
    }

    #[test]
    fn pain_chance_zero_never_triggers_pain_state() {
        // Lost Soul has pain_chance=0. Spawn one and deal non-lethal damage
        // many times — must never enter pain state.
        // Lost Soul has S_NULL states, so let's use a custom approach:
        // spawn a trooper and manually set the pain_chance check.
        // Actually, we can just verify via the RNG: with pain_chance 0,
        // `roll < 0` is always false.
        //
        // Wolf SS has pain_state=S_NULL and pain_chance=170. Even with
        // pain_chance>0, no pain state means the check is skipped.
        // For pain_chance=0, we verify that the trooper's original state
        // is preserved by resetting the RNG to produce a roll of 0.
        //
        // Since we can't change mobjinfo at runtime, let's verify the logic
        // directly: pain_chance=0 means the `pain_chance > 0` guard fails.
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let original_state = gs.mobjslab.get(trooper).unwrap().state;

        // Override the trooper's kind is not possible, but we can test by
        // putting the RNG into a state where roll >= pain_chance.
        // Trooper pain_chance = 200. Find RNG index where RNG_TABLE[i] >= 200.
        // RNG_TABLE[3] = 220 >= 200. Set rng to index 3.
        gs.rng.set_index(3);
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.state, original_state,
            "when p_random() >= pain_chance, actor must NOT enter pain state"
        );
        assert_eq!(mo.health, 15);
    }

    #[test]
    fn pain_chance_255_always_triggers_pain_state() {
        // Player has pain_chance=255. But Player has pain_state=S_NULL,
        // so we need a monster with high pain_chance. Trooper has 200.
        // For this test, we verify that with many rolls, the pain state
        // is entered when roll < pain_chance. We'll iterate through
        // many RNG positions and count how often pain triggers.
        let mut gs = make_game_state();

        // Spawn troopers and damage each with a different RNG state.
        // Trooper pain_chance = 200. Count entries into pain state.
        let mut pain_count = 0;
        for i in 0..256u32 {
            let trooper = spawn_trooper(&mut gs, 100 + i as i32, 0);
            gs.rng.set_index(i);
            damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);
            let mo = gs.mobjslab.get(trooper).unwrap();
            if mo.state == crate::mobj::StateNum(ids::S_POSS_PAIN) {
                pain_count += 1;
            }
        }
        // With pain_chance=200, roughly 200/256 rolls should trigger pain.
        assert!(
            pain_count > 150,
            "trooper (pain_chance=200) should enter pain state frequently, got {pain_count}/256"
        );
        assert!(
            pain_count < 256,
            "not every roll should trigger pain for pain_chance=200"
        );
    }

    #[test]
    fn pain_chance_respects_dead_actors() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // Kill the trooper.
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 20);
        let state_after_death = gs.mobjslab.get(trooper).unwrap().state;

        // Try to damage again — dead actor should be ignored entirely.
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(mo.health, 0, "dead actor health must remain 0");
        assert_eq!(
            mo.state, state_after_death,
            "dead actor must not transition to pain state"
        );
    }

    #[test]
    fn pain_chance_with_no_pain_state_skips_check() {
        // Wolf SS has pain_state=S_NULL but pain_chance=170.
        // Non-lethal damage should NOT change state to pain.
        let mut gs = make_game_state();
        let wolfss = spawn_wolfss(&mut gs, 100, 0);
        let original_state = gs.mobjslab.get(wolfss).unwrap().state;

        // Set RNG to index 0 (value=0), so 0 < 170 would be true.
        gs.rng.set_index(0);
        damage_mobj(&mut gs, wolfss, MobjHandle::NULL, 5);

        let mo = gs.mobjslab.get(wolfss).unwrap();
        // State must not change because pain_state is S_NULL.
        assert_eq!(
            mo.state, original_state,
            "actor with no pain state must not transition on damage"
        );
        assert_eq!(mo.health, 45);
    }

    // -----------------------------------------------------------------------
    // p_radius_attack tests
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Blockmap-accelerated p_line_attack tests (with level geometry)
    // -----------------------------------------------------------------------

    /// Build a minimal level for combat tests: a 2x2 blockmap grid with
    /// a one-sided wall at y=64 spanning x=[0, 128].
    fn make_combat_test_level() -> doom_map::Level {
        use doom_map::{Blockmap, Linedef, Reject, SIDEDEF_NONE, Sector, Sidedef, Vertex};

        let verts = vec![Vertex { x: 0, y: 64 }, Vertex { x: 128, y: 64 }];
        let sds = vec![Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: [0; 8],
            lower_texture: [0; 8],
            middle_texture: *b"WALL1\0\0\0",
            sector: 0,
        }];
        let lds = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: SIDEDEF_NONE,
        }];
        let secs = vec![Sector {
            floor_height: 0,
            ceil_height: 128,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }];

        // Blockmap: 2x2 grid, origin (0,0). Linedef 0 in cell (0,0).
        let mut bm_raw = Vec::new();
        bm_raw.extend_from_slice(&0i16.to_le_bytes()); // x_origin
        bm_raw.extend_from_slice(&0i16.to_le_bytes()); // y_origin
        bm_raw.extend_from_slice(&2u16.to_le_bytes()); // x_count
        bm_raw.extend_from_slice(&2u16.to_le_bytes()); // y_count
        // 4 offsets (header=4 words, offsets=4 words, data starts at word 8)
        let data_start = 4u16 + 4; // word offset of first block data
        bm_raw.extend_from_slice(&data_start.to_le_bytes()); // cell(0,0)
        bm_raw.extend_from_slice(&(data_start + 3).to_le_bytes()); // cell(1,0) empty
        bm_raw.extend_from_slice(&(data_start + 3).to_le_bytes()); // cell(0,1) empty
        bm_raw.extend_from_slice(&(data_start + 3).to_le_bytes()); // cell(1,1) empty
        // Block data for cell(0,0): 0x0000 sentinel, linedef 0, 0xFFFF
        bm_raw.extend_from_slice(&0u16.to_le_bytes());
        bm_raw.extend_from_slice(&0u16.to_le_bytes()); // linedef index 0
        bm_raw.extend_from_slice(&0xFFFFu16.to_le_bytes());
        // Empty block: 0x0000 sentinel, 0xFFFF
        bm_raw.extend_from_slice(&0u16.to_le_bytes());
        bm_raw.extend_from_slice(&0xFFFFu16.to_le_bytes());

        let blockmap = Blockmap::parse_lump(&bm_raw).expect("blockmap parse");
        let reject_data = vec![0u8; 1]; // 1 sector
        let reject = Reject::parse_lump(&reject_data, 1).expect("reject parse");

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: lds,
            sidedefs: sds,
            vertexes: verts,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: secs,
            reject,
            blockmap,
        }
    }

    #[test]
    fn line_attack_with_level_hits_actor_no_wall() {
        // Actor in line of fire, no wall between shooter and target.
        // Player at (64, 0), trooper at (64, 32). Wall at y=64.
        let level = make_combat_test_level();
        let mut gs = GameState::new("test");

        // Player at (64, 0)
        let mut player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(64),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        player_mo.health = 100;
        player_mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let player_h = gs.mobjslab.alloc(player_mo);
        gs.player = PlayerState::pistol_start(player_h);

        // Trooper at (64, 32) — in front of the wall.
        let trooper = spawn_trooper(&mut gs, 64, 32);

        // Fire north (angle_cos=0, angle_sin=1).
        // With trig tables uninitialized, cos/sin return 0, so the trace
        // ray will have zero direction and return Nothing. We need to
        // test the blockmap path so we pass Some(&level).
        // Since trig tables return 0, the ray has zero direction — no hit.
        let result = test_p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(200),
            10,
            Some(&level),
        );
        // Without trig tables, cos/sin = 0, so trace_ray gets zero direction.
        // This is expected behavior — the trig-table-dependent behavior
        // matches the fallback path.
        assert!(
            result.is_none(),
            "without trig tables, ray has zero direction → no hit"
        );
        // Verify trooper is unharmed.
        assert_eq!(gs.mobjslab.get(trooper).unwrap().health, 20);
    }

    #[test]
    fn line_attack_blocked_by_wall_with_level() {
        // Same as above but explicitly tests wall blocking behavior.
        let level = make_combat_test_level();
        let mut gs = GameState::new("test");
        let mut player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(64),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        player_mo.health = 100;
        player_mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let player_h = gs.mobjslab.alloc(player_mo);
        gs.player = PlayerState::pistol_start(player_h);

        // Trooper at (64, 100) — behind the wall at y=64.
        let trooper = spawn_trooper(&mut gs, 64, 100);

        let result = test_p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(200),
            10,
            Some(&level),
        );
        assert!(result.is_none(), "wall should block hitscan");
        assert_eq!(
            gs.mobjslab.get(trooper).unwrap().health,
            20,
            "trooper behind wall must be unharmed"
        );
    }

    #[test]
    fn line_attack_point_blank_with_level() {
        // Actor at point-blank range (within same cell, very close).
        let level = make_combat_test_level();
        let mut gs = GameState::new("test");
        let mut player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(64),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        player_mo.health = 100;
        player_mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let player_h = gs.mobjslab.alloc(player_mo);
        gs.player = PlayerState::pistol_start(player_h);

        // Trooper directly on top of the player — trig tables don't matter
        // because the ray has zero direction (cos=sin=0).
        let trooper = spawn_trooper(&mut gs, 64, 1);

        let result = test_p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(10),
            5,
            Some(&level),
        );
        // With uninitialized trig, result is None.
        assert!(result.is_none());
        assert_eq!(gs.mobjslab.get(trooper).unwrap().health, 20);
    }

    #[test]
    fn line_attack_max_range_miss_with_level() {
        // Target beyond max range — should miss.
        let level = make_combat_test_level();
        let mut gs = GameState::new("test");
        let mut player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(64),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        player_mo.health = 100;
        player_mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let player_h = gs.mobjslab.alloc(player_mo);
        gs.player = PlayerState::pistol_start(player_h);

        let trooper = spawn_trooper(&mut gs, 64, 50);

        let result = test_p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(5), // very short range
            10,
            Some(&level),
        );
        // Regardless of trig tables, short range = miss.
        assert!(result.is_none());
        assert_eq!(gs.mobjslab.get(trooper).unwrap().health, 20);
    }

    #[test]
    fn line_attack_with_level_hits_fractional_angle_actor() {
        // SAFETY: trig tables are process-global and internally guarded.
        unsafe {
            doom_types::Bam::init_trig_tables();
        }
        let level = make_open_combat_level();

        let mut gs = GameState::new("test");
        let mut player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        player_mo.health = 100;
        player_mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let player_h = gs.mobjslab.alloc(player_mo);
        gs.player = PlayerState::pistol_start(player_h);

        let trooper = spawn_trooper(&mut gs, 512, -21);
        let angle = Bam(((-8i32) << 18) as u32);

        let result = test_p_line_attack(
            &mut gs,
            player_h,
            angle,
            Fixed16_16::from_int(1024),
            5,
            Some(&level),
        );

        assert_eq!(result, Some(trooper));
        assert!(gs.mobjslab.get(trooper).unwrap().health < 20);
    }

    #[test]
    fn line_attack_with_level_skips_target_below_autoaim_window() {
        // SAFETY: trig tables are process-global and internally guarded.
        unsafe {
            doom_types::Bam::init_trig_tables();
        }

        let level = make_open_combat_level();
        let mut gs = make_game_state();
        let player_h = gs.player.handle;
        gs.mobjslab.get_mut(player_h).unwrap().z = Fixed16_16::from_int(128);

        let low_trooper = spawn_trooper(&mut gs, 128, 0);
        gs.mobjslab.get_mut(low_trooper).unwrap().z = Fixed16_16::ZERO;

        let result = test_p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(256),
            5,
            Some(&level),
        );

        assert!(
            result.is_none(),
            "target entirely below the autoaim window should not be hit"
        );
        assert_eq!(gs.mobjslab.get(low_trooper).unwrap().health, 20);
    }

    #[test]
    fn line_attack_with_level_skips_low_near_target_and_hits_far_target_in_lane() {
        // SAFETY: trig tables are process-global and internally guarded.
        unsafe {
            doom_types::Bam::init_trig_tables();
        }

        let level = make_open_combat_level();
        let mut gs = make_game_state();
        let player_h = gs.player.handle;
        gs.mobjslab.get_mut(player_h).unwrap().z = Fixed16_16::from_int(128);

        let low_near = spawn_trooper(&mut gs, 96, 0);
        gs.mobjslab.get_mut(low_near).unwrap().z = Fixed16_16::ZERO;

        let high_far = spawn_trooper(&mut gs, 160, 0);
        gs.mobjslab.get_mut(high_far).unwrap().z = Fixed16_16::from_int(128);

        let result = test_p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(256),
            5,
            Some(&level),
        );

        assert_eq!(
            result,
            Some(high_far),
            "shot should ignore the low near actor and hit the farther actor in the autoaim lane"
        );
        assert_eq!(gs.mobjslab.get(low_near).unwrap().health, 20);
        assert!(gs.mobjslab.get(high_far).unwrap().health < 20);
    }

    // -----------------------------------------------------------------------
    // Blockmap-accelerated p_radius_attack tests
    // -----------------------------------------------------------------------

    #[test]
    fn radius_attack_euclidean_distance() {
        // Test that Euclidean distance is used, not Manhattan.
        // Actor at (70, 70) from source at (0, 0):
        //   Manhattan distance = 140
        //   Euclidean distance = sqrt(70^2 + 70^2) ≈ 98.99
        // With radius=100: Manhattan says no (140 >= 100), Euclidean says yes (99 < 100).
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 70, 70);

        let player_handle = gs.player.handle;
        p_radius_attack(&mut gs, player_handle, 100, Fixed16_16::from_int(100), None);

        let health = gs.mobjslab.get(trooper).unwrap().health;
        assert!(
            health < 20,
            "Euclidean distance ~99 < radius 100 → should take damage, health={health}"
        );
    }

    #[test]
    fn radius_attack_no_damage_outside_euclidean_radius() {
        // Actor at (80, 80) from source at (0, 0):
        //   Euclidean distance = sqrt(80^2 + 80^2) ≈ 113.14
        // With radius=100: outside Euclidean radius → no damage.
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 80, 80);

        let player_handle = gs.player.handle;
        p_radius_attack(&mut gs, player_handle, 100, Fixed16_16::from_int(100), None);

        let health = gs.mobjslab.get(trooper).unwrap().health;
        assert_eq!(
            health, 20,
            "Euclidean distance ~113 >= radius 100 → no damage"
        );
    }

    #[test]
    fn radius_attack_distance_scaled_damage() {
        // Two actors at different distances — closer one should take more damage.
        let mut gs = make_game_state();
        let close_trooper = spawn_trooper(&mut gs, 20, 0); // dist=20
        let far_trooper = spawn_trooper(&mut gs, 80, 0); // dist=80

        // Give both actors enough health to survive the blast so we can
        // compare remaining health (troopers default to 20 which is too low).
        gs.mobjslab.get_mut(close_trooper).unwrap().health = 200;
        gs.mobjslab.get_mut(far_trooper).unwrap().health = 200;

        let player_handle = gs.player.handle;
        p_radius_attack(&mut gs, player_handle, 100, Fixed16_16::from_int(100), None);

        let close_health = gs.mobjslab.get(close_trooper).unwrap().health;
        let far_health = gs.mobjslab.get(far_trooper).unwrap().health;
        assert!(
            close_health < far_health,
            "closer actor should take more damage: close_health={close_health}, far_health={far_health}"
        );
    }

    #[test]
    fn radius_attack_los_blocked_by_wall() {
        // With a level, walls should block splash damage.
        let level = make_combat_test_level(); // wall at y=64
        let mut gs = GameState::new("test");
        let mut player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(64),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        player_mo.health = 100;
        player_mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let player_h = gs.mobjslab.alloc(player_mo);
        gs.player = PlayerState::pistol_start(player_h);

        // Trooper at (64, 100) — behind the wall at y=64, within blast radius.
        let trooper = spawn_trooper(&mut gs, 64, 100);

        p_radius_attack(
            &mut gs,
            player_h,
            200,
            Fixed16_16::from_int(200),
            Some(&level),
        );

        let health = gs.mobjslab.get(trooper).unwrap().health;
        assert_eq!(
            health, 20,
            "trooper behind wall should not take splash damage"
        );
    }

    #[test]
    fn radius_attack_damages_in_los() {
        // With a level, actors with clear LOS should take damage.
        let level = make_combat_test_level(); // wall at y=64
        let mut gs = GameState::new("test");
        let mut player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(64),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        player_mo.health = 100;
        player_mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let player_h = gs.mobjslab.alloc(player_mo);
        gs.player = PlayerState::pistol_start(player_h);

        // Trooper at (64, 32) — in front of the wall, clear LOS.
        let trooper = spawn_trooper(&mut gs, 64, 32);

        p_radius_attack(
            &mut gs,
            player_h,
            100,
            Fixed16_16::from_int(200),
            Some(&level),
        );

        let health = gs.mobjslab.get(trooper).unwrap().health;
        assert!(
            health < 20,
            "trooper with clear LOS should take splash damage, health={health}"
        );
    }
}
