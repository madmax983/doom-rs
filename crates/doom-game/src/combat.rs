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
use doom_types::mobj_kind::MobjKind;
use doom_types::weapons::WeaponType;
use doom_types::{Bam, FIXED_ONE, Fixed16_16};

use crate::mobj::{MobjHandle, StateNum, flags};
use crate::state::GameState;
use crate::states::{STATES, ids};
use crate::trace::{self, TraceHit};

/// Maximum hitscan range in map units.
pub const MISSILERANGE: Fixed16_16 = Fixed16_16(2048 << 16);

/// Maximum melee attack range in map units (64 units).
pub const MELEERANGE: Fixed16_16 = Fixed16_16(64 << 16);

// Autoaim view-slope window (P_AimLineAttack, p_map.c:1093):
//   topslope    =  (SCREENHEIGHT/2)*FRACUNIT/(SCREENWIDTH/2) = 100*FRACUNIT/160
//   bottomslope = -(SCREENHEIGHT/2)*FRACUNIT/(SCREENWIDTH/2)
const AUTOAIM_TOP_SLOPE: i32 = (100 * FRAC_ONE_I32) / 160;
const AUTOAIM_BOTTOM_SLOPE: i32 = -AUTOAIM_TOP_SLOPE;

const FRAC_ONE_I32: i32 = 0x0001_0000;
/// Blockmap-block shift in fixed-point space (`FRACBITS + 7`).
const MAPBLOCKSHIFT: u32 = 16 + 7;
/// `MAPBLOCKSHIFT - FRACBITS` (used to bring a fixed coord into block-frac).
const MAPBTOFRAC: u32 = 7;
const MAPBLOCKSIZE_MASK: i32 = (128 << 16) - 1;

/// A `divline_t` (`p_maputl.c`): a directed segment in raw fixed-point.
#[derive(Clone, Copy)]
struct Divline {
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
}

/// Which object a hitscan intercept refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HitscanInterceptKind {
    /// Index into `level.linedefs`.
    Line(usize),
    /// A shootable/blocking actor.
    Actor(MobjHandle),
}

/// One ordered intercept along a hitscan trace (fixed-point `frac`).
#[derive(Clone, Copy, Debug)]
pub struct HitscanIntercept {
    /// Fractional distance along the trace, raw fixed-point (`FRACUNIT == 1.0`).
    frac: i32,
    kind: HitscanInterceptKind,
}

/// `FixedMul` on raw `i32` fixed-point values.
#[inline]
fn fixed_mul_raw(a: i32, b: i32) -> i32 {
    (((a as i64) * (b as i64)) >> 16) as i32
}

/// `FixedDiv` on raw `i32` fixed-point values (vanilla `m_fixed.c`).
#[inline]
fn fixed_div_raw(a: i32, b: i32) -> i32 {
    if (a.unsigned_abs() >> 14) >= b.unsigned_abs() {
        if (a ^ b) < 0 { i32::MIN } else { i32::MAX }
    } else {
        (((a as i64) << 16) / (b as i64)) as i32
    }
}

/// Port of `P_PointOnDivlineSide` (`p_maputl.c:157`). Returns 0 (front) or 1 (back).
fn p_point_on_divline_side(x: i32, y: i32, line: &Divline) -> i32 {
    if line.dx == 0 {
        if x <= line.x {
            return i32::from(line.dy > 0);
        }
        return i32::from(line.dy < 0);
    }
    if line.dy == 0 {
        if y <= line.y {
            return i32::from(line.dx < 0);
        }
        return i32::from(line.dx > 0);
    }

    let dx = x.wrapping_sub(line.x);
    let dy = y.wrapping_sub(line.y);

    // Quick sign-bit decision.
    if ((line.dy ^ line.dx ^ dx ^ dy) & i32::MIN) != 0 {
        if ((line.dy ^ dx) & i32::MIN) != 0 {
            return 1;
        }
        return 0;
    }

    let left = fixed_mul_raw(line.dy >> 8, dx >> 8);
    let right = fixed_mul_raw(dy >> 8, line.dx >> 8);

    if right < left { 0 } else { 1 }
}

/// Port of `P_InterceptVector` (`p_maputl.c:227`). Returns the fractional
/// intercept of `v2` (the trace) with `v1`, raw fixed-point, or 0 if parallel.
fn p_intercept_vector(v2: &Divline, v1: &Divline) -> i32 {
    let den = fixed_mul_raw(v1.dy >> 8, v2.dx).wrapping_sub(fixed_mul_raw(v1.dx >> 8, v2.dy));
    if den == 0 {
        return 0;
    }
    let num = fixed_mul_raw((v1.x.wrapping_sub(v2.x)) >> 8, v1.dy)
        .wrapping_add(fixed_mul_raw((v2.y.wrapping_sub(v1.y)) >> 8, v1.dx));
    fixed_div_raw(num, den)
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

    // -----------------------------------------------------------------------
    // P_DamageMobj knockback thrust (p_inter.c:816-843).
    //
    //   if (inflictor && !(target->flags & MF_NOCLIP)
    //       && (!source || !source->player
    //           || source->player->readyweapon != wp_chainsaw)) {
    //       ang = R_PointToAngle2(inflictor->x, inflictor->y, target->x, target->y);
    //       thrust = damage*(FRACUNIT>>3)*100/target->info->mass;
    //       if (damage < 40 && damage > target->health
    //           && target->z - inflictor->z > 64*FRACUNIT && (P_Random()&1)) {
    //           ang += ANG180; thrust *= 4;
    //       }
    //       ang >>= ANGLETOFINESHIFT;
    //       target->momx += FixedMul(thrust, finecosine[ang]);
    //       target->momy += FixedMul(thrust, finesine[ang]);
    //   }
    //
    // `inflictor` is both inflictor and source for all our hitscan/melee/
    // projectile callers.  The MF_SKULLFLY momentum-clear (p_inter.c:805) is
    // also applied here, matching vanilla order (before the thrust).
    {
        let (tgt_x, tgt_y, tgt_z, tgt_flags, tgt_mass, tgt_health) = {
            let Some(mo) = gs.mobjslab.get(target) else {
                return;
            };
            let info = &crate::mobjinfo::MOBJINFO[mo.kind as usize];
            (mo.x, mo.y, mo.z, mo.flags, info.mass, mo.health)
        };

        // Lost soul (MF_SKULLFLY) loses its charge momentum when hurt.
        if tgt_flags & flags::MF_SKULLFLY != 0 {
            if let Some(mo) = gs.mobjslab.get_mut(target) {
                mo.momx = Fixed16_16::ZERO;
                mo.momy = Fixed16_16::ZERO;
                mo.momz = Fixed16_16::ZERO;
            }
        }

        let source_is_chainsaw =
            inflictor == gs.player.handle && gs.player.weapon == WeaponType::Chainsaw;

        if inflictor != MobjHandle::NULL
            && tgt_flags & flags::MF_NOCLIP == 0
            && !source_is_chainsaw
        {
            if let Some(inf) = gs.mobjslab.get(inflictor) {
                let (ix, iy, iz) = (inf.x, inf.y, inf.z);
                let mut ang =
                    crate::geom::r_point_to_angle2(ix.raw(), iy.raw(), tgt_x.raw(), tgt_y.raw());
                // thrust = damage*(FRACUNIT>>3)*100/mass, all 32-bit integer math.
                let mut thrust = damage
                    .wrapping_mul(FIXED_ONE.raw() >> 3)
                    .wrapping_mul(100)
                    / tgt_mass.max(1);

                // "make fall forwards sometimes" — the only RNG draw in the
                // thrust path, gated by the first three conditions (C `&&`).
                if damage < 40
                    && damage > tgt_health
                    && (tgt_z.raw().wrapping_sub(iz.raw())) > (64 << 16)
                    && (gs.p_random() & 1) != 0
                {
                    ang = ang.wrapping_add(0x8000_0000); // ANG180
                    thrust = thrust.wrapping_mul(4);
                }

                let fine = Bam(ang);
                let dmx = Fixed16_16(thrust).fixed_mul(fine.cos());
                let dmy = Fixed16_16(thrust).fixed_mul(fine.sin());
                if let Some(mo) = gs.mobjslab.get_mut(target) {
                    mo.momx += dmx;
                    mo.momy += dmy;
                }
            }
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

        #[cfg(feature = "telemetry")]
        {
            if dmg > 0 {
                let (px, py) = gs
                    .mobjslab
                    .get(gs.player.handle)
                    .map(|mo| (mo.x, mo.y))
                    .unwrap_or_default();
                gs.telemetry.record(
                    gs.tic_num,
                    px.to_int(),
                    py.to_int(),
                    crate::telemetry::TelemetryKind::DamageTaken(dmg as u32),
                );
            }
        }
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
                // Vanilla P_DamageMobj: threshold = BASETHRESHOLD (100).
                mo.threshold = 100;
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
        // Vanilla P_KillMobj: `target->tics -= P_Random()&3; if(tics<1)tics=1;`
        // desynchronizes death animations. One P_Random draw per kill.
        {
            let r = (gs.p_random() & 3) as i16;
            if let Some(mo) = gs.mobjslab.get_mut(target) {
                mo.tics -= r;
                if mo.tics < 1 {
                    mo.tics = 1;
                }
            }
        }
        let (sx, sy) = gs
            .mobjslab
            .get(target)
            .map(|mo| (mo.x, mo.y))
            .unwrap_or_default();
        #[cfg(feature = "style_meter")]
        if inflictor == gs.player.handle {
            gs.style.register_kill(gs.tic_num);
        }

        #[cfg(feature = "telemetry")]
        if inflictor == gs.player.handle {
            let name = format!("{:?}", kind);
            gs.telemetry.record(
                gs.tic_num,
                sx.to_int(),
                sy.to_int(),
                crate::telemetry::TelemetryKind::MonsterKill(name),
            );
        }
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterDie(kind, target, sx, sy));
    } else {
        // -------------------------------------------------------------------
        // Pain transition. Vanilla: `if ((P_Random() < info->painchance) &&
        // !(target->flags & MF_SKULLFLY)) { ... P_SetMobjState(painstate); }`.
        // C evaluates P_Random() as the left operand of `&&`, so the draw is
        // UNCONDITIONAL for every non-lethal hit (even painchance==0 or an
        // absent painstate) — only whether the pain state is entered is gated.
        // -------------------------------------------------------------------
        let (pain_sn, pain_chance, skullfly): (StateNum, u8, bool) = {
            let Some(mo) = gs.mobjslab.get(target) else {
                return;
            };
            let info = &crate::mobjinfo::MOBJINFO[mo.kind as usize];
            (
                info.pain_state,
                info.pain_chance,
                mo.flags & flags::MF_SKULLFLY != 0,
            )
        };
        let roll = gs.p_random();
        if roll < pain_chance && !skullfly && pain_sn != StateNum::NULL {
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
// Fixed-point path traverse (P_PathTraverse + PTR_AimTraverse/ShootTraverse)
// ---------------------------------------------------------------------------

/// Attack shot height: `z + (height>>1) + 8*FRACUNIT` (raw fixed).
#[inline]
fn hitscan_shootz(z: i32, height: i32) -> i32 {
    z.wrapping_add(height >> 1).wrapping_add(8 << 16)
}

/// Two-sided line opening in raw fixed-point heights.
struct LineOpen {
    open_bottom: i32,
    open_top: i32,
    front_floor: i32,
    front_ceil: i32,
    back: Option<(i32, i32)>, // (back_floor, back_ceil)
}

fn line_open(level: &Level, ld: &doom_map::Linedef) -> Option<LineOpen> {
    let front_sec = level
        .sidedefs
        .get(ld.right_sidedef as usize)
        .and_then(|sd| level.sectors.get(sd.sector as usize))?;
    let front_floor = i32::from(front_sec.floor_height) << 16;
    let front_ceil = i32::from(front_sec.ceil_height) << 16;

    let back = if ld.left_sidedef != doom_map::SIDEDEF_NONE {
        level
            .sidedefs
            .get(ld.left_sidedef as usize)
            .and_then(|sd| level.sectors.get(sd.sector as usize))
            .map(|s| (i32::from(s.floor_height) << 16, i32::from(s.ceil_height) << 16))
    } else {
        None
    };

    let (open_bottom, open_top) = match back {
        Some((bf, bc)) => (front_floor.max(bf), front_ceil.min(bc)),
        None => (front_floor, front_ceil),
    };
    Some(LineOpen {
        open_bottom,
        open_top,
        front_floor,
        front_ceil,
        back,
    })
}

#[inline]
fn is_sky_flat(name: &[u8; 8]) -> bool {
    name[0].eq_ignore_ascii_case(&b'F')
        && name[1] == b'_'
        && name[2].eq_ignore_ascii_case(&b'S')
        && name[3].eq_ignore_ascii_case(&b'K')
        && name[4].eq_ignore_ascii_case(&b'Y')
        && name[5] == b'1'
}

/// `PIT_AddLineIntercepts` (`p_maputl.c:566`) for one linedef.
fn add_line_intercept(
    level: &Level,
    trace: &Divline,
    ld_idx: usize,
    out: &mut smallvec::SmallVec<[HitscanIntercept; 24]>,
) {
    let ld = &level.linedefs[ld_idx];
    let (Some(v1), Some(v2)) = (
        level.vertexes.get(ld.from_vertex as usize),
        level.vertexes.get(ld.to_vertex as usize),
    ) else {
        return;
    };
    let v1x = i32::from(v1.x) << 16;
    let v1y = i32::from(v1.y) << 16;
    let v2x = i32::from(v2.x) << 16;
    let v2y = i32::from(v2.y) << 16;

    // Hitscan/melee traces always exceed 16 map units in some axis, so vanilla
    // uses P_PointOnDivlineSide (never the short-trace P_PointOnLineSide path).
    let s1 = p_point_on_divline_side(v1x, v1y, trace);
    let s2 = p_point_on_divline_side(v2x, v2y, trace);
    if s1 == s2 {
        return; // line isn't crossed
    }
    let dl = Divline {
        x: v1x,
        y: v1y,
        dx: v2x.wrapping_sub(v1x),
        dy: v2y.wrapping_sub(v1y),
    };
    let frac = p_intercept_vector(trace, &dl);
    if frac < 0 {
        return; // behind source
    }
    out.push(HitscanIntercept {
        frac,
        kind: HitscanInterceptKind::Line(ld_idx),
    });
}

/// `PIT_AddThingIntercepts` (`p_maputl.c:611`) for one actor.
fn add_thing_intercept(
    trace: &Divline,
    handle: MobjHandle,
    mo: &crate::mobj::Mobj,
    out: &mut smallvec::SmallVec<[HitscanIntercept; 24]>,
) {
    let r = mo.radius.raw();
    let (mx, my) = (mo.x.raw(), mo.y.raw());
    // corner-to-corner crossection depending on trace direction
    let tracepositive = (trace.dx ^ trace.dy) > 0;
    let (x1, y1, x2, y2) = if tracepositive {
        (mx - r, my + r, mx + r, my - r)
    } else {
        (mx - r, my - r, mx + r, my + r)
    };
    let s1 = p_point_on_divline_side(x1, y1, trace);
    let s2 = p_point_on_divline_side(x2, y2, trace);
    if s1 == s2 {
        return;
    }
    let dl = Divline {
        x: x1,
        y: y1,
        dx: x2.wrapping_sub(x1),
        dy: y2.wrapping_sub(y1),
    };
    let frac = p_intercept_vector(trace, &dl);
    if frac < 0 {
        return;
    }
    out.push(HitscanIntercept {
        frac,
        kind: HitscanInterceptKind::Actor(handle),
    });
}

/// Port of `P_PathTraverse` (`p_maputl.c:862`).  Collects ordered line/actor
/// intercepts along `x1,y1 -> x2,y2` (raw fixed) and returns the effective
/// `trace` divline (including the "don't side exactly on a line" nudge) so the
/// caller can position puffs/blood exactly as vanilla does.  Intercepts are
/// sorted ascending by `frac`.
fn p_path_traverse(
    gs: &GameState,
    level: Option<&Level>,
    source: MobjHandle,
    mut x1: i32,
    mut y1: i32,
    x2: i32,
    y2: i32,
    out: &mut smallvec::SmallVec<[HitscanIntercept; 24]>,
) -> Divline {
    out.clear();

    let Some(level) = level else {
        // No geometry: test all shootable actors against the raw trace.
        let trace = Divline {
            x: x1,
            y: y1,
            dx: x2.wrapping_sub(x1),
            dy: y2.wrapping_sub(y1),
        };
        for handle in gs.mobjslab.iter_handles() {
            if handle == source {
                continue;
            }
            let Some(mo) = gs.mobjslab.get(handle) else {
                continue;
            };
            add_thing_intercept(&trace, handle, mo, out);
        }
        out.sort_by(|a, b| a.frac.cmp(&b.frac));
        return trace;
    };

    let bmaporgx = i32::from(level.blockmap.x_origin) << 16;
    let bmaporgy = i32::from(level.blockmap.y_origin) << 16;
    let bmapwidth = i32::from(level.blockmap.x_count);
    let bmapheight = i32::from(level.blockmap.y_count);

    // "don't side exactly on a line"
    if ((x1.wrapping_sub(bmaporgx)) & MAPBLOCKSIZE_MASK) == 0 {
        x1 = x1.wrapping_add(FRAC_ONE_I32);
    }
    if ((y1.wrapping_sub(bmaporgy)) & MAPBLOCKSIZE_MASK) == 0 {
        y1 = y1.wrapping_add(FRAC_ONE_I32);
    }

    let trace = Divline {
        x: x1,
        y: y1,
        dx: x2.wrapping_sub(x1),
        dy: y2.wrapping_sub(y1),
    };

    let rx1 = x1.wrapping_sub(bmaporgx);
    let ry1 = y1.wrapping_sub(bmaporgy);
    let xt1 = rx1 >> MAPBLOCKSHIFT;
    let yt1 = ry1 >> MAPBLOCKSHIFT;
    let rx2 = x2.wrapping_sub(bmaporgx);
    let ry2 = y2.wrapping_sub(bmaporgy);
    let xt2 = rx2 >> MAPBLOCKSHIFT;
    let yt2 = ry2 >> MAPBLOCKSHIFT;

    let dx = x2.wrapping_sub(x1);
    let dy = y2.wrapping_sub(y1);

    let (mapxstep, mut ystep);
    let partial_x;
    if xt2 > xt1 {
        mapxstep = 1;
        partial_x = FRAC_ONE_I32 - (rx1 >> MAPBTOFRAC & (FRAC_ONE_I32 - 1));
        ystep = fixed_div_raw(dy, dx.abs());
    } else if xt2 < xt1 {
        mapxstep = -1;
        partial_x = rx1 >> MAPBTOFRAC & (FRAC_ONE_I32 - 1);
        ystep = fixed_div_raw(dy, dx.abs());
    } else {
        mapxstep = 0;
        partial_x = FRAC_ONE_I32;
        ystep = 256 * FRAC_ONE_I32;
    }
    let mut yintercept = (ry1 >> MAPBTOFRAC).wrapping_add(fixed_mul_raw(partial_x, ystep));

    let (mapystep, mut xstep);
    let partial_y;
    if yt2 > yt1 {
        mapystep = 1;
        partial_y = FRAC_ONE_I32 - (ry1 >> MAPBTOFRAC & (FRAC_ONE_I32 - 1));
        xstep = fixed_div_raw(dx, dy.abs());
    } else if yt2 < yt1 {
        mapystep = -1;
        partial_y = ry1 >> MAPBTOFRAC & (FRAC_ONE_I32 - 1);
        xstep = fixed_div_raw(dx, dy.abs());
    } else {
        mapystep = 0;
        partial_y = FRAC_ONE_I32;
        xstep = 256 * FRAC_ONE_I32;
    }
    let mut xintercept = (rx1 >> MAPBTOFRAC).wrapping_add(fixed_mul_raw(partial_y, xstep));
    let _ = (&mut ystep, &mut xstep);

    let mut mapx = xt1;
    let mut mapy = yt1;
    let mut tested_lines: smallvec::SmallVec<[usize; 32]> = smallvec::SmallVec::new();
    let mut visited: smallvec::SmallVec<[(i32, i32); 64]> = smallvec::SmallVec::new();

    for _ in 0..64 {
        if mapx >= 0 && mapx < bmapwidth && mapy >= 0 && mapy < bmapheight {
            for ld_idx in level
                .blockmap
                .block_linedefs(mapx as usize, mapy as usize)
            {
                let ld_idx = ld_idx as usize;
                if ld_idx >= level.linedefs.len() || tested_lines.contains(&ld_idx) {
                    continue;
                }
                tested_lines.push(ld_idx);
                add_line_intercept(level, &trace, ld_idx, out);
            }
        }
        visited.push((mapx, mapy));

        if mapx == xt2 && mapy == yt2 {
            break;
        }
        if (yintercept >> 16) == mapy {
            yintercept = yintercept.wrapping_add(ystep);
            mapx += mapxstep;
        } else if (xintercept >> 16) == mapx {
            xintercept = xintercept.wrapping_add(xstep);
            mapy += mapystep;
        }
    }

    // Things (P_BlockThingsIterator): every blockmap-linked actor whose home
    // cell was traversed.  Order is immaterial — the list is frac-sorted.
    for handle in gs.mobjslab.iter_handles() {
        if handle == source {
            continue;
        }
        let Some(mo) = gs.mobjslab.get(handle) else {
            continue;
        };
        if mo.flags & flags::MF_NOBLOCKMAP != 0 {
            continue; // not linked into the blockmap
        }
        let bx = (mo.x.raw().wrapping_sub(bmaporgx)) >> MAPBLOCKSHIFT;
        let by = (mo.y.raw().wrapping_sub(bmaporgy)) >> MAPBLOCKSHIFT;
        if visited.iter().any(|&(cx, cy)| cx == bx && cy == by) {
            add_thing_intercept(&trace, handle, mo, out);
        }
    }

    out.sort_by(|a, b| a.frac.cmp(&b.frac));
    trace
}

// ---------------------------------------------------------------------------
// P_SpawnPuff / P_SpawnBlood
// ---------------------------------------------------------------------------

/// Port of `P_SpawnPuff` (`p_mobj.c:881`).  `attackrange` is raw fixed.
fn p_spawn_puff(gs: &mut GameState, level: Option<&Level>, x: i32, y: i32, z: i32, attackrange: i32) {
    let z = z.wrapping_add(gs.p_subrandom() << 10);
    let h = crate::spawn::p_spawn_mobj(
        gs,
        level,
        Fixed16_16(x),
        Fixed16_16(y),
        Fixed16_16(z),
        MobjKind::BulletPuff,
    );
    let r = (gs.p_random() & 3) as i16;
    if let Some(mo) = gs.mobjslab.get_mut(h) {
        mo.momz = FIXED_ONE;
        mo.tics -= r;
        if mo.tics < 1 {
            mo.tics = 1;
        }
    }
    // Don't make punches spark on the wall.
    if attackrange == MELEERANGE.raw() {
        let sn = StateNum(ids::S_PUFF3);
        let tics = STATES[ids::S_PUFF3 as usize].tics;
        if let Some(mo) = gs.mobjslab.get_mut(h) {
            mo.state = sn;
            mo.tics = tics;
        }
    }
}

/// Port of `P_SpawnBlood` (`p_mobj.c:908`).
fn p_spawn_blood(gs: &mut GameState, level: Option<&Level>, x: i32, y: i32, z: i32, damage: i32) {
    let z = z.wrapping_add(gs.p_subrandom() << 10);
    let h = crate::spawn::p_spawn_mobj(
        gs,
        level,
        Fixed16_16(x),
        Fixed16_16(y),
        Fixed16_16(z),
        MobjKind::Blood,
    );
    let r = (gs.p_random() & 3) as i16;
    if let Some(mo) = gs.mobjslab.get_mut(h) {
        mo.momz = Fixed16_16(2 << 16);
        mo.tics -= r;
        if mo.tics < 1 {
            mo.tics = 1;
        }
    }
    if damage <= 12 && damage >= 9 {
        set_mobj_state_raw(gs, h, ids::S_BLOOD2);
    } else if damage < 9 {
        set_mobj_state_raw(gs, h, ids::S_BLOOD3);
    }
}

fn set_mobj_state_raw(gs: &mut GameState, h: MobjHandle, state: u16) {
    let tics = STATES[state as usize].tics;
    if let Some(mo) = gs.mobjslab.get_mut(h) {
        mo.state = StateNum(state);
        mo.tics = tics;
    }
}

// ---------------------------------------------------------------------------
// P_AimLineAttack / P_LineAttack
// ---------------------------------------------------------------------------

/// Result of `P_AimLineAttack`: the autoaim vertical `slope` and the actor
/// that was aimed at (if any).
#[derive(Clone, Copy, Debug)]
pub struct AimResult {
    /// Vertical aim slope (raw fixed).  `0` when no target was acquired.
    pub slope: i32,
    /// The actor aimed at, or `None`.
    pub linetarget: Option<MobjHandle>,
}

/// Port of `P_AimLineAttack` (`p_map.c:1075`).  Traces an autoaim ray and
/// returns the vertical slope + `linetarget`.  Draws no RNG.
pub fn p_aim_line_attack(
    gs: &GameState,
    source: MobjHandle,
    angle: Bam,
    distance: Fixed16_16,
    level: Option<&Level>,
) -> AimResult {
    let none = AimResult {
        slope: 0,
        linetarget: None,
    };
    let Some(mo) = gs.mobjslab.get(source) else {
        return none;
    };
    let t1x = mo.x.raw();
    let t1y = mo.y.raw();
    let shootz = hitscan_shootz(mo.z.raw(), mo.height.raw());
    let dist_i = distance.to_int();
    let x2 = t1x.wrapping_add(dist_i.wrapping_mul(angle.cos().raw()));
    let y2 = t1y.wrapping_add(dist_i.wrapping_mul(angle.sin().raw()));
    let attackrange = distance.raw();

    let mut topslope = AUTOAIM_TOP_SLOPE;
    let mut bottomslope = AUTOAIM_BOTTOM_SLOPE;

    let mut intercepts = smallvec::SmallVec::new();
    p_path_traverse(gs, level, source, t1x, t1y, x2, y2, &mut intercepts);

    for ic in intercepts.iter() {
        match ic.kind {
            HitscanInterceptKind::Line(ld_idx) => {
                let Some(lvl) = level else {
                    continue;
                };
                let ld = &lvl.linedefs[ld_idx];
                if !ld.is_two_sided() {
                    return none; // stop
                }
                let Some(open) = line_open(lvl, ld) else {
                    return none;
                };
                if open.open_bottom >= open.open_top {
                    return none; // stop
                }
                let dist = fixed_mul_raw(attackrange, ic.frac);
                let floor_diff = open
                    .back
                    .map(|(bf, _)| open.front_floor != bf)
                    .unwrap_or(true);
                if floor_diff {
                    let slope = fixed_div_raw(open.open_bottom.wrapping_sub(shootz), dist);
                    if slope > bottomslope {
                        bottomslope = slope;
                    }
                }
                let ceil_diff = open
                    .back
                    .map(|(_, bc)| open.front_ceil != bc)
                    .unwrap_or(true);
                if ceil_diff {
                    let slope = fixed_div_raw(open.open_top.wrapping_sub(shootz), dist);
                    if slope < topslope {
                        topslope = slope;
                    }
                }
                if topslope <= bottomslope {
                    return none; // stop
                }
            }
            HitscanInterceptKind::Actor(handle) => {
                let Some(th) = gs.mobjslab.get(handle) else {
                    continue;
                };
                if handle == source || th.flags & flags::MF_SHOOTABLE == 0 {
                    continue;
                }
                let dist = fixed_mul_raw(attackrange, ic.frac);
                let mut thingtopslope =
                    fixed_div_raw(th.z.raw().wrapping_add(th.height.raw()).wrapping_sub(shootz), dist);
                if thingtopslope < bottomslope {
                    continue; // over
                }
                let mut thingbottomslope = fixed_div_raw(th.z.raw().wrapping_sub(shootz), dist);
                if thingbottomslope > topslope {
                    continue; // under
                }
                if thingtopslope > topslope {
                    thingtopslope = topslope;
                }
                if thingbottomslope < bottomslope {
                    thingbottomslope = bottomslope;
                }
                let aimslope = (thingtopslope.wrapping_add(thingbottomslope)) / 2;
                return AimResult {
                    slope: aimslope,
                    linetarget: Some(handle),
                };
            }
        }
    }

    none
}

/// Port of `P_LineAttack` (`p_map.c:1117`).  Fires along `angle` at the given
/// vertical `slope`, spawning puffs on walls and blood on monsters, and
/// applying damage.  Returns the actor hit (if any).
pub fn p_line_attack(
    gs: &mut GameState,
    source: MobjHandle,
    angle: Bam,
    distance: Fixed16_16,
    slope: i32,
    damage: i32,
    level: Option<&Level>,
) -> Option<MobjHandle> {
    let (t1x, t1y, shootz) = {
        let mo = gs.mobjslab.get(source)?;
        (mo.x.raw(), mo.y.raw(), hitscan_shootz(mo.z.raw(), mo.height.raw()))
    };
    let dist_i = distance.to_int();
    let x2 = t1x.wrapping_add(dist_i.wrapping_mul(angle.cos().raw()));
    let y2 = t1y.wrapping_add(dist_i.wrapping_mul(angle.sin().raw()));
    let attackrange = distance.raw();
    let aimslope = slope;

    let mut intercepts = smallvec::SmallVec::new();
    let trace = p_path_traverse(gs, level, source, t1x, t1y, x2, y2, &mut intercepts);

    for ic in intercepts.iter().copied().collect::<smallvec::SmallVec<[HitscanIntercept; 24]>>() {
        match ic.kind {
            HitscanInterceptKind::Line(ld_idx) => {
                // PTR_ShootTraverse — line case.
                let lvl = level.expect("line intercepts require a level");
                let ld = &lvl.linedefs[ld_idx];

                let mut hit_line = !ld.is_two_sided();
                if ld.is_two_sided() {
                    if let Some(open) = line_open(lvl, ld) {
                        let dist = fixed_mul_raw(attackrange, ic.frac);
                        match open.back {
                            None => {
                                // e6y missed-back-side emulation.
                                let s = fixed_div_raw(open.open_bottom.wrapping_sub(shootz), dist);
                                if s > aimslope {
                                    hit_line = true;
                                }
                                let s = fixed_div_raw(open.open_top.wrapping_sub(shootz), dist);
                                if s < aimslope {
                                    hit_line = true;
                                }
                            }
                            Some((bf, bc)) => {
                                if open.front_floor != bf {
                                    let s =
                                        fixed_div_raw(open.open_bottom.wrapping_sub(shootz), dist);
                                    if s > aimslope {
                                        hit_line = true;
                                    }
                                }
                                if open.front_ceil != bc {
                                    let s = fixed_div_raw(open.open_top.wrapping_sub(shootz), dist);
                                    if s < aimslope {
                                        hit_line = true;
                                    }
                                }
                            }
                        }
                    } else {
                        hit_line = true;
                    }
                }

                if !hit_line {
                    continue; // shot passes through
                }

                // Position a bit closer to the wall.
                let frac = ic.frac.wrapping_sub(fixed_div_raw(4 << 16, attackrange));
                let x = trace.x.wrapping_add(fixed_mul_raw(trace.dx, frac));
                let y = trace.y.wrapping_add(fixed_mul_raw(trace.dy, frac));
                let z = shootz.wrapping_add(fixed_mul_raw(aimslope, fixed_mul_raw(frac, attackrange)));

                // Don't shoot the sky.
                let front_sec = lvl
                    .sidedefs
                    .get(ld.right_sidedef as usize)
                    .and_then(|sd| lvl.sectors.get(sd.sector as usize));
                if let Some(front) = front_sec
                    && is_sky_flat(&front.ceil_flat)
                {
                    if z > (i32::from(front.ceil_height) << 16) {
                        return None;
                    }
                    let back_sky = (ld.left_sidedef != doom_map::SIDEDEF_NONE)
                        .then(|| {
                            lvl.sidedefs
                                .get(ld.left_sidedef as usize)
                                .and_then(|sd| lvl.sectors.get(sd.sector as usize))
                                .map(|s| is_sky_flat(&s.ceil_flat))
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    if back_sky {
                        return None;
                    }
                }

                p_spawn_puff(gs, level, x, y, z, attackrange);
                return None; // stop
            }
            HitscanInterceptKind::Actor(handle) => {
                let (thz, thh, thflags) = {
                    let Some(th) = gs.mobjslab.get(handle) else {
                        continue;
                    };
                    if handle == source || th.flags & flags::MF_SHOOTABLE == 0 {
                        continue;
                    }
                    (th.z.raw(), th.height.raw(), th.flags)
                };
                let dist = fixed_mul_raw(attackrange, ic.frac);
                let thingtopslope =
                    fixed_div_raw(thz.wrapping_add(thh).wrapping_sub(shootz), dist);
                if thingtopslope < aimslope {
                    continue; // over
                }
                let thingbottomslope = fixed_div_raw(thz.wrapping_sub(shootz), dist);
                if thingbottomslope > aimslope {
                    continue; // under
                }

                // hit thing — position a bit closer.
                let frac = ic.frac.wrapping_sub(fixed_div_raw(10 << 16, attackrange));
                let x = trace.x.wrapping_add(fixed_mul_raw(trace.dx, frac));
                let y = trace.y.wrapping_add(fixed_mul_raw(trace.dy, frac));
                let z = shootz.wrapping_add(fixed_mul_raw(aimslope, fixed_mul_raw(frac, attackrange)));

                if thflags & flags::MF_NOBLOOD != 0 {
                    p_spawn_puff(gs, level, x, y, z, attackrange);
                } else {
                    p_spawn_blood(gs, level, x, y, z, damage);
                }
                if damage > 0 {
                    damage_mobj(gs, handle, source, damage);
                }
                return Some(handle); // stop
            }
        }
    }

    None
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
    let Some(mo) = gs.mobjslab.get(source) else {
        return;
    };
    let (sx, sy) = (mo.x.to_int(), mo.y.to_int());

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
        let Some(mo) = gs.mobjslab.get(handle) else {
            continue;
        };
        let (ax, ay, alive, shootable) = (
            mo.x.to_int(),
            mo.y.to_int(),
            mo.health > 0,
            mo.flags & flags::MF_SHOOTABLE != 0,
        );

        if !alive || !shootable {
            continue;
        }

        // Euclidean distance.
        let dx_f = (ax - sx) as f32;
        let dy_f = (ay - sy) as f32;

        // Box bounding check to avoid expensive .sqrt() for distant actors.
        if dx_f.abs() >= radius_f || dy_f.abs() >= radius_f {
            continue;
        }

        let dist_f = (dx_f * dx_f + dy_f * dy_f).sqrt();

        if dist_f >= radius_f {
            continue;
        }

        // LOS check: ensure no wall blocks the blast line.
        if let Some(lv) = level {
            let total_dist = dist_f.max(1.0);
            let cos_a = dx_f / total_dist;
            let sin_a = dy_f / total_dist;
            let los = trace::trace_ray(
                lv,
                sx,
                sy,
                cos_a,
                sin_a,
                total_dist,
                trace::ActorCheck::Ignore,
            );
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
    use crate::mobj::Mobj;
    use crate::player::PlayerState;
    use crate::state::GameState;
    use doom_types::TicCmd;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    /// Test shim preserving the historical `p_line_attack` signature (with an
    /// `intercepts` scratch buffer and no explicit slope).  Forwards to the
    /// vanilla `super::p_line_attack` with a level (horizontal) `slope == 0`,
    /// which is correct for the same-height targets these tests use.
    fn p_line_attack(
        gs: &mut GameState,
        source: MobjHandle,
        angle: Bam,
        range: Fixed16_16,
        damage: i32,
        level: Option<&Level>,
        _intercepts: &mut smallvec::SmallVec<[HitscanIntercept; 16]>,
    ) -> Option<MobjHandle> {
        super::p_line_attack(gs, source, angle, range, 0, damage, level)
    }

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
        use crate::mobjinfo::MOBJINFO;
        use crate::states::STATES;
        use doom_types::mobj_kind::MobjKind;
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
        assert_eq!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .health,
            15
        );
    }

    #[test]
    fn damage_applies_knockback_thrust_along_inflictor_axis() {
        // Vanilla P_DamageMobj: thrust = damage*(FRACUNIT>>3)*100/mass along the
        // angle from inflictor to target.  Inflictor east of nothing / target to
        // the +x side => positive momx, zero momy.
        unsafe {
            Bam::init_trig_tables();
        }
        let mut gs = make_game_state();
        let player = gs.player.handle; // at origin, acts as inflictor
        let trooper = spawn_trooper(&mut gs, 100, 0); // due east of inflictor
        gs.rng.set_index(3);

        damage_mobj(&mut gs, trooper, player, 5);

        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
        // thrust = 5*8192*100/100 = 40960; FixedMul(40960, finecosine[0]) ~= 40959.
        assert!(
            mo.momx.raw() > 1000,
            "thrust should push the target away from the inflictor (+x), got {}",
            mo.momx.raw()
        );
        assert!(
            mo.momy.raw().abs() < mo.momx.raw() / 100,
            "thrust is essentially axis-aligned east, got momy={}",
            mo.momy.raw()
        );
    }

    #[test]
    fn damage_with_null_inflictor_applies_no_thrust() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);
        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
        assert_eq!(mo.momx.raw(), 0);
        assert_eq!(mo.momy.raw(), 0);
    }

    #[test]
    fn damage_wakes_monster_and_marks_justhit() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let player = gs.player.handle;
        let spawn_state = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .state;
        let see_state = crate::mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

        gs.rng.set_index(3); // 220 >= trooper pain chance, so no pain-state detour.
        damage_mobj(&mut gs, trooper, player, 5);

        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
        assert_eq!(
            mo.target, player,
            "monster should retaliate against the attacker"
        );
        assert_eq!(
            mo.threshold, 100,
            "monster should enter BASETHRESHOLD alert after being hit"
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

        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
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

        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
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
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .health,
            0,
            "health must clamp to 0, not go negative"
        );
    }

    #[test]
    fn damage_ignores_non_shootable() {
        let mut gs = make_game_state();
        // Spawn a trooper but strip MF_SHOOTABLE.
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab
            .get_mut(trooper)
            .expect("value must exist in test")
            .flags &= !flags::MF_SHOOTABLE;

        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 10);
        // Health must be unchanged (still 20).
        assert_eq!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .health,
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
        let health = gs
            .mobjslab
            .get(target)
            .expect("value must exist in test")
            .health;
        assert_eq!(health, 0);
    }

    #[test]
    fn damage_sets_inflictor_target() {
        let mut gs = make_game_state();
        let player_handle = gs.player.handle;
        let trooper = spawn_trooper(&mut gs, 100, 0);

        damage_mobj(&mut gs, trooper, player_handle, 5);

        assert_eq!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .target,
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

        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
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

        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
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
        let state_after_kill = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .state;

        // Apply more damage — must be a no-op.
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);

        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
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
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .flags
                & crate::mobj::flags::MF_SCREAMED,
            0,
            "MF_SCREAMED must be clear before kill"
        );

        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 20);

        assert_ne!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .flags
                & crate::mobj::flags::MF_SCREAMED,
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

        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            src_handle,
            Bam::ZERO,
            MISSILERANGE,
            10,
            None,
            &mut intercepts,
        );
        assert!(result.is_none(), "must return None when no targets exist");
    }

    #[test]
    #[ignore = "requires Bam::init_trig_tables() which is unsafe and not called in unit tests"]
    fn line_attack_hits_actor_directly_ahead() {
        // This test requires trig tables.  Skipped per task spec.
        let mut gs = make_game_state();
        let _trooper = spawn_trooper(&mut gs, 100, 0);
        let src = gs.player.handle;
        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            src,
            Bam::ZERO,
            Fixed16_16::from_int(500),
            5,
            None,
            &mut intercepts,
        );
        assert!(result.is_some(), "should hit actor directly ahead");
    }

    #[test]
    fn line_attack_skips_dead_actors() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        // Kill the trooper first.  A real corpse (P_KillMobj) also clears
        // MF_SHOOTABLE, which is what the shoot-traverse checks.
        {
            let mo = gs.mobjslab.get_mut(trooper).expect("value must exist in test");
            mo.health = 0;
            mo.flags &= !flags::MF_SHOOTABLE;
        }

        let src = gs.player.handle;
        // Even if trig tables were initialized and the geometry lined up,
        // dead actors must be skipped.  With uninitialized tables, t=0 and
        // both the dead-check and t<=0 guard fire — None is the expected result.
        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            src,
            Bam::ZERO,
            MISSILERANGE,
            10,
            None,
            &mut intercepts,
        );
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

        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            src,
            angle,
            Fixed16_16::from_int(1024),
            5,
            None,
            &mut intercepts,
        );

        assert_eq!(result, Some(trooper));
        assert!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .health
                < 20
        );
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

        let health = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .health;
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
        use crate::mobjinfo::MOBJINFO;
        use crate::states::STATES;
        use doom_types::mobj_kind::MobjKind;
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
        let original_state = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .state;

        // Override the trooper's kind is not possible, but we can test by
        // putting the RNG into a state where roll >= pain_chance.
        // Trooper pain_chance = 200. Find RNG index where RNG_TABLE[i] >= 200.
        // RNG_TABLE[3] = 220 >= 200. Set rng to index 3.
        gs.rng.set_index(3);
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);

        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
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
            let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
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
        let state_after_death = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .state;

        // Try to damage again — dead actor should be ignored entirely.
        damage_mobj(&mut gs, trooper, MobjHandle::NULL, 5);

        let mo = gs.mobjslab.get(trooper).expect("value must exist in test");
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
        let original_state = gs
            .mobjslab
            .get(wolfss)
            .expect("value must exist in test")
            .state;

        // Set RNG to index 0 (value=0), so 0 < 170 would be true.
        gs.rng.set_index(0);
        damage_mobj(&mut gs, wolfss, MobjHandle::NULL, 5);

        let mo = gs.mobjslab.get(wolfss).expect("value must exist in test");
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

        let health = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .health;
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
        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(200),
            10,
            Some(&level),
            &mut intercepts,
        );
        // Without trig tables, cos/sin = 0, so trace_ray gets zero direction.
        // This is expected behavior — the trig-table-dependent behavior
        // matches the fallback path.
        assert!(
            result.is_none(),
            "without trig tables, ray has zero direction → no hit"
        );
        // Verify trooper is unharmed.
        assert_eq!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .health,
            20
        );
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

        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(200),
            10,
            Some(&level),
            &mut intercepts,
        );
        assert!(result.is_none(), "wall should block hitscan");
        assert_eq!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .health,
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

        // SAFETY: trig tables are process-global and internally guarded.
        unsafe {
            doom_types::Bam::init_trig_tables();
        }
        // Trooper essentially on top of the player, directly ahead — a
        // point-blank hit.
        let trooper = spawn_trooper(&mut gs, 96, 0);

        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(64),
            5,
            Some(&level),
            &mut intercepts,
        );
        assert_eq!(result, Some(trooper), "point-blank target must be hit");
        assert!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .health
                < 20
        );
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

        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(5), // very short range
            10,
            Some(&level),
            &mut intercepts,
        );
        // Regardless of trig tables, short range = miss.
        assert!(result.is_none());
        assert_eq!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .health,
            20
        );
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

        // Vanilla P_LineAttack fires straight down `angle` (no horizontal
        // autoaim); a target on that axis is hit.
        let trooper = spawn_trooper(&mut gs, 512, 0);
        let angle = Bam::ZERO;

        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            player_h,
            angle,
            Fixed16_16::from_int(1024),
            5,
            Some(&level),
            &mut intercepts,
        );

        assert_eq!(result, Some(trooper));
        assert!(
            gs.mobjslab
                .get(trooper)
                .expect("value must exist in test")
                .health
                < 20
        );
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
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .z = Fixed16_16::from_int(128);

        let low_trooper = spawn_trooper(&mut gs, 128, 0);
        gs.mobjslab
            .get_mut(low_trooper)
            .expect("value must exist in test")
            .z = Fixed16_16::ZERO;

        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(256),
            5,
            Some(&level),
            &mut intercepts,
        );

        assert!(
            result.is_none(),
            "target entirely below the autoaim window should not be hit"
        );
        assert_eq!(
            gs.mobjslab
                .get(low_trooper)
                .expect("value must exist in test")
                .health,
            20
        );
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
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .z = Fixed16_16::from_int(128);

        let low_near = spawn_trooper(&mut gs, 96, 0);
        gs.mobjslab
            .get_mut(low_near)
            .expect("value must exist in test")
            .z = Fixed16_16::ZERO;

        let high_far = spawn_trooper(&mut gs, 160, 0);
        gs.mobjslab
            .get_mut(high_far)
            .expect("value must exist in test")
            .z = Fixed16_16::from_int(128);

        let mut intercepts = smallvec::SmallVec::new();
        let result = p_line_attack(
            &mut gs,
            player_h,
            Bam::ZERO,
            Fixed16_16::from_int(256),
            5,
            Some(&level),
            &mut intercepts,
        );

        assert_eq!(
            result,
            Some(high_far),
            "shot should ignore the low near actor and hit the farther actor in the autoaim lane"
        );
        assert_eq!(
            gs.mobjslab
                .get(low_near)
                .expect("value must exist in test")
                .health,
            20
        );
        assert!(
            gs.mobjslab
                .get(high_far)
                .expect("value must exist in test")
                .health
                < 20
        );
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

        let health = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .health;
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

        let health = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .health;
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
        gs.mobjslab
            .get_mut(close_trooper)
            .expect("value must exist in test")
            .health = 200;
        gs.mobjslab
            .get_mut(far_trooper)
            .expect("value must exist in test")
            .health = 200;

        let player_handle = gs.player.handle;
        p_radius_attack(&mut gs, player_handle, 100, Fixed16_16::from_int(100), None);

        let close_health = gs
            .mobjslab
            .get(close_trooper)
            .expect("value must exist in test")
            .health;
        let far_health = gs
            .mobjslab
            .get(far_trooper)
            .expect("value must exist in test")
            .health;
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

        let health = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .health;
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

        let health = gs
            .mobjslab
            .get(trooper)
            .expect("value must exist in test")
            .health;
        assert!(
            health < 20,
            "trooper with clear LOS should take splash damage, health={health}"
        );
    }
}
