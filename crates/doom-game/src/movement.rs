//! P_TryMove — blockmap-based collision detection for actor movement.
//!
//! Port of Doom's `p_map.c: P_TryMove` with a basic `p_slide_move` helper.
//!
//! # Algorithm
//! 1. Compute the actor's proposed bounding box at `(new_x, new_y)`.
//! 2. Iterate over every blockmap cell the bounding box overlaps.
//! 3. For each linedef in those cells:
//!    a. Quick AABB rejection (actor bbox vs linedef bbox).
//!    b. Line-straddling test (is bbox on both sides of the infinite line?).
//!    c. One-sided → always blocked.
//!    d. Two-sided → check opening height ≥ actor height and step ≤ 24 units.
//! 4. Return `true` if no blocking linedef was found.

use crate::mobj::{MobjHandle, MobjSlab, flags};
use doom_map::Level;
use doom_types::Fixed16_16;

#[derive(Clone, Copy, Debug)]
struct BlockingLine {
    linedef_idx: usize,
    x1: Fixed16_16,
    y1: Fixed16_16,
    x2: Fixed16_16,
    y2: Fixed16_16,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum height difference an actor can step up automatically (24 units).
///
/// Doom original: `MAXSTEP = 24 * FRACUNIT`.
pub const MAX_STEP_HEIGHT: Fixed16_16 = Fixed16_16(24 << 16);

/// Blockmap cell size in map units.
const BLOCK_SIZE: i32 = 128;

// ---------------------------------------------------------------------------
// P_TryMove
// ---------------------------------------------------------------------------

/// Attempt to move actor `handle` to `(new_x, new_y)`.
///
/// Returns `true` if the move is legal (no blocking linedefs, steps within
/// height limit).  The caller is responsible for applying the position update
/// when `true` is returned.
///
/// # Notes
/// - `MF_NOCLIP` bypasses all geometry checks.
/// - Actor-vs-actor clipping is not implemented yet (Batch 3).
/// - `mo.z` is used as the actor's current floor height for step calculations.
pub fn p_try_move(
    slab: &MobjSlab,
    handle: MobjHandle,
    new_x: Fixed16_16,
    new_y: Fixed16_16,
    level: &Level,
) -> bool {
    try_move_with_blocker(slab, handle, new_x, new_y, level).0
}

/// Attempt to move actor `handle` to `(new_x, new_y)` and report the exact
/// blocking linedef index when movement fails on geometry.
pub fn p_try_move_blocker(
    slab: &MobjSlab,
    handle: MobjHandle,
    new_x: Fixed16_16,
    new_y: Fixed16_16,
    level: &Level,
) -> (bool, Option<usize>) {
    let (can_move, blocker) = try_move_with_blocker(slab, handle, new_x, new_y, level);
    (can_move, blocker.map(|line| line.linedef_idx))
}

/// Attempt a movement with wall-sliding fallback.
///
/// Caller provides both the current origin `(old_x, old_y)` and desired target
/// `(new_x, new_y)`. If `P_TryMove` fails, this projects the move vector onto
/// the blocking linedef tangent, then falls back to axis-wise slide attempts.
/// Returns the best legal target, or the original origin if fully blocked.
#[must_use]
pub fn p_slide_move(
    slab: &MobjSlab,
    handle: MobjHandle,
    old_x: Fixed16_16,
    old_y: Fixed16_16,
    new_x: Fixed16_16,
    new_y: Fixed16_16,
    level: &Level,
) -> (Fixed16_16, Fixed16_16) {
    if slab.get(handle).is_none() {
        return (old_x, old_y);
    }
    if p_try_move(slab, handle, new_x, new_y, level) {
        return (new_x, new_y);
    }

    let mut best: Option<(Fixed16_16, Fixed16_16, i128)> = None;
    let mut consider = |cx: Fixed16_16, cy: Fixed16_16| {
        if (cx == old_x && cy == old_y) || !p_try_move(slab, handle, cx, cy, level) {
            return;
        }
        let dx = (cx - old_x).0 as i128;
        let dy = (cy - old_y).0 as i128;
        let dist_sq = dx * dx + dy * dy;
        let replace = match best {
            None => true,
            Some((_, _, cur)) => dist_sq > cur,
        };
        if replace {
            best = Some((cx, cy, dist_sq));
        }
    };

    // 1) Project onto blocking line tangent for a Doom-like slide.
    let (_, blocker) = try_move_with_blocker(slab, handle, new_x, new_y, level);
    if let Some(bl) = blocker {
        let tx = (bl.x2 - bl.x1).0 as i128;
        let ty = (bl.y2 - bl.y1).0 as i128;
        let vx = (new_x - old_x).0 as i128;
        let vy = (new_y - old_y).0 as i128;
        let mag2 = tx * tx + ty * ty;
        if mag2 > 0 {
            let dot = vx * tx + vy * ty;
            let sx_raw = (tx * dot) / mag2;
            let sy_raw = (ty * dot) / mag2;
            let slide_x = old_x + Fixed16_16::from_raw(clamp_i128_to_i32(sx_raw));
            let slide_y = old_y + Fixed16_16::from_raw(clamp_i128_to_i32(sy_raw));
            consider(slide_x, slide_y);
        }
    }

    // 2) Axis slide fallback (helps with axis-aligned corners).
    consider(new_x, old_y);
    consider(old_x, new_y);

    best.map(|(x, y, _)| (x, y)).unwrap_or((old_x, old_y))
}

// ---------------------------------------------------------------------------
// Vanilla P_TryMove (committing) + P_SlideMove
// ---------------------------------------------------------------------------

/// `FRACUNIT` as a raw fixed-point integer.
const FRACUNIT: i32 = 1 << 16;

/// Attempt to move actor `handle` to `(x, y)` and, on success, commit the new
/// position (mirroring vanilla `P_TryMove`, which sets `mo->x/mo->y`).
///
/// Returns `true` if the move was legal (and applied), `false` otherwise.
pub fn p_try_move_commit(
    slab: &mut MobjSlab,
    handle: MobjHandle,
    x: Fixed16_16,
    y: Fixed16_16,
    level: &Level,
) -> bool {
    p_try_move_commit_inner(slab, handle, x, y, level, None)
}

/// Like [`p_try_move_commit`], but on a successful move also appends the
/// walkover line-crossings (vanilla `P_TryMove`'s `spechit` walk) to `sink`.
///
/// Used by the player move path so every committed `P_TryMove` step — the
/// split-move halves and each `P_SlideMove` sub-step — records its crossings
/// exactly where vanilla would fire `P_CrossSpecialLine`, in vanilla order.
pub fn p_try_move_commit_tracked(
    slab: &mut MobjSlab,
    handle: MobjHandle,
    x: Fixed16_16,
    y: Fixed16_16,
    level: &Level,
    sink: &mut Vec<usize>,
) -> bool {
    p_try_move_commit_inner(slab, handle, x, y, level, Some(sink))
}

fn p_try_move_commit_inner(
    slab: &mut MobjSlab,
    handle: MobjHandle,
    x: Fixed16_16,
    y: Fixed16_16,
    level: &Level,
    sink: Option<&mut Vec<usize>>,
) -> bool {
    if !p_try_move(slab, handle, x, y, level) {
        return false;
    }
    let old = slab.get(handle).map(|mo| (mo.x, mo.y));
    // Vanilla `P_TryMove`: unlink from the blockmap, move, then relink
    // (`P_UnsetThingPosition` → set `x`/`y` → `P_SetThingPosition`).  Unlink
    // must precede the coordinate change (the head-unlink re-derives the cell).
    slab.unset_thing_position(handle);
    if let Some(mo) = slab.get_mut(handle) {
        mo.x = x;
        mo.y = y;
    }
    slab.set_thing_position(handle);
    if let (Some(sink), Some((old_x, old_y))) = (sink, old) {
        record_player_crossings(slab, handle, old_x, old_y, x, y, level, sink);
    }
    true
}

/// Raw fixed-point endpoints + deltas of a linedef.
struct LineGeom {
    v1x: i32,
    v1y: i32,
    dx: i32,
    dy: i32,
}

fn line_geom(level: &Level, ld: &doom_map::Linedef) -> LineGeom {
    let v1 = &level.vertexes[ld.from_vertex as usize];
    let v2 = &level.vertexes[ld.to_vertex as usize];
    let v1x = (v1.x as i32) << 16;
    let v1y = (v1.y as i32) << 16;
    let v2x = (v2.x as i32) << 16;
    let v2y = (v2.y as i32) << 16;
    LineGeom {
        v1x,
        v1y,
        dx: v2x - v1x,
        dy: v2y - v1y,
    }
}

/// Collect the linedefs a slide trace crosses, with their intercept fractions,
/// mirroring `PIT_AddLineIntercepts` + `P_TraverseIntercepts` (`p_maputl.c`).
///
/// The trace runs from `(x1, y1)` by `(dx, dy)` (raw fixed-point). Only lines
/// with `0 <= frac <= FRACUNIT` are returned, sorted ascending by `frac`.
fn collect_slide_intercepts(
    level: &Level,
    x1: i32,
    y1: i32,
    dx: i32,
    dy: i32,
    out: &mut Vec<(i32, usize)>,
) {
    use crate::geom::{DivLine, p_intercept_vector, p_point_on_divline_side, p_point_on_line_side};

    out.clear();
    let trace = DivLine {
        x: x1,
        y: y1,
        dx,
        dy,
    };
    let use_divline =
        dx > FRACUNIT * 16 || dy > FRACUNIT * 16 || dx < -FRACUNIT * 16 || dy < -FRACUNIT * 16;

    let bm = &level.blockmap;
    let x_origin = bm.x_origin as i32;
    let y_origin = bm.y_origin as i32;
    let x_count = bm.x_count as i32;
    let y_count = bm.y_count as i32;
    let to_block = |world_fixed: i32, origin: i32, count: i32| -> i32 {
        let cell = ((world_fixed >> 16) - origin) / BLOCK_SIZE;
        cell.max(0).min(count - 1)
    };

    let lo_x = x1.min(x1 + dx);
    let hi_x = x1.max(x1 + dx);
    let lo_y = y1.min(y1 + dy);
    let hi_y = y1.max(y1 + dy);
    let col_lo = to_block(lo_x, x_origin, x_count);
    let col_hi = to_block(hi_x, x_origin, x_count);
    let row_lo = to_block(lo_y, y_origin, y_count);
    let row_hi = to_block(hi_y, y_origin, y_count);

    let mut seen: smallvec::SmallVec<[usize; 32]> = smallvec::SmallVec::new();

    for row in row_lo..=row_hi {
        for col in col_lo..=col_hi {
            for ld_idx in bm.block_linedefs(col as usize, row as usize) {
                let ld_idx = ld_idx as usize;
                if seen.contains(&ld_idx) {
                    continue;
                }
                seen.push(ld_idx);
                let Some(ld) = level.linedefs.get(ld_idx) else {
                    continue;
                };
                let g = line_geom(level, ld);

                let (s1, s2) = if use_divline {
                    (
                        p_point_on_divline_side(g.v1x, g.v1y, &trace),
                        p_point_on_divline_side(g.v1x + g.dx, g.v1y + g.dy, &trace),
                    )
                } else {
                    (
                        p_point_on_line_side(trace.x, trace.y, g.v1x, g.v1y, g.dx, g.dy),
                        p_point_on_line_side(
                            trace.x + trace.dx,
                            trace.y + trace.dy,
                            g.v1x,
                            g.v1y,
                            g.dx,
                            g.dy,
                        ),
                    )
                };
                if s1 == s2 {
                    continue; // line isn't crossed by the trace
                }

                let dl = DivLine {
                    x: g.v1x,
                    y: g.v1y,
                    dx: g.dx,
                    dy: g.dy,
                };
                let frac = p_intercept_vector(&trace, &dl);
                if !(0..=FRACUNIT).contains(&frac) {
                    continue;
                }
                out.push((frac, ld_idx));
            }
        }
    }

    out.sort_by_key(|&(frac, _)| frac);
}

/// Running best-slide state shared across the three corner traces
/// (`bestslidefrac` / `bestslideline` in `p_map.c`).
struct SlideState {
    best_frac: i32,
    best_line: Option<usize>,
}

/// Port of `PTR_SlideTraverse` for one corner trace: walk the crossed lines in
/// frac order and record the first that blocks, updating `state`.
fn slide_traverse(
    slab: &MobjSlab,
    handle: MobjHandle,
    level: &Level,
    x1: i32,
    y1: i32,
    dx: i32,
    dy: i32,
    scratch: &mut Vec<(i32, usize)>,
    state: &mut SlideState,
) {
    use crate::geom::p_point_on_line_side;

    let Some((mo_x, mo_y, mo_z, mo_height)) = slab
        .get(handle)
        .map(|mo| (mo.x.raw(), mo.y.raw(), mo.z.raw(), mo.height.raw()))
    else {
        return;
    };

    collect_slide_intercepts(level, x1, y1, dx, dy, scratch);

    for &(frac, ld_idx) in scratch.iter() {
        let Some(ld) = level.linedefs.get(ld_idx) else {
            continue;
        };

        let blocking;
        if !ld.is_two_sided() {
            // One-sided: only blocks from the front side.
            let g = line_geom(level, ld);
            if p_point_on_line_side(mo_x, mo_y, g.v1x, g.v1y, g.dx, g.dy) == 1 {
                continue; // don't hit the back side
            }
            blocking = true;
        } else {
            // Two-sided: P_LineOpening.
            let (open_bottom, open_top) = match crate::trace::line_opening(level, ld) {
                Some(o) => o,
                None => continue,
            };
            let opentop = open_top.raw();
            let openbottom = open_bottom.raw();
            let openrange = opentop - openbottom;

            if openrange < mo_height {
                blocking = true;
            } else if opentop - mo_z < mo_height {
                blocking = true;
            } else if openbottom - mo_z > 24 * FRACUNIT {
                blocking = true;
            } else {
                continue; // this line doesn't block movement
            }
        }

        if blocking {
            if frac < state.best_frac {
                state.best_frac = frac;
                state.best_line = Some(ld_idx);
            }
            return; // stop this trace at the first blocker
        }
    }
}

/// Port of `P_HitSlideLine` (`p_map.c`): clip `(tmxmove, tmymove)` so the next
/// move slides along `ld`.
fn p_hit_slide_line(
    level: &Level,
    ld_idx: usize,
    slide_x: i32,
    slide_y: i32,
    tmxmove: &mut i32,
    tmymove: &mut i32,
) {
    use crate::geom::{
        ANG180, fine_cosine, fine_sine, fixed_mul, p_aprox_distance, p_point_on_line_side,
        r_point_to_angle2,
    };

    let Some(ld) = level.linedefs.get(ld_idx) else {
        return;
    };
    let g = line_geom(level, ld);

    // ST_HORIZONTAL / ST_VERTICAL special cases.
    if g.dy == 0 {
        *tmymove = 0;
        return;
    }
    if g.dx == 0 {
        *tmxmove = 0;
        return;
    }

    let side = p_point_on_line_side(slide_x, slide_y, g.v1x, g.v1y, g.dx, g.dy);

    let mut lineangle = r_point_to_angle2(0, 0, g.dx, g.dy);
    if side == 1 {
        lineangle = lineangle.wrapping_add(ANG180);
    }

    let moveangle = r_point_to_angle2(0, 0, *tmxmove, *tmymove);
    let mut deltaangle = moveangle.wrapping_sub(lineangle);
    if deltaangle > ANG180 {
        deltaangle = deltaangle.wrapping_add(ANG180);
    }

    let movelen = p_aprox_distance(*tmxmove, *tmymove);
    let newlen = fixed_mul(movelen, fine_cosine(deltaangle));

    *tmxmove = fixed_mul(newlen, fine_cosine(lineangle));
    *tmymove = fixed_mul(newlen, fine_sine(lineangle));
}

/// Vanilla `P_SlideMove` (`p_map.c`): the blocked `momx/momy` move is retried
/// as a slide along the first wall hit. Mutates the actor's position and
/// momentum in place (via [`p_try_move_commit`]).
pub fn p_slide_move_vanilla(
    slab: &mut MobjSlab,
    handle: MobjHandle,
    level: &Level,
    sink: &mut Vec<usize>,
) {
    use crate::geom::fixed_mul;

    let mut scratch: Vec<(i32, usize)> = Vec::new();
    let mut hitcount = 0;

    loop {
        hitcount += 1;
        if hitcount == 3 {
            slide_stairstep(slab, handle, level, sink);
            return;
        }

        let Some((mo_x, mo_y, momx, momy, radius)) = slab.get(handle).map(|mo| {
            (
                mo.x.raw(),
                mo.y.raw(),
                mo.momx.raw(),
                mo.momy.raw(),
                mo.radius.raw(),
            )
        }) else {
            return;
        };

        // Trace along the three leading corners.
        let (leadx, trailx) = if momx > 0 {
            (mo_x + radius, mo_x - radius)
        } else {
            (mo_x - radius, mo_x + radius)
        };
        let (leady, traily) = if momy > 0 {
            (mo_y + radius, mo_y - radius)
        } else {
            (mo_y - radius, mo_y + radius)
        };

        let mut state = SlideState {
            best_frac: FRACUNIT + 1,
            best_line: None,
        };

        slide_traverse(
            slab,
            handle,
            level,
            leadx,
            leady,
            momx,
            momy,
            &mut scratch,
            &mut state,
        );
        slide_traverse(
            slab,
            handle,
            level,
            trailx,
            leady,
            momx,
            momy,
            &mut scratch,
            &mut state,
        );
        slide_traverse(
            slab,
            handle,
            level,
            leadx,
            traily,
            momx,
            momy,
            &mut scratch,
            &mut state,
        );

        // Move up to the wall.
        if state.best_frac == FRACUNIT + 1 {
            slide_stairstep(slab, handle, level, sink);
            return;
        }

        // Fudge a bit to make sure it doesn't hit.
        let mut best_frac = state.best_frac - 0x800;
        if best_frac > 0 {
            let newx = fixed_mul(momx, best_frac);
            let newy = fixed_mul(momy, best_frac);
            if !p_try_move_commit_tracked(
                slab,
                handle,
                Fixed16_16::from_raw(mo_x + newx),
                Fixed16_16::from_raw(mo_y + newy),
                level,
                sink,
            ) {
                slide_stairstep(slab, handle, level, sink);
                return;
            }
        }

        // Continue along the wall: compute the remainder.
        best_frac = FRACUNIT - (state.best_frac - 0x800 + 0x800);
        if best_frac > FRACUNIT {
            best_frac = FRACUNIT;
        }
        if best_frac <= 0 {
            return;
        }

        let mut tmxmove = fixed_mul(momx, best_frac);
        let mut tmymove = fixed_mul(momy, best_frac);

        if let Some(bl) = state.best_line {
            let (sx, sy) = slab
                .get(handle)
                .map(|mo| (mo.x.raw(), mo.y.raw()))
                .unwrap_or((mo_x, mo_y));
            p_hit_slide_line(level, bl, sx, sy, &mut tmxmove, &mut tmymove);
        }

        if let Some(mo) = slab.get_mut(handle) {
            mo.momx = Fixed16_16::from_raw(tmxmove);
            mo.momy = Fixed16_16::from_raw(tmymove);
        }

        let (cx, cy) = slab
            .get(handle)
            .map(|mo| (mo.x.raw(), mo.y.raw()))
            .unwrap_or((mo_x, mo_y));
        if p_try_move_commit_tracked(
            slab,
            handle,
            Fixed16_16::from_raw(cx + tmxmove),
            Fixed16_16::from_raw(cy + tmymove),
            level,
            sink,
        ) {
            return;
        }
        // else retry (loop again)
    }
}

/// Vanilla `stairstep` fallback inside `P_SlideMove`.
fn slide_stairstep(slab: &mut MobjSlab, handle: MobjHandle, level: &Level, sink: &mut Vec<usize>) {
    let Some((mo_x, mo_y, momx, momy)) = slab.get(handle).map(|mo| (mo.x, mo.y, mo.momx, mo.momy))
    else {
        return;
    };
    if !p_try_move_commit_tracked(slab, handle, mo_x, mo_y + momy, level, sink) {
        p_try_move_commit_tracked(slab, handle, mo_x + momx, mo_y, level, sink);
    }
}

/// Compute the Doom-shaped support floor under an actor at `(x, y)`.
///
/// This uses the actor's full bounding box rather than just the center point,
/// so a player descending stairs keeps the higher support floor until their
/// bbox fully clears the upper step.
#[must_use]
pub(crate) fn support_state_at(
    slab: &MobjSlab,
    handle: MobjHandle,
    x: Fixed16_16,
    y: Fixed16_16,
    level: &Level,
) -> Option<(Fixed16_16, Option<usize>)> {
    let (radius, fallback_z) = slab.get(handle).map(|mo| (mo.radius, mo.z))?;

    let left = x - radius;
    let right = x + radius;
    let bottom = y - radius;
    let top = y + radius;

    let bm = &level.blockmap;
    let x_origin = bm.x_origin as i32;
    let y_origin = bm.y_origin as i32;
    let x_count = bm.x_count as i32;
    let y_count = bm.y_count as i32;
    let to_block = |world: Fixed16_16, origin: i32, count: i32| -> usize {
        let cell = (world.to_int() - origin) / BLOCK_SIZE;
        cell.max(0).min(count - 1) as usize
    };

    let mut floor_z = level.floor_at(x.to_int(), y.to_int()).unwrap_or(fallback_z);

    let col_lo = to_block(left, x_origin, x_count);
    let col_hi = to_block(right, x_origin, x_count);
    let row_lo = to_block(bottom, y_origin, y_count);
    let row_hi = to_block(top, y_origin, y_count);

    for row in row_lo..=row_hi {
        for col in col_lo..=col_hi {
            for ld_idx in bm.block_linedefs(col, row) {
                let Some(ld) = level.linedefs.get(ld_idx as usize) else {
                    continue;
                };
                if !ld.is_two_sided() {
                    continue;
                }

                let v1 = &level.vertexes[ld.from_vertex as usize];
                let v2 = &level.vertexes[ld.to_vertex as usize];
                let lx1 = Fixed16_16::from_int(v1.x as i32);
                let ly1 = Fixed16_16::from_int(v1.y as i32);
                let lx2 = Fixed16_16::from_int(v2.x as i32);
                let ly2 = Fixed16_16::from_int(v2.y as i32);

                let lx_min = lx1.min(lx2);
                let lx_max = lx1.max(lx2);
                let ly_min = ly1.min(ly2);
                let ly_max = ly1.max(ly2);
                if right <= lx_min || left >= lx_max || top <= ly_min || bottom >= ly_max {
                    continue;
                }
                if !bbox_straddles_line(left, bottom, right, top, lx1, ly1, lx2, ly2) {
                    continue;
                }

                let Some(right_sd) = level.sidedefs.get(ld.right_sidedef as usize) else {
                    continue;
                };
                let Some(left_sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
                    continue;
                };
                let front = &level.sectors[right_sd.sector as usize];
                let back = &level.sectors[left_sd.sector as usize];
                let open_floor = front.floor_height.max(back.floor_height);
                floor_z = floor_z.max(open_floor);
            }
        }
    }

    Some((floor_z, level.subsector_index_at(x.to_int(), y.to_int())))
}

/// Vanilla `P_CheckPosition` line-opening scan over an actor's bounding box:
/// returns `(tmfloorz, tmceilingz, touches_target)`.
///
/// `tmfloorz` is the highest floor and `tmceilingz` the lowest ceiling opening
/// over the bbox (`P_LineOpening` accumulated across every two-sided line the
/// bbox straddles), seeded from the center subsector's sector. `touches_target`
/// reports whether any straddled two-sided line borders `target_sector` (or the
/// center is in it) — the `P_ChangeSector` "thing is in the moving sector's
/// blockbox" gate. Used by the corpse-crush path.
#[must_use]
pub(crate) fn bbox_open_heights(
    slab: &MobjSlab,
    handle: MobjHandle,
    x: Fixed16_16,
    y: Fixed16_16,
    level: &Level,
    target_sector: usize,
) -> Option<(Fixed16_16, Fixed16_16, bool)> {
    let radius = slab.get(handle).map(|mo| mo.radius)?;

    let left = x - radius;
    let right = x + radius;
    let bottom = y - radius;
    let top = y + radius;

    let center_sector = level.sector_index_at(x.to_int(), y.to_int());
    let (mut floor_z, mut ceil_z) = match center_sector.and_then(|si| level.sectors.get(si)) {
        Some(s) => (s.floor_height, s.ceil_height),
        None => (
            slab.get(handle).map(|mo| mo.z).unwrap_or(Fixed16_16::ZERO),
            Fixed16_16::from_int(32767),
        ),
    };
    let mut touches = center_sector == Some(target_sector);

    let bm = &level.blockmap;
    let x_origin = bm.x_origin as i32;
    let y_origin = bm.y_origin as i32;
    let x_count = bm.x_count as i32;
    let y_count = bm.y_count as i32;
    let to_block = |world: Fixed16_16, origin: i32, count: i32| -> usize {
        let cell = (world.to_int() - origin) / BLOCK_SIZE;
        cell.max(0).min(count - 1) as usize
    };

    let col_lo = to_block(left, x_origin, x_count);
    let col_hi = to_block(right, x_origin, x_count);
    let row_lo = to_block(bottom, y_origin, y_count);
    let row_hi = to_block(top, y_origin, y_count);

    for row in row_lo..=row_hi {
        for col in col_lo..=col_hi {
            for ld_idx in bm.block_linedefs(col, row) {
                let Some(ld) = level.linedefs.get(ld_idx as usize) else {
                    continue;
                };
                if !ld.is_two_sided() {
                    continue;
                }

                let v1 = &level.vertexes[ld.from_vertex as usize];
                let v2 = &level.vertexes[ld.to_vertex as usize];
                let lx1 = Fixed16_16::from_int(v1.x as i32);
                let ly1 = Fixed16_16::from_int(v1.y as i32);
                let lx2 = Fixed16_16::from_int(v2.x as i32);
                let ly2 = Fixed16_16::from_int(v2.y as i32);

                let lx_min = lx1.min(lx2);
                let lx_max = lx1.max(lx2);
                let ly_min = ly1.min(ly2);
                let ly_max = ly1.max(ly2);
                if right <= lx_min || left >= lx_max || top <= ly_min || bottom >= ly_max {
                    continue;
                }
                if !bbox_straddles_line(left, bottom, right, top, lx1, ly1, lx2, ly2) {
                    continue;
                }

                let Some(right_sd) = level.sidedefs.get(ld.right_sidedef as usize) else {
                    continue;
                };
                let Some(left_sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
                    continue;
                };
                let front = &level.sectors[right_sd.sector as usize];
                let back = &level.sectors[left_sd.sector as usize];
                // P_LineOpening: opening = [max(floors), min(ceilings)].
                let open_bottom = front.floor_height.max(back.floor_height);
                let open_top = front.ceil_height.min(back.ceil_height);
                floor_z = floor_z.max(open_bottom);
                ceil_z = ceil_z.min(open_top);
                if right_sd.sector as usize == target_sector
                    || left_sd.sector as usize == target_sector
                {
                    touches = true;
                }
            }
        }
    }

    Some((floor_z, ceil_z, touches))
}

fn clamp_i128_to_i32(v: i128) -> i32 {
    v.clamp(i32::MIN as i128, i32::MAX as i128) as i32
}

/// Vanilla `P_CheckPosition`'s thing pass (`P_BlockThingsIterator` /
/// `PIT_CheckThing`), reduced to the solid-clip subset relevant to a walking
/// monster: does the destination bounding box overlap a **solid** thing?
///
/// Vanilla checks things BEFORE lines and returns `false` from
/// `P_CheckPosition` on the first solid overlap — so when a thing blocks, the
/// line pass never runs and `numspechit` stays 0. Both `try_move_with_blocker`
/// (does the move succeed?) and `move_spechit` (which lines did the
/// blocked box cross?) must apply this same thing-first ordering, or a monster
/// blocked by another monster would spuriously accumulate `spechit` from a
/// special line beyond it and (via `P_Move`) clear its `movedir`, corrupting the
/// `olddir`/`turnaround` seed of the ensuing `P_NewChaseDir`.
///
/// Missiles and charging lost souls (`MF_SKULLFLY`) take damage-dealing
/// `PIT_CheckThing` branches handled elsewhere (the missile path in `tic.rs`,
/// `missile_check_things`), so the walk thing-block does not apply to them.
fn move_blocked_by_solid_thing(
    slab: &MobjSlab,
    handle: MobjHandle,
    radius: Fixed16_16,
    mo_flags: u32,
    new_x: Fixed16_16,
    new_y: Fixed16_16,
) -> bool {
    if mo_flags & (flags::MF_MISSILE | flags::MF_SKULLFLY) != 0 {
        return false;
    }
    for other in slab.iter_handles() {
        if other == handle {
            continue;
        }
        let Some(t) = slab.get(other) else {
            continue;
        };
        // Vanilla gate: only SOLID / SPECIAL / SHOOTABLE things are considered.
        if t.flags & (flags::MF_SOLID | flags::MF_SPECIAL | flags::MF_SHOOTABLE) == 0 {
            continue;
        }
        let blockdist = (t.radius + radius).raw();
        if (t.x.raw() - new_x.raw()).abs() >= blockdist
            || (t.y.raw() - new_y.raw()).abs() >= blockdist
        {
            // Bounding boxes don't overlap — no contact.
            continue;
        }
        // `PIT_CheckThing` returns `!(thing->flags & MF_SOLID)`: a solid thing
        // blocks the move; non-solid specials/shootables do not.
        if t.flags & flags::MF_SOLID != 0 {
            return true;
        }
    }
    false
}

fn try_move_with_blocker(
    slab: &MobjSlab,
    handle: MobjHandle,
    new_x: Fixed16_16,
    new_y: Fixed16_16,
    level: &Level,
) -> (bool, Option<BlockingLine>) {
    // Extract what we need, releasing the borrow before iterating blockmap.
    let (old_x, old_y, radius, height, mo_flags, mo_z) = match slab.get(handle) {
        Some(mo) => (mo.x, mo.y, mo.radius, mo.height, mo.flags, mo.z),
        None => return (false, None),
    };

    if mo_flags & flags::MF_NOCLIP != 0 {
        return (true, None);
    }

    // --- PIT_CheckThing (solid mobj-mobj clipping) ---
    // Vanilla `P_CheckPosition` iterates nearby things (`P_BlockThingsIterator`,
    // `PIT_CheckThing`) BEFORE lines: a move into any SOLID thing is blocked.
    if move_blocked_by_solid_thing(slab, handle, radius, mo_flags, new_x, new_y) {
        return (false, None);
    }

    let current_floor = level.floor_at(old_x.to_int(), old_y.to_int());
    let step_base_z = current_floor.map_or(mo_z, |floor_z| mo_z.max(floor_z));

    // Proposed bounding box.
    let left = new_x - radius;
    let right = new_x + radius;
    let bottom = new_y - radius;
    let top = new_y + radius;

    let bm = &level.blockmap;
    let x_origin = bm.x_origin as i32;
    let y_origin = bm.y_origin as i32;
    let x_count = bm.x_count as i32;
    let y_count = bm.y_count as i32;

    // Convert a world coordinate to a blockmap column/row index.
    let to_block = |world: Fixed16_16, origin: i32, count: i32| -> usize {
        let cell = (world.to_int() - origin) / BLOCK_SIZE;
        cell.max(0).min(count - 1) as usize
    };

    let col_lo = to_block(left, x_origin, x_count);
    let col_hi = to_block(right, x_origin, x_count);
    let row_lo = to_block(bottom, y_origin, y_count);
    let row_hi = to_block(top, y_origin, y_count);

    // Iterate blockmap cells covered by the bounding box.
    for row in row_lo..=row_hi {
        for col in col_lo..=col_hi {
            for ld_idx in bm.block_linedefs(col, row) {
                let Some(ld) = level.linedefs.get(ld_idx as usize) else {
                    continue;
                };

                let v1 = &level.vertexes[ld.from_vertex as usize];
                let v2 = &level.vertexes[ld.to_vertex as usize];

                let lx1 = Fixed16_16::from_int(v1.x as i32);
                let ly1 = Fixed16_16::from_int(v1.y as i32);
                let lx2 = Fixed16_16::from_int(v2.x as i32);
                let ly2 = Fixed16_16::from_int(v2.y as i32);

                // --- AABB rejection ---
                let lx_min = lx1.min(lx2);
                let lx_max = lx1.max(lx2);
                let ly_min = ly1.min(ly2);
                let ly_max = ly1.max(ly2);
                if right <= lx_min || left >= lx_max || top <= ly_min || bottom >= ly_max {
                    continue;
                }

                // --- Line-straddling test ---
                if !bbox_straddles_line(left, bottom, right, top, lx1, ly1, lx2, ly2) {
                    continue;
                }

                // --- One-sided: always blocked ---
                if !ld.is_two_sided() {
                    return (
                        false,
                        Some(BlockingLine {
                            linedef_idx: ld_idx as usize,
                            x1: lx1,
                            y1: ly1,
                            x2: lx2,
                            y2: ly2,
                        }),
                    );
                }

                // --- Two-sided: ML_BLOCKING / ML_BLOCKMONSTERS ---
                // Vanilla `PIT_CheckLine` gates both on `!(tmthing->flags &
                // MF_MISSILE)`: missiles pass through impassable and
                // block-monsters lines (they explode on geometry/openings, not
                // on these flags). Applying ML_BLOCKING to a missile made an
                // ImpFireball explode one tic early against an impassable
                // two-sided line (DEMO3/E1M7 lt1411 line 338), reordering its
                // P_ExplodeMissile RNG draw ahead of a trooper's A_PosAttack and
                // corrupting that shot's angle spread.
                let is_missile = mo_flags & flags::MF_MISSILE != 0;
                if !is_missile {
                    if ld.flags & doom_map::lumps::FLAG_BLOCKING != 0 {
                        return (
                            false,
                            Some(BlockingLine {
                                linedef_idx: ld_idx as usize,
                                x1: lx1,
                                y1: ly1,
                                x2: lx2,
                                y2: ly2,
                            }),
                        );
                    }
                    let is_monster = slab
                        .get(handle)
                        .map(|mo| mo.flags & flags::MF_COUNTKILL != 0)
                        .unwrap_or(false);
                    if ld.flags & doom_map::lumps::FLAG_BLOCKMONSTERS != 0 && is_monster {
                        return (
                            false,
                            Some(BlockingLine {
                                linedef_idx: ld_idx as usize,
                                x1: lx1,
                                y1: ly1,
                                x2: lx2,
                                y2: ly2,
                            }),
                        );
                    }
                }

                // --- Two-sided: check opening ---
                let Some(right_sd) = level.sidedefs.get(ld.right_sidedef as usize) else {
                    return (
                        false,
                        Some(BlockingLine {
                            linedef_idx: ld_idx as usize,
                            x1: lx1,
                            y1: ly1,
                            x2: lx2,
                            y2: ly2,
                        }),
                    );
                };
                let Some(left_sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
                    return (
                        false,
                        Some(BlockingLine {
                            linedef_idx: ld_idx as usize,
                            x1: lx1,
                            y1: ly1,
                            x2: lx2,
                            y2: ly2,
                        }),
                    );
                };
                let front = &level.sectors[right_sd.sector as usize];
                let back = &level.sectors[left_sd.sector as usize];

                let open_floor = front.floor_height.max(back.floor_height);
                let open_ceil = front.ceil_height.min(back.ceil_height);
                let dropoff_floor = front.floor_height.min(back.floor_height);

                // Gap too small for actor to fit.
                if open_ceil - open_floor < height {
                    return (
                        false,
                        Some(BlockingLine {
                            linedef_idx: ld_idx as usize,
                            x1: lx1,
                            y1: ly1,
                            x2: lx2,
                            y2: ly2,
                        }),
                    );
                }

                // Head hits the ceiling: the actor must lower itself to fit.
                // Vanilla `P_TryMove`: `if (tmceilingz - thing->z < thing->height)
                // return false;`. Without this a player descending stairs toward a
                // low-ceiling doorway (DEMO3/E1M7 leveltime 534: ceiling 72, z 19,
                // 72 - 19 = 53 < 56) walks through instead of being blocked until it
                // has dropped low enough. The slide path already gates on this
                // (`slide_traverse`); the straight move must too.
                if open_ceil - mo_z < height {
                    return (
                        false,
                        Some(BlockingLine {
                            linedef_idx: ld_idx as usize,
                            x1: lx1,
                            y1: ly1,
                            x2: lx2,
                            y2: ly2,
                        }),
                    );
                }

                // Step too high to climb.
                if open_floor - step_base_z > MAX_STEP_HEIGHT {
                    return (
                        false,
                        Some(BlockingLine {
                            linedef_idx: ld_idx as usize,
                            x1: lx1,
                            y1: ly1,
                            x2: lx2,
                            y2: ly2,
                        }),
                    );
                }

                if mo_flags & (flags::MF_DROPOFF | flags::MF_FLOAT) == 0
                    && open_floor - dropoff_floor > MAX_STEP_HEIGHT
                {
                    return (
                        false,
                        Some(BlockingLine {
                            linedef_idx: ld_idx as usize,
                            x1: lx1,
                            y1: ly1,
                            x2: lx2,
                            y2: ly2,
                        }),
                    );
                }
            }
        }
    }

    (true, None)
}

/// Faithful port of the `spechit` accumulation performed by vanilla
/// `P_CheckPosition`'s line pass (`PIT_CheckLine`, `p_map.c`).
///
/// Returns the ordered list of special linedefs the actor's bounding box at
/// `(new_x, new_y)` crosses, exactly as vanilla builds the global `spechit[]`
/// array: it walks the blockmap cells the box overlaps in `bx`-outer / `by`-inner
/// order (never expanded by `MAXRADIUS` — that expansion is only for the *thing*
/// pass), processing each linedef at most once (vanilla `validcount`), and for
/// each line that the box actually straddles it:
///  - stops the whole scan on the first hard blocker — a one-sided line, or (for
///    non-missiles) an `ML_BLOCKING` line, or an `ML_BLOCKMONSTERS` line when the
///    mover is a monster — because vanilla `PIT_CheckLine` returns `false` there,
///    freezing `spechit` at whatever it had accumulated so far;
///  - otherwise (a two-sided passable line) appends it to the list when it has a
///    special, regardless of the opening height (the fit/step/dropoff rejection
///    is deferred to `P_TryMove` and never gates `spechit`).
///
/// This is what `P_Move` consults via `numspechit` to decide whether a blocked
/// monster halts (`movedir = DI_NODIR`) and tries the crossed lines as doors,
/// and what `P_TryMove` walks (via `record_player_crossings`) after a
/// successful player/monster step to fire `P_CrossSpecialLine`. Unlike the
/// movement bool (`try_move_with_blocker`), which early-returns at the first
/// blocker, this must see every special line crossed *before* the blocker, so it
/// is computed separately.
///
/// The vanilla `!tmthing->player` guard on `ML_BLOCKMONSTERS` is expressed here
/// via `MF_COUNTKILL` (players and non-monster things carry no `MF_COUNTKILL`),
/// so this same routine serves the player, monster and missile spechit passes.
#[must_use]
pub fn move_spechit(
    slab: &MobjSlab,
    handle: MobjHandle,
    new_x: Fixed16_16,
    new_y: Fixed16_16,
    level: &Level,
) -> Vec<usize> {
    let (radius, mo_flags) = match slab.get(handle) {
        Some(mo) => (mo.radius, mo.flags),
        None => return Vec::new(),
    };

    let mut spechit: Vec<usize> = Vec::new();

    if mo_flags & flags::MF_NOCLIP != 0 {
        return spechit;
    }

    // Vanilla `P_CheckPosition` checks THINGS before LINES: if a solid thing
    // blocks the destination box, `P_CheckPosition` returns false before the
    // line pass runs, so `numspechit` stays 0. Replicate that ordering — without
    // it a monster blocked by another monster spuriously collects a special line
    // beyond the blocker, and `P_Move` clears its `movedir`, corrupting the
    // `olddir`/`turnaround` seed handed to `P_NewChaseDir` (DEMO3/E1M7: an imp
    // at lt611 picked its move-away direction instead of vanilla's, shifting its
    // chase step and desyncing the demo).
    if move_blocked_by_solid_thing(slab, handle, radius, mo_flags, new_x, new_y) {
        return spechit;
    }

    let is_missile = mo_flags & flags::MF_MISSILE != 0;
    // `P_Move` is only ever called for non-player monsters, so the vanilla
    // `!tmthing->player` guard on `ML_BLOCKMONSTERS` is always satisfied here.
    let blocks_monsters = mo_flags & flags::MF_COUNTKILL != 0;

    let left = new_x - radius;
    let right = new_x + radius;
    let bottom = new_y - radius;
    let top = new_y + radius;

    let bm = &level.blockmap;
    let x_origin = bm.x_origin as i32;
    let y_origin = bm.y_origin as i32;
    let x_count = bm.x_count as i32;
    let y_count = bm.y_count as i32;

    let to_block = |world: Fixed16_16, origin: i32, count: i32| -> usize {
        let cell = (world.to_int() - origin) / BLOCK_SIZE;
        cell.max(0).min(count - 1) as usize
    };

    let col_lo = to_block(left, x_origin, x_count);
    let col_hi = to_block(right, x_origin, x_count);
    let row_lo = to_block(bottom, y_origin, y_count);
    let row_hi = to_block(top, y_origin, y_count);

    // Vanilla `validcount`: each linedef is examined once across the whole scan.
    let mut seen: Vec<usize> = Vec::new();

    // Vanilla iterates `for (bx...) for (by...)` — column-major.
    for col in col_lo..=col_hi {
        for row in row_lo..=row_hi {
            for ld_idx in bm.block_linedefs(col, row) {
                let ld_idx = ld_idx as usize;
                if seen.contains(&ld_idx) {
                    continue;
                }
                seen.push(ld_idx);

                let Some(ld) = level.linedefs.get(ld_idx) else {
                    continue;
                };

                let v1 = &level.vertexes[ld.from_vertex as usize];
                let v2 = &level.vertexes[ld.to_vertex as usize];
                let lx1 = Fixed16_16::from_int(v1.x as i32);
                let ly1 = Fixed16_16::from_int(v1.y as i32);
                let lx2 = Fixed16_16::from_int(v2.x as i32);
                let ly2 = Fixed16_16::from_int(v2.y as i32);

                // Line bbox rejection (vanilla PIT_CheckLine first test).
                let lx_min = lx1.min(lx2);
                let lx_max = lx1.max(lx2);
                let ly_min = ly1.min(ly2);
                let ly_max = ly1.max(ly2);
                if right <= lx_min || left >= lx_max || top <= ly_min || bottom >= ly_max {
                    continue;
                }

                // P_BoxOnLineSide != -1 → the box does not straddle the line.
                if !bbox_straddles_line(left, bottom, right, top, lx1, ly1, lx2, ly2) {
                    continue;
                }

                // A line has been hit.
                if !ld.is_two_sided() {
                    return spechit; // one-sided → PIT_CheckLine returns false
                }
                if !is_missile {
                    if ld.flags & doom_map::lumps::FLAG_BLOCKING != 0 {
                        return spechit; // explicitly blocking everything
                    }
                    if blocks_monsters && ld.flags & doom_map::lumps::FLAG_BLOCKMONSTERS != 0 {
                        return spechit; // block monsters only
                    }
                }

                // Two-sided passable line: opening heights are irrelevant to
                // spechit; only the special membership matters.
                if ld.special != 0 {
                    spechit.push(ld_idx);
                }
            }
        }
    }

    spechit
}

/// Vanilla `P_TryMove`'s post-move `spechit` walk (`p_map.c`), for the player.
///
/// After a successful move vanilla runs:
/// ```c
/// while (numspechit--) {
///     ld = spechit[numspechit];
///     side = P_PointOnLineSide (thing->x, thing->y, ld);
///     oldside = P_PointOnLineSide (oldx, oldy, ld);
///     if (side != oldside)
///         if (ld->special)
///             P_CrossSpecialLine (ld-lines, oldside, thing);
/// }
/// ```
/// i.e. for every special line the destination box straddled (the [`move_spechit`]
/// list), in reverse accumulation order, fire the crossing iff the centre's
/// **infinite-line** side changed between the old and new position. `spechit`
/// already holds only special lines, so `ld->special` is implied.
///
/// `sink` receives the `linedef` indices to cross, in the vanilla
/// `while(numspechit--)` reverse order; the caller dispatches them. The exact
/// fixed-point [`p_point_on_line_side`](crate::geom::p_point_on_line_side) is used
/// on the raw 16.16 centre coordinates, matching vanilla `P_PointOnLineSide`
/// (never the truncated-integer segment intersection the old detection used).
fn record_player_crossings(
    slab: &MobjSlab,
    handle: MobjHandle,
    old_x: Fixed16_16,
    old_y: Fixed16_16,
    new_x: Fixed16_16,
    new_y: Fixed16_16,
    level: &Level,
    sink: &mut Vec<usize>,
) {
    use crate::geom::p_point_on_line_side;

    let spechit = move_spechit(slab, handle, new_x, new_y, level);
    for &ld_idx in spechit.iter().rev() {
        let Some(ld) = level.linedefs.get(ld_idx) else {
            continue;
        };
        let g = line_geom(level, ld);
        let side = p_point_on_line_side(new_x.raw(), new_y.raw(), g.v1x, g.v1y, g.dx, g.dy);
        let oldside = p_point_on_line_side(old_x.raw(), old_y.raw(), g.v1x, g.v1y, g.dx, g.dy);
        if side != oldside {
            sink.push(ld_idx);
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: does the actor bounding box straddle the linedef?
// ---------------------------------------------------------------------------

/// Returns `true` if the AABB `(left, bottom, right, top)` straddles the
/// infinite line passing through `(x1, y1)` and `(x2, y2)`.
///
/// This is a port of Doom's `P_BoxOnLineSide`: choose two "extreme" corners
/// of the bbox based on the line's quadrant, then check if they have opposite
/// signs of the cross product with the line direction.
fn bbox_straddles_line(
    left: Fixed16_16,
    bottom: Fixed16_16,
    right: Fixed16_16,
    top: Fixed16_16,
    x1: Fixed16_16,
    y1: Fixed16_16,
    x2: Fixed16_16,
    y2: Fixed16_16,
) -> bool {
    p_box_on_line_side(left, bottom, right, top, x1, y1, x2, y2) == -1
}

/// Faithful port of Doom's `P_BoxOnLineSide` (`p_maputl.c`), operating on the
/// raw 16.16 fixed-point coordinates of the bbox and the linedef.
///
/// Returns `-1` if the box straddles the line (the two extreme corners lie on
/// opposite sides), otherwise the common side (`0` = front, `1` = back). The
/// slope-type dispatch and the `P_PointOnLineSide` calls match vanilla exactly,
/// including operating on the full fractional coordinates rather than truncated
/// integers — the fractional bits are decisive for corner-tangent cases.
pub(crate) fn p_box_on_line_side(
    left: Fixed16_16,
    bottom: Fixed16_16,
    right: Fixed16_16,
    top: Fixed16_16,
    x1: Fixed16_16,
    y1: Fixed16_16,
    x2: Fixed16_16,
    y2: Fixed16_16,
) -> i32 {
    use crate::geom::p_point_on_line_side;

    let (left, bottom, right, top) = (left.raw(), bottom.raw(), right.raw(), top.raw());
    let (v1x, v1y) = (x1.raw(), y1.raw());
    let ldx = x2.raw().wrapping_sub(v1x);
    let ldy = y2.raw().wrapping_sub(v1y);

    let (mut p1, mut p2);
    if ldx == 0 {
        // ST_VERTICAL
        p1 = (right < v1x) as i32;
        p2 = (left < v1x) as i32;
        if ldy < 0 {
            p1 ^= 1;
            p2 ^= 1;
        }
    } else if ldy == 0 {
        // ST_HORIZONTAL
        p1 = (top > v1y) as i32;
        p2 = (bottom > v1y) as i32;
        if ldx < 0 {
            p1 ^= 1;
            p2 ^= 1;
        }
    } else if (ldx > 0) == (ldy > 0) {
        // ST_POSITIVE (FixedDiv(dy, dx) > 0)
        p1 = p_point_on_line_side(left, top, v1x, v1y, ldx, ldy);
        p2 = p_point_on_line_side(right, bottom, v1x, v1y, ldx, ldy);
    } else {
        // ST_NEGATIVE
        p1 = p_point_on_line_side(right, top, v1x, v1y, ldx, ldy);
        p2 = p_point_on_line_side(left, bottom, v1x, v1y, ldx, ldy);
    }

    if p1 == p2 { p1 } else { -1 }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjSlab};
    use doom_map::{Node, NodeBBox, lumps::NODE_SUBSECTOR_BIT};
    use doom_types::Bam;
    use doom_types::mobj_kind::MobjKind;

    // -----------------------------------------------------------------------
    // Minimal Level builder — 1×1 blockmap, no linedefs
    // -----------------------------------------------------------------------

    fn make_open_level() -> doom_map::Level {
        // 1×1 blockmap at origin with one empty block.
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count
        // offset table: block 0 is at word-offset 5 from start of lump.
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes()); // sentinel
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).expect("value must exist in test");

        let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("value must exist in test");

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(128),
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

    fn make_level_with_one_sided_walls(walls: &[(i16, i16, i16, i16)]) -> doom_map::Level {
        use doom_map::{Blockmap, Linedef, Reject, SIDEDEF_NONE, Sector, Sidedef, Vertex};

        let mut vertexes = Vec::with_capacity(walls.len() * 2);
        let mut linedefs = Vec::with_capacity(walls.len());
        for &(x1, y1, x2, y2) in walls {
            let from_vertex = vertexes.len() as u16;
            vertexes.push(Vertex { x: x1, y: y1 });
            let to_vertex = vertexes.len() as u16;
            vertexes.push(Vertex { x: x2, y: y2 });
            linedefs.push(Linedef {
                from_vertex,
                to_vertex,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: SIDEDEF_NONE,
            });
        }

        let sidedefs = vec![Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: [0; 8],
            lower_texture: [0; 8],
            middle_texture: *b"WALL1\0\0\0",
            sector: 0,
        }];
        let sectors = vec![Sector {
            floor_height: doom_types::Fixed16_16::from_int(0),
            ceil_height: doom_types::Fixed16_16::from_int(128),
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }];

        // 1x1 blockmap at origin with all wall linedefs in the single cell.
        let mut bm_data = Vec::new();
        bm_data.extend_from_slice(&0i16.to_le_bytes()); // x_origin
        bm_data.extend_from_slice(&0i16.to_le_bytes()); // y_origin
        bm_data.extend_from_slice(&1u16.to_le_bytes()); // x_count
        bm_data.extend_from_slice(&1u16.to_le_bytes()); // y_count
        let data_start = 4u16 + 1; // header words + 1 offset word
        bm_data.extend_from_slice(&data_start.to_le_bytes());
        bm_data.extend_from_slice(&0u16.to_le_bytes()); // sentinel
        for ld_idx in 0..linedefs.len() {
            bm_data.extend_from_slice(&(ld_idx as u16).to_le_bytes());
        }
        bm_data.extend_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
        let blockmap = Blockmap::parse_lump(&bm_data).expect("value must exist in test");
        let reject = Reject::parse_lump(&[0u8], 1).expect("value must exist in test");

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    fn make_two_sided_step_level(front_floor: i16, back_floor: i16) -> doom_map::Level {
        use doom_map::{Blockmap, FLAG_TWO_SIDED, Linedef, Reject, Sector, Sidedef, Vertex};

        let vertexes = vec![Vertex { x: 64, y: 0 }, Vertex { x: 64, y: 128 }];
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: FLAG_TWO_SIDED,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];
        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
        ];
        let sectors = vec![
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(i32::from(front_floor)),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(i32::from(back_floor)),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];

        let mut bm_data = Vec::new();
        bm_data.extend_from_slice(&0i16.to_le_bytes());
        bm_data.extend_from_slice(&0i16.to_le_bytes());
        bm_data.extend_from_slice(&1u16.to_le_bytes());
        bm_data.extend_from_slice(&1u16.to_le_bytes());
        let data_start = 4u16 + 1;
        bm_data.extend_from_slice(&data_start.to_le_bytes());
        bm_data.extend_from_slice(&0u16.to_le_bytes());
        bm_data.extend_from_slice(&0u16.to_le_bytes());
        bm_data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("value must exist in test");
        let reject = Reject::parse_lump(&[0u8], 2).expect("value must exist in test");

        doom_map::Level {
            name: "STEP".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    /// Two flat-floored sectors sharing a two-sided line at x=64. The back
    /// sector (1) has a parametrized (low) ceiling; the front sector (0) is tall.
    fn make_two_sided_low_ceiling_level(back_ceil: i16) -> doom_map::Level {
        let mut level = make_two_sided_step_level(0, 0);
        level.sectors[1].ceil_height = doom_types::Fixed16_16::from_int(i32::from(back_ceil));
        level
    }

    #[test]
    fn try_move_blocked_when_head_hits_low_ceiling() {
        // Vanilla P_TryMove: `if (tmceilingz - thing->z < thing->height) return
        // false;`. Back ceiling 72, player height 56: a player at z=24 (72-24=48
        // < 56) must be blocked until it lowers itself. Regression for DEMO3/E1M7
        // leveltime 534.
        let level = make_two_sided_low_ceiling_level(72);
        let (mut slab, handle) = make_player_slab();
        slab.get_mut(handle).expect("player").z = Fixed16_16::from_int(24);
        let new_x = Fixed16_16::from_int(70); // crosses the x=64 line into sector 1
        let new_y = Fixed16_16::from_int(64);
        assert!(
            !p_try_move(&slab, handle, new_x, new_y, &level),
            "head at z+height=80 must not fit under a 72 ceiling"
        );
    }

    #[test]
    fn try_move_allowed_when_low_enough_for_ceiling() {
        // Same geometry, but the player has descended to z=16 (72-16=56, not
        // < 56): it now fits and the move succeeds.
        let level = make_two_sided_low_ceiling_level(72);
        let (mut slab, handle) = make_player_slab();
        slab.get_mut(handle).expect("player").z = Fixed16_16::from_int(16);
        let new_x = Fixed16_16::from_int(70);
        let new_y = Fixed16_16::from_int(64);
        assert!(
            p_try_move(&slab, handle, new_x, new_y, &level),
            "head at z+height=72 fits exactly under a 72 ceiling"
        );
    }

    /// Vanilla `PIT_CheckLine` gates the `ML_BLOCKING`/`ML_BLOCKMONSTERS`
    /// checks on `!(tmthing->flags & MF_MISSILE)`: a missile flies THROUGH an
    /// impassable two-sided line (it only explodes on geometry/openings), while
    /// a non-missile actor is stopped by it. Regression for DEMO3/E1M7 lt1411:
    /// an ImpFireball wrongly exploded one tic early against impassable line
    /// 338, reordering its P_ExplodeMissile RNG draw ahead of a trooper's
    /// A_PosAttack and corrupting that shot's angle spread (player never took
    /// the hit, position diverged at lt1414).
    #[test]
    fn missile_passes_through_impassable_two_sided_line() {
        use doom_map::lumps::FLAG_BLOCKING;
        // Flat floors/ceilings, no opening obstruction — the ONLY thing that
        // could block is the ML_BLOCKING flag on the shared two-sided line.
        let mut level = make_two_sided_step_level(0, 0);
        level.linedefs[0].flags |= FLAG_BLOCKING;

        let new_x = Fixed16_16::from_int(70); // crosses the x=64 line into sector 1
        let new_y = Fixed16_16::from_int(64);

        // A monster (non-missile) is stopped by the impassable line.
        let (mut slab, handle) = make_player_slab();
        if let Some(mo) = slab.get_mut(handle) {
            mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
            mo.height = Fixed16_16::from_int(56);
        }
        assert!(
            !p_try_move(&slab, handle, new_x, new_y, &level),
            "a non-missile actor must be blocked by an ML_BLOCKING line"
        );

        // A missile passes through the same impassable line.
        let (mut mslab, mhandle) = make_player_slab();
        if let Some(mo) = mslab.get_mut(mhandle) {
            mo.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_DROPOFF;
            mo.radius = Fixed16_16::from_int(6);
            mo.height = Fixed16_16::from_int(8);
            mo.z = Fixed16_16::from_int(32);
        }
        assert!(
            p_try_move(&mslab, mhandle, new_x, new_y, &level),
            "a missile (MF_MISSILE) must pass through an ML_BLOCKING two-sided line"
        );
    }

    fn make_partition_step_level(right_floor: i16, left_floor: i16) -> doom_map::Level {
        use doom_map::{
            Blockmap, FLAG_TWO_SIDED, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Vertex,
        };

        let vertexes = vec![
            Vertex { x: 64, y: -128 },
            Vertex { x: 64, y: 128 },
            Vertex { x: 0, y: -128 },
            Vertex { x: 0, y: 128 },
        ];
        let linedefs = vec![
            Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: FLAG_TWO_SIDED,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: FLAG_TWO_SIDED,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
        ];
        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
        ];
        let sectors = vec![
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(i32::from(right_floor)),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(i32::from(left_floor)),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        let segs = vec![
            Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            },
            Seg {
                from_vertex: 3,
                to_vertex: 2,
                angle: 0,
                linedef: 1,
                direction: 1,
                offset: 0,
            },
        ];
        let ssectors = vec![
            Ssector {
                seg_count: 1,
                first_seg: 0,
            },
            Ssector {
                seg_count: 1,
                first_seg: 1,
            },
        ];
        let nodes = vec![Node {
            x: 64,
            y: 0,
            dx: 0,
            dy: 1,
            right_bbox: NodeBBox {
                ymax: 128,
                ymin: -128,
                xmin: 64,
                xmax: 256,
            },
            left_bbox: NodeBBox {
                ymax: 128,
                ymin: -128,
                xmin: -128,
                xmax: 64,
            },
            right_child: NODE_SUBSECTOR_BIT,
            left_child: NODE_SUBSECTOR_BIT | 1,
        }];

        let mut bm_data = Vec::new();
        bm_data.extend_from_slice(&0i16.to_le_bytes());
        bm_data.extend_from_slice(&0i16.to_le_bytes());
        bm_data.extend_from_slice(&2u16.to_le_bytes());
        bm_data.extend_from_slice(&1u16.to_le_bytes());
        let data_start = 4u16 + 2;
        bm_data.extend_from_slice(&data_start.to_le_bytes());
        bm_data.extend_from_slice(&(data_start + 4).to_le_bytes());
        bm_data.extend_from_slice(&0u16.to_le_bytes());
        bm_data.extend_from_slice(&0u16.to_le_bytes());
        bm_data.extend_from_slice(&1u16.to_le_bytes());
        bm_data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        bm_data.extend_from_slice(&0u16.to_le_bytes());
        bm_data.extend_from_slice(&1u16.to_le_bytes());
        bm_data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("value must exist in test");
        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");

        doom_map::Level {
            name: "DROP".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes,
            sectors,
            reject,
            blockmap,
        }
    }

    fn make_player_slab() -> (MobjSlab, MobjHandle) {
        let mut slab = MobjSlab::new();
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        mo.radius = Fixed16_16::from_int(16);
        mo.height = Fixed16_16::from_int(56);
        let handle = slab.alloc(mo);
        (slab, handle)
    }

    fn make_monster_slab(x: i32, y: i32, z: i32) -> (MobjSlab, MobjHandle) {
        let mut slab = MobjSlab::new();
        let mut mo = Mobj::new(
            MobjKind::Imp,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 60;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.radius = Fixed16_16::from_int(20);
        mo.height = Fixed16_16::from_int(56);
        mo.z = Fixed16_16::from_int(z);
        let handle = slab.alloc(mo);
        (slab, handle)
    }

    #[test]
    fn monster_spechit_thing_block_suppresses_special_line() {
        // Vanilla `P_CheckPosition` checks THINGS before LINES. When a solid
        // thing blocks the destination box, `P_CheckPosition` returns false
        // before the line pass runs, so `numspechit` stays 0. This mirrors the
        // DEMO3/E1M7 lt611 imp: it was blocked by another imp to its south, yet
        // our `spechit` pass reported a special line beyond the blocker, causing
        // `P_Move` to clear `movedir` and corrupt the `P_NewChaseDir` seed.
        //
        // Geometry: a two-sided *special* line at x=64. An imp (radius 20) at
        // (50,64) moving to (60,64) has a destination box spanning x∈[40,80],
        // which straddles the x=64 line — so absent a blocker the spechit pass
        // must return that one special line.
        let mut level = make_two_sided_step_level(0, 0);
        level.linedefs[0].special = 88; // WR lift — any nonzero special line

        let (mut slab, mover) = make_monster_slab(50, 64, 0);
        let new_x = Fixed16_16::from_int(60);
        let new_y = Fixed16_16::from_int(64);

        // Baseline: no other thing → the straddled special line is collected.
        let sh = move_spechit(&slab, mover, new_x, new_y, &level);
        assert_eq!(
            sh,
            vec![0usize],
            "box straddling a two-sided special line must collect it"
        );

        // Add a solid thing overlapping the destination box (thing-first block).
        let mut blocker = Mobj::new(MobjKind::Imp, new_x, new_y, Bam::ZERO);
        blocker.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        blocker.radius = Fixed16_16::from_int(20);
        blocker.height = Fixed16_16::from_int(56);
        slab.alloc(blocker);

        let sh_blocked = move_spechit(&slab, mover, new_x, new_y, &level);
        assert!(
            sh_blocked.is_empty(),
            "a solid thing blocking the destination must suppress the line pass \
             (vanilla numspechit == 0), got {sh_blocked:?}"
        );
    }

    #[test]
    fn open_level_allows_all_movement() {
        let level = make_open_level();
        let (slab, handle) = make_player_slab();
        assert!(
            p_try_move(
                &slab,
                handle,
                Fixed16_16::from_int(50),
                Fixed16_16::from_int(50),
                &level
            ),
            "empty level must allow all movement"
        );
    }

    #[test]
    fn noclip_bypasses_any_check() {
        let level = make_open_level();
        let (mut slab, handle) = make_player_slab();
        slab.get_mut(handle)
            .expect("value must exist in test")
            .flags |= flags::MF_NOCLIP;
        // Even with impossible coordinates, noclip always succeeds.
        assert!(p_try_move(
            &slab,
            handle,
            Fixed16_16::from_int(99999),
            Fixed16_16::from_int(99999),
            &level
        ));
    }

    #[test]
    fn stale_handle_returns_false() {
        let level = make_open_level();
        let (mut slab, handle) = make_player_slab();
        slab.free(handle);
        assert!(!p_try_move(
            &slab,
            handle,
            Fixed16_16::from_int(10),
            Fixed16_16::from_int(10),
            &level
        ));
    }

    #[test]
    fn bbox_straddles_line_basic() {
        // Horizontal wall at y=0.  Actor bbox from y=-10 to y=+10 straddles it.
        let straddles = bbox_straddles_line(
            Fixed16_16::from_int(-10),
            Fixed16_16::from_int(-10),
            Fixed16_16::from_int(10),
            Fixed16_16::from_int(10),
            Fixed16_16::from_int(-100),
            Fixed16_16::ZERO,
            Fixed16_16::from_int(100),
            Fixed16_16::ZERO,
        );
        assert!(straddles);
    }

    #[test]
    fn bbox_does_not_straddle_far_line() {
        // Wall at y=1000, actor at y=-10..+10.
        let straddles = bbox_straddles_line(
            Fixed16_16::from_int(-10),
            Fixed16_16::from_int(-10),
            Fixed16_16::from_int(10),
            Fixed16_16::from_int(10),
            Fixed16_16::from_int(-100),
            Fixed16_16::from_int(1000),
            Fixed16_16::from_int(100),
            Fixed16_16::from_int(1000),
        );
        assert!(!straddles);
    }

    #[test]
    fn max_step_height_is_24_units() {
        assert_eq!(MAX_STEP_HEIGHT.to_int(), 24);
    }

    #[test]
    fn bbox_straddles_positive_slope_corner_tangent() {
        // Regression for the DEMO1/E1M5 wall-slide clip (leveltime 62): a
        // one-sided ST_POSITIVE wall from (-416,-112)->(-320,-96) with the
        // player (radius 16) at fixed (-25353890, -8245162). The player's
        // top-left bbox corner lands essentially tangent to the line; vanilla
        // `P_BoxOnLineSide` returns -1 (straddles → blocked) thanks to the
        // fractional coordinate bits, so this move must be rejected. The old
        // integer-truncating implementation wrongly reported no straddle and
        // let the player slip ~3.46 units into the wall.
        let radius = Fixed16_16::from_int(16);
        let px = Fixed16_16::from_raw(-25353890);
        let py = Fixed16_16::from_raw(-8245162);
        let straddles = bbox_straddles_line(
            px - radius,
            py - radius,
            px + radius,
            py + radius,
            Fixed16_16::from_int(-416),
            Fixed16_16::from_int(-112),
            Fixed16_16::from_int(-320),
            Fixed16_16::from_int(-96),
        );
        assert!(
            straddles,
            "corner-tangent positive-slope wall must straddle"
        );
    }

    #[test]
    fn slide_move_slides_along_vertical_wall() {
        let level = make_level_with_one_sided_walls(&[(64, 0, 64, 128)]);
        let (slab, handle) = make_player_slab();

        let old_x = Fixed16_16::from_int(32);
        let old_y = Fixed16_16::from_int(32);
        let new_x = Fixed16_16::from_int(70);
        let new_y = Fixed16_16::from_int(96);

        assert!(
            !p_try_move(&slab, handle, new_x, new_y, &level),
            "direct move should be blocked by one-sided wall"
        );

        let (slide_x, slide_y) = p_slide_move(&slab, handle, old_x, old_y, new_x, new_y, &level);
        assert_eq!(
            slide_x, old_x,
            "slide should preserve x against vertical wall"
        );
        assert_eq!(slide_y, new_y, "slide should keep forward y progress");
        assert!(p_try_move(&slab, handle, slide_x, slide_y, &level));
    }

    #[test]
    fn slide_move_returns_origin_when_corner_traps_actor() {
        let level = make_level_with_one_sided_walls(&[(64, 0, 64, 128), (0, 64, 128, 64)]);
        let (slab, handle) = make_player_slab();

        let old_x = Fixed16_16::from_int(32);
        let old_y = Fixed16_16::from_int(32);
        let new_x = Fixed16_16::from_int(70);
        let new_y = Fixed16_16::from_int(70);

        assert!(!p_try_move(&slab, handle, new_x, new_y, &level));
        let (slide_x, slide_y) = p_slide_move(&slab, handle, old_x, old_y, new_x, new_y, &level);
        assert_eq!((slide_x, slide_y), (old_x, old_y));
    }

    #[test]
    fn slide_move_with_stale_handle_returns_origin() {
        let level = make_open_level();
        let (mut slab, handle) = make_player_slab();
        slab.free(handle);

        let old_x = Fixed16_16::from_int(10);
        let old_y = Fixed16_16::from_int(20);
        let (slide_x, slide_y) = p_slide_move(
            &slab,
            handle,
            old_x,
            old_y,
            Fixed16_16::from_int(40),
            Fixed16_16::from_int(60),
            &level,
        );
        assert_eq!((slide_x, slide_y), (old_x, old_y));
    }

    #[test]
    fn step_height_uses_current_sector_floor_when_mobj_z_is_stale() {
        let level = make_two_sided_step_level(16, 32);
        let (mut slab, handle) = make_player_slab();
        let mo = slab.get_mut(handle).expect("value must exist in test");
        mo.x = Fixed16_16::from_int(48);
        mo.y = Fixed16_16::from_int(64);
        mo.z = Fixed16_16::ZERO;

        assert!(
            p_try_move(
                &slab,
                handle,
                Fixed16_16::from_int(80),
                Fixed16_16::from_int(64),
                &level
            ),
            "current sector floor (16) should allow stepping up to 32 even if mo.z is stale at 0"
        );
    }

    #[test]
    fn monsters_without_dropoff_flag_cannot_walk_off_high_ledges() {
        let level = make_partition_step_level(64, 0);
        let (slab, handle) = make_monster_slab(96, 0, 64);

        assert!(
            !p_try_move(
                &slab,
                handle,
                Fixed16_16::from_int(72),
                Fixed16_16::ZERO,
                &level
            ),
            "ground monsters should refuse drop-offs higher than 24 units"
        );
    }

    #[test]
    fn player_with_dropoff_flag_can_step_to_ledge_edge() {
        let level = make_partition_step_level(64, 0);
        let (mut slab, handle) = make_player_slab();
        let mo = slab.get_mut(handle).expect("value must exist in test");
        mo.flags |= flags::MF_DROPOFF;
        mo.x = Fixed16_16::from_int(96);
        mo.y = Fixed16_16::ZERO;
        mo.z = Fixed16_16::from_int(64);

        assert!(
            p_try_move(
                &slab,
                handle,
                Fixed16_16::from_int(72),
                Fixed16_16::ZERO,
                &level
            ),
            "players keep MF_DROPOFF and should not inherit the monster ledge restriction"
        );
    }
}
