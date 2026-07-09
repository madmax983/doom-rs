//! Line-of-sight checking (P_CheckSight) and monster player-seeking (P_LookForPlayers).
//!
//! Port of Doom's `p_sight.c` and portions of `p_enemy.c`.
//!
//! # Algorithm
//! 1. Reject table quick-reject: if both actors have known sector indices,
//!    check `level.reject.visible(sector_a, sector_b)`.  If the reject table
//!    marks them as not visible, return `false` immediately.
//! 2. Trivial acceptance: if source and target are in the same sector, return `true`.
//! 3. Line traversal: cast a 2D ray from source to target.  For each linedef
//!    the ray crosses:
//!    - One-sided lines always block LOS.
//!    - Two-sided lines: compute the floor/ceiling opening.  If the opening is
//!      zero or the sight line's Z doesn't pass through, LOS is blocked.
//! 4. If the ray reaches the target without being blocked, return `true`.

use doom_map::Level;
use doom_types::{ANG90, ANG270, Bam, Fixed16_16};

use crate::mobj::MobjHandle;
use crate::state::GameState;

// ---------------------------------------------------------------------------
// Sector lookup from subsector
// ---------------------------------------------------------------------------

/// Resolve the sector index for a given subsector index.
///
/// Path: `ssectors[sub]` -> first seg -> linedef -> sidedef -> sector.
/// Returns `None` if any index is out of range.
pub fn sector_from_subsector(level: &Level, subsector: usize) -> Option<usize> {
    let ss = level.ssectors.get(subsector)?;
    let seg = level.segs.get(ss.first_seg as usize)?;
    let ld = level.linedefs.get(seg.linedef as usize)?;
    let sd_idx = if seg.direction == 0 {
        ld.right_sidedef
    } else {
        ld.left_sidedef
    };
    let sd = level.sidedefs.get(sd_idx as usize)?;
    Some(sd.sector as usize)
}

/// Resolve an actor's sector from its current position, falling back to the
/// stored subsector when test geometry or synthetic levels cannot answer the
/// BSP query.
pub fn sector_from_position_or_subsector(
    level: &Level,
    x: Fixed16_16,
    y: Fixed16_16,
    subsector: usize,
) -> Option<usize> {
    level
        .sector_index_at(x.to_int(), y.to_int())
        .or_else(|| sector_from_subsector(level, subsector))
}

// ---------------------------------------------------------------------------
// point_on_side
// ---------------------------------------------------------------------------

/// Determine which side of a linedef a point is on.
///
/// Returns `0` for the front (right) side, `1` for the back (left) side.
/// Points exactly on the line return `0` (front side).
///
/// Uses the cross-product sign of `(linedef_direction) x (point - linedef_start)`.
/// All arithmetic uses `i64` to prevent overflow.
pub fn point_on_side(x: Fixed16_16, y: Fixed16_16, linedef_idx: usize, level: &Level) -> i32 {
    let ld = &level.linedefs[linedef_idx];
    let v1 = &level.vertexes[ld.from_vertex as usize];
    let v2 = &level.vertexes[ld.to_vertex as usize];

    let dx = v2.x as i64 - v1.x as i64;
    let dy = v2.y as i64 - v1.y as i64;
    let px = x.to_int() as i64 - v1.x as i64;
    let py = y.to_int() as i64 - v1.y as i64;

    // Cross product: dx * py - dy * px
    // Positive = left side (back), negative or zero = right side (front).
    let cross = dx * py - dy * px;
    if cross > 0 { 1 } else { 0 }
}

// ---------------------------------------------------------------------------
// ray_crosses_linedef
// ---------------------------------------------------------------------------

/// Check if a 2D ray from `(x1, y1)` to `(x2, y2)` crosses the given linedef.
///
/// Uses the "opposite sides" test: the ray crosses the linedef if and only if
/// the ray endpoints are on opposite sides of the linedef AND the linedef
/// endpoints are on opposite sides of the ray.
///
/// All arithmetic is `i64` to avoid overflow.
pub fn ray_crosses_linedef(
    x1: Fixed16_16,
    y1: Fixed16_16,
    x2: Fixed16_16,
    y2: Fixed16_16,
    linedef_idx: usize,
    level: &Level,
) -> bool {
    let ld = &level.linedefs[linedef_idx];
    let v1 = &level.vertexes[ld.from_vertex as usize];
    let v2 = &level.vertexes[ld.to_vertex as usize];

    let lx1 = v1.x as i64;
    let ly1 = v1.y as i64;
    let lx2 = v2.x as i64;
    let ly2 = v2.y as i64;

    let rx1 = x1.to_int() as i64;
    let ry1 = y1.to_int() as i64;
    let rx2 = x2.to_int() as i64;
    let ry2 = y2.to_int() as i64;

    // Test 1: Are the ray endpoints on opposite sides of the linedef?
    let ldx = lx2 - lx1;
    let ldy = ly2 - ly1;
    let cross_a = ldx * (ry1 - ly1) - ldy * (rx1 - lx1);
    let cross_b = ldx * (ry2 - ly1) - ldy * (rx2 - lx1);

    // If both are on the same side (same sign and both non-zero), no crossing.
    if (cross_a > 0 && cross_b > 0) || (cross_a < 0 && cross_b < 0) {
        return false;
    }

    // Test 2: Are the linedef endpoints on opposite sides of the ray?
    let rdx = rx2 - rx1;
    let rdy = ry2 - ry1;
    let cross_c = rdx * (ly1 - ry1) - rdy * (lx1 - rx1);
    let cross_d = rdx * (ly2 - ry1) - rdy * (lx2 - rx1);

    if (cross_c > 0 && cross_d > 0) || (cross_c < 0 && cross_d < 0) {
        return false;
    }

    // Handle degenerate case: if ray has zero length, no crossing.
    if rdx == 0 && rdy == 0 {
        return false;
    }

    // Handle degenerate case: if linedef has zero length, no crossing.
    if ldx == 0 && ldy == 0 {
        return false;
    }

    true
}

// ---------------------------------------------------------------------------
// Sight Z-height computation
// ---------------------------------------------------------------------------

/// Compute the eye height of an actor for sight checks.
///
/// In vanilla Doom, actors look from 75% of their height:
/// `z + height - (height >> 2)` which equals `z + 3/4 * height`.
#[inline]
fn sight_eye_z(z: Fixed16_16, height: Fixed16_16) -> Fixed16_16 {
    z + height - Fixed16_16(height.0 >> 2)
}

// ---------------------------------------------------------------------------
// Vanilla BSP line-of-sight traversal (p_sight.c)
// ---------------------------------------------------------------------------
//
// This is a faithful port of Doom's `P_CrossBSPNode` / `P_CrossSubsector` /
// `P_DivlineSide` / `P_InterceptVector2`.  Vanilla does NOT iterate the
// linedef array; it descends the BSP from the head node, and inside each
// crossed subsector clips the sight against the two-sided lines' openings by
// *accumulating* `topslope`/`bottomslope` in fixed-point.  All coordinates
// here are `fixed_t` (16.16), matching vanilla: map vertexes and node/sector
// values are shifted `<< FRACBITS`, mobj positions/heights are already fixed.

/// Bit in a BSP child pointer that marks the child as a subsector leaf.
const NF_SUBSECTOR: u16 = 0x8000;

/// A directed line segment in fixed-point, mirroring vanilla's `divline_t`.
#[derive(Clone, Copy)]
struct Divline {
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
}

/// Port of vanilla `P_DivlineSide`: returns 0 (front), 1 (back), or 2 (on).
///
/// Uses the exact `>> FRACBITS` shifts and wrapping `int` multiply of the
/// original so the true/false verdict matches vanilla bit-for-bit.
fn divline_side(x: i32, y: i32, node: &Divline) -> i32 {
    if node.dx == 0 {
        if x == node.x {
            return 2;
        }
        if x <= node.x {
            return i32::from(node.dy > 0);
        }
        return i32::from(node.dy < 0);
    }

    if node.dy == 0 {
        // NOTE: vanilla compares `x` to `node->y` here (not `node->x`); this
        // quirk is preserved deliberately for demo-exact behavior.
        if x == node.y {
            return 2;
        }
        if y <= node.y {
            return i32::from(node.dx < 0);
        }
        return i32::from(node.dx > 0);
    }

    let dx = x.wrapping_sub(node.x);
    let dy = y.wrapping_sub(node.y);

    let left = (node.dy >> 16).wrapping_mul(dx >> 16);
    let right = (dy >> 16).wrapping_mul(node.dx >> 16);

    if right < left {
        0 // front side
    } else if left == right {
        2
    } else {
        1 // back side
    }
}

/// Port of vanilla `P_InterceptVector2(v2 = strace, v1 = divl)`: the fractional
/// intercept of `v2` along `v1`, in fixed-point.
fn intercept_vector2(v2: &Divline, v1: &Divline) -> i32 {
    let fmul = |a: i32, b: i32| Fixed16_16(a).fixed_mul(Fixed16_16(b)).0;
    let fdiv = |a: i32, b: i32| Fixed16_16(a).fixed_div(Fixed16_16(b)).0;

    let den = fmul(v1.dy >> 8, v2.dx).wrapping_sub(fmul(v1.dx >> 8, v2.dy));
    if den == 0 {
        return 0;
    }
    let num = fmul((v1.x.wrapping_sub(v2.x)) >> 8, v1.dy)
        .wrapping_add(fmul((v2.y.wrapping_sub(v1.y)) >> 8, v1.dx));
    fdiv(num, den)
}

/// Mutable state carried through the recursive BSP sight traversal, mirroring
/// the module-level statics vanilla uses in `p_sight.c`.
struct SightState<'a> {
    level: &'a Level,
    strace: Divline,
    t2x: i32,
    t2y: i32,
    sightzstart: i32,
    topslope: i32,
    bottomslope: i32,
    /// Per-linedef "already checked the other side?" flags (vanilla validcount).
    line_seen: Vec<bool>,
}

impl SightState<'_> {
    #[inline]
    fn vertex_fixed(&self, v: u16) -> (i32, i32) {
        let vx = &self.level.vertexes[v as usize];
        ((vx.x as i32) << 16, (vx.y as i32) << 16)
    }

    /// Port of `P_CrossSubsector`: returns `true` if `strace` crosses the
    /// subsector without being blocked.
    fn cross_subsector(&mut self, num: usize) -> bool {
        let Some(sub) = self.level.ssectors.get(num) else {
            return true;
        };
        let first = sub.first_seg as usize;
        let end = first + sub.seg_count as usize;

        for seg_idx in first..end {
            let Some(seg) = self.level.segs.get(seg_idx) else {
                continue;
            };
            let line_idx = seg.linedef as usize;
            let Some(line) = self.level.linedefs.get(line_idx) else {
                continue;
            };

            // Already checked the other side of this line?
            if self.line_seen.get(line_idx).copied().unwrap_or(true) {
                continue;
            }
            if let Some(flag) = self.line_seen.get_mut(line_idx) {
                *flag = true;
            }

            // Line's own vertices (the linedef, not the seg).
            let (v1x, v1y) = self.vertex_fixed(line.from_vertex);
            let (v2x, v2y) = self.vertex_fixed(line.to_vertex);

            let s1 = divline_side(v1x, v1y, &self.strace);
            let s2 = divline_side(v2x, v2y, &self.strace);
            if s1 == s2 {
                continue; // line isn't crossed
            }

            let divl = Divline {
                x: v1x,
                y: v1y,
                dx: v2x.wrapping_sub(v1x),
                dy: v2y.wrapping_sub(v1y),
            };
            let s1 = divline_side(self.strace.x, self.strace.y, &divl);
            let s2 = divline_side(self.t2x, self.t2y, &divl);
            if s1 == s2 {
                continue; // line isn't crossed
            }

            // Determine front/back sectors from the seg's side (direction).
            let side = seg.direction;
            let (front_sd, back_sd) = if side == 0 {
                (line.right_sidedef, line.left_sidedef)
            } else {
                (line.left_sidedef, line.right_sidedef)
            };

            // Backsector NULL (one-sided / glass hack) or not two-sided: blocks.
            if !line.is_two_sided() || back_sd == doom_map::SIDEDEF_NONE {
                return false;
            }
            let (Some(front_side), Some(back_side)) = (
                self.level.sidedefs.get(front_sd as usize),
                self.level.sidedefs.get(back_sd as usize),
            ) else {
                return false;
            };
            let front = &self.level.sectors[front_side.sector as usize];
            let back = &self.level.sectors[back_side.sector as usize];

            let front_floor = (front.floor_height as i32) << 16;
            let front_ceil = (front.ceil_height as i32) << 16;
            let back_floor = (back.floor_height as i32) << 16;
            let back_ceil = (back.ceil_height as i32) << 16;

            // No wall to block sight with (identical opening)?
            if front_floor == back_floor && front_ceil == back_ceil {
                continue;
            }

            let opentop = front_ceil.min(back_ceil);
            let openbottom = front_floor.max(back_floor);

            // Quick test for totally closed doors.
            if openbottom >= opentop {
                return false;
            }

            let frac = intercept_vector2(&self.strace, &divl);

            if front_floor != back_floor {
                let slope = Fixed16_16(openbottom.wrapping_sub(self.sightzstart))
                    .fixed_div(Fixed16_16(frac))
                    .0;
                if slope > self.bottomslope {
                    self.bottomslope = slope;
                }
            }

            if front_ceil != back_ceil {
                let slope = Fixed16_16(opentop.wrapping_sub(self.sightzstart))
                    .fixed_div(Fixed16_16(frac))
                    .0;
                if slope < self.topslope {
                    self.topslope = slope;
                }
            }

            if self.topslope <= self.bottomslope {
                return false;
            }
        }

        // Passed the subsector ok.
        true
    }

    /// Port of `P_CrossBSPNode`: returns `true` if `strace` crosses the node.
    ///
    /// `bspnum` is an `i32` so the vanilla `bspnum == -1` head-node case (a map
    /// with zero BSP nodes) is preserved.
    fn cross_bsp_node(&mut self, bspnum: i32) -> bool {
        if bspnum & i32::from(NF_SUBSECTOR) != 0 {
            if bspnum == -1 {
                return self.cross_subsector(0);
            }
            return self.cross_subsector((bspnum & !i32::from(NF_SUBSECTOR)) as usize);
        }

        let Some(bsp) = self.level.nodes.get(bspnum as usize) else {
            return true;
        };
        let node = Divline {
            x: (bsp.x as i32) << 16,
            y: (bsp.y as i32) << 16,
            dx: (bsp.dx as i32) << 16,
            dy: (bsp.dy as i32) << 16,
        };
        let right_child = bsp.right_child as i32;
        let left_child = bsp.left_child as i32;

        // Decide which side the start point is on.
        let mut side = divline_side(self.strace.x, self.strace.y, &node);
        if side == 2 {
            side = 0; // an "on" should cross both sides
        }

        // Cross the starting side.
        let start_child = if side == 0 { right_child } else { left_child };
        if !self.cross_bsp_node(start_child) {
            return false;
        }

        // If the partition plane isn't actually crossed, we're done.
        if side == divline_side(self.t2x, self.t2y, &node) {
            return true;
        }

        // Cross the ending side.
        let end_child = if side == 0 { left_child } else { right_child };
        self.cross_bsp_node(end_child)
    }
}

// ---------------------------------------------------------------------------
// p_check_sight
// ---------------------------------------------------------------------------

/// Full line-of-sight check between two actors.
///
/// Steps:
/// 1. Reject table quick-reject based on sector pairs.
/// 2. Trivial accept if both actors are in the same sector.
/// 3. Cast a 2D ray from source to target, checking each linedef.
///    - One-sided lines block LOS.
///    - Two-sided lines: compute the opening and check if the sight line
///      (with Z heights) passes through.
///
/// Returns `true` if source can see target.
pub fn p_check_sight(
    gs: &GameState,
    level: &Level,
    source: MobjHandle,
    target: MobjHandle,
) -> bool {
    // Extract source data.
    let Some(mo) = gs.mobjslab.get(source) else {
        return false;
    };
    let src_x = mo.x;
    let src_y = mo.y;
    let src_z = mo.z;
    let src_height = mo.height;
    let src_subsector = mo.subsector as usize;

    // Extract target data.
    let Some(mo) = gs.mobjslab.get(target) else {
        return false;
    };
    let tgt_x = mo.x;
    let tgt_y = mo.y;
    let tgt_z = mo.z;
    let tgt_height = mo.height;
    let tgt_subsector = mo.subsector as usize;

    // Resolve sector indices from current world positions, falling back to the
    // actor's tracked subsector in synthetic/unit-test geometry.
    let src_sector = sector_from_position_or_subsector(level, src_x, src_y, src_subsector);
    let tgt_sector = sector_from_position_or_subsector(level, tgt_x, tgt_y, tgt_subsector);

    // Step 1: REJECT table trivial rejection (vanilla `P_CheckSight`).
    // Vanilla does NOT trivially accept same-sector pairs; it always walks the
    // BSP after the reject check.
    if let (Some(ss), Some(ts)) = (src_sector, tgt_sector) {
        if !level.reject.visible(ss, ts) {
            return false;
        }
    }

    // Vanilla sight geometry (all fixed-point):
    //   sightzstart = t1->z + t1->height - (t1->height >> 2)
    //   topslope    = (t2->z + t2->height) - sightzstart
    //   bottomslope = t2->z - sightzstart
    let sightzstart = sight_eye_z(src_z, src_height).0;
    let topslope = (tgt_z.0.wrapping_add(tgt_height.0)).wrapping_sub(sightzstart);
    let bottomslope = tgt_z.0.wrapping_sub(sightzstart);

    let mut state = SightState {
        level,
        strace: Divline {
            x: src_x.0,
            y: src_y.0,
            dx: tgt_x.0.wrapping_sub(src_x.0),
            dy: tgt_y.0.wrapping_sub(src_y.0),
        },
        t2x: tgt_x.0,
        t2y: tgt_y.0,
        sightzstart,
        topslope,
        bottomslope,
        line_seen: vec![false; level.linedefs.len()],
    };

    // The head node is the last node output; a nodeless map yields bspnum -1.
    let head = level.nodes.len() as i32 - 1;
    state.cross_bsp_node(head)
}

// ---------------------------------------------------------------------------
// p_aim_line_slope
// ---------------------------------------------------------------------------

/// Compute the vertical aim slope from source to target.
///
/// The slope is the Z difference between target eye and source eye,
/// divided by the horizontal distance.  Returned as a fixed-point value
/// (positive = aiming up, negative = aiming down).
///
/// Returns `0` if the horizontal distance is zero (actors at same position).
pub fn p_aim_line_slope(gs: &GameState, source: MobjHandle, target: MobjHandle) -> Fixed16_16 {
    let Some(mo) = gs.mobjslab.get(source) else {
        return Fixed16_16::ZERO;
    };
    let src_x = mo.x;
    let src_y = mo.y;
    let src_z = mo.z;
    let src_h = mo.height;
    let Some(mo) = gs.mobjslab.get(target) else {
        return Fixed16_16::ZERO;
    };
    let tgt_x = mo.x;
    let tgt_y = mo.y;
    let tgt_z = mo.z;
    let tgt_h = mo.height;

    let src_eye = sight_eye_z(src_z, src_h);
    let tgt_eye = sight_eye_z(tgt_z, tgt_h);
    let dz = tgt_eye - src_eye;

    // Horizontal distance (using fixed-point Manhattan-ish approach).
    let dx = (tgt_x - src_x).abs();
    let dy = (tgt_y - src_y).abs();
    let dist = if dx > dy {
        dx + Fixed16_16(dy.0 >> 1)
    } else {
        dy + Fixed16_16(dx.0 >> 1)
    };

    if dist.0 == 0 {
        return Fixed16_16::ZERO;
    }

    dz.fixed_div(dist)
}

// ---------------------------------------------------------------------------
// p_look_for_players
// ---------------------------------------------------------------------------

/// Monster AI: scan for visible players and return a handle if found.
///
/// Currently only checks player 0 (single-player).  In vanilla Doom,
/// monsters check sight every 4 tics for performance; the caller is
/// responsible for throttling.
///
/// Returns `Some(player_handle)` if the player is alive and visible to
/// the actor, `None` otherwise.
pub fn p_look_for_players(gs: &GameState, level: &Level, actor: MobjHandle) -> Option<MobjHandle> {
    let player_handle = gs.player.handle;

    // Check the actor itself exists.
    let actor_mo = gs.mobjslab.get(actor)?;

    // Check the player exists and is alive.
    let player_mo = gs.mobjslab.get(player_handle)?;
    if player_mo.is_dead() {
        return None;
    }

    let dx = player_mo.x.to_int() - actor_mo.x.to_int();
    let dy = player_mo.y.to_int() - actor_mo.y.to_int();

    let angle_to_player = bam_from_delta(dx, dy);
    let angle_delta = angle_to_player - actor_mo.angle;
    let dist_sq = i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy);
    let melee_range = i64::from(crate::combat::MELEERANGE.to_int());

    // Doom's default look path does not acquire targets behind the monster
    // unless they are close enough for melee.
    if angle_delta > ANG90 && angle_delta < ANG270 && dist_sq > melee_range * melee_range {
        return None;
    }

    // Check line of sight.
    if p_check_sight(gs, level, actor, player_handle) {
        Some(player_handle)
    } else {
        None
    }
}

fn bam_from_delta(dx: i32, dy: i32) -> Bam {
    if dx == 0 && dy == 0 {
        return Bam::ZERO;
    }

    let angle_rad = (dy as f64).atan2(dx as f64);
    let turns = angle_rad / std::f64::consts::TAU;
    Bam((turns * (u32::MAX as f64 + 1.0)) as u32)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, flags};
    use crate::player::PlayerState;
    use crate::state::GameState;
    use doom_map::{
        Blockmap, Linedef, Node, NodeBBox, Reject, Sector, Seg, Sidedef, Ssector, Vertex,
    };
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    /// Build a minimal 1x1 blockmap at origin.
    fn make_minimal_blockmap() -> Blockmap {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        Blockmap::parse_lump(&bm_data).expect("value must exist in test")
    }

    /// Build a default sector (floor=0, ceil=128).
    fn make_sector(floor: i16, ceil: i16) -> Sector {
        Sector {
            floor_height: floor,
            ceil_height: ceil,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }
    }

    /// Make a sidedef that references the given sector.
    fn make_sidedef(sector: u16) -> Sidedef {
        Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"-\0\0\0\0\0\0\0",
            lower_texture: *b"-\0\0\0\0\0\0\0",
            middle_texture: *b"WALL1\0\0\0",
            sector,
        }
    }

    /// Make a one-sided linedef between two vertex indices.
    fn make_one_sided_linedef(from: u16, to: u16, right_sd: u16) -> Linedef {
        Linedef {
            from_vertex: from,
            to_vertex: to,
            flags: 0, // NOT two-sided
            special: 0,
            tag: 0,
            right_sidedef: right_sd,
            left_sidedef: 0xFFFF,
        }
    }

    /// Make a two-sided linedef between two vertex indices.
    fn make_two_sided_linedef(from: u16, to: u16, right_sd: u16, left_sd: u16) -> Linedef {
        Linedef {
            from_vertex: from,
            to_vertex: to,
            flags: 0x0004, // FLAG_TWO_SIDED
            special: 0,
            tag: 0,
            right_sidedef: right_sd,
            left_sidedef: left_sd,
        }
    }

    /// Build a level with one sector and no blocking linedefs (open space).
    ///
    /// Contains a single one-sided linedef forming the boundary (not blocking
    /// any internal sight lines since the ray won't cross it for in-sector checks).
    fn make_open_level() -> Level {
        Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_one_sided_linedef(0, 1, 0)],
            sidedefs: vec![make_sidedef(0)],
            vertexes: vec![Vertex { x: -1000, y: -1000 }, Vertex { x: -1000, y: 1000 }],
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
            sectors: vec![make_sector(0, 128)],
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("value must exist in test"),
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build a level with 2 sectors and a one-sided wall between them.
    ///
    /// Layout:
    ///   Sector 0: left side (x < 64)
    ///   Sector 1: right side (x > 64)
    ///   Wall at x=64, from (64, -128) to (64, 128)
    fn make_wall_level() -> Level {
        let sectors = vec![make_sector(0, 128), make_sector(0, 128)];
        let sidedefs = vec![
            make_sidedef(0), // 0: faces sector 0
            make_sidedef(1), // 1: faces sector 1
        ];
        let vertexes = vec![Vertex { x: 64, y: -128 }, Vertex { x: 64, y: 128 }];
        let linedefs = vec![
            make_one_sided_linedef(0, 1, 0), // solid wall at x=64
        ];
        // Segs for both subsectors referencing different sidedefs.
        // Subsector 0 -> seg 0 -> linedef 0 -> right sidedef (sector 0).
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
                from_vertex: 1,
                to_vertex: 0,
                angle: 0,
                linedef: 0,
                direction: 1, // reversed direction -> left sidedef
                offset: 0,
            },
        ];
        // Two subsectors: one for each sector.
        let ssectors = vec![
            Ssector {
                seg_count: 1,
                first_seg: 0,
            }, // subsector 0 -> sector 0
            Ssector {
                seg_count: 1,
                first_seg: 1,
            }, // subsector 1 -> sector 1
        ];

        // Reject: 2 sectors, all visible.
        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");

        Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build a level with 2 sectors separated by a two-sided linedef.
    ///
    /// The opening between sectors can be controlled by adjusting sector heights.
    fn make_portal_level(
        front_floor: i16,
        front_ceil: i16,
        back_floor: i16,
        back_ceil: i16,
    ) -> Level {
        let sectors = vec![
            make_sector(front_floor, front_ceil),
            make_sector(back_floor, back_ceil),
        ];
        let sidedefs = vec![
            make_sidedef(0), // 0: right (front, sector 0)
            make_sidedef(1), // 1: left (back, sector 1)
        ];
        let vertexes = vec![Vertex { x: 64, y: -128 }, Vertex { x: 64, y: 128 }];
        let linedefs = vec![make_two_sided_linedef(0, 1, 0, 1)];
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
                from_vertex: 1,
                to_vertex: 0,
                angle: 0,
                linedef: 0,
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
        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");

        Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build a game state with a player at the given position.
    fn make_gs_with_player(x: i32, y: i32) -> (GameState, MobjHandle) {
        let mut gs = GameState::new("TEST");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 100;
        mo.height = Fixed16_16::from_int(56);
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        mo.subsector = 0;
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        (gs, handle)
    }

    /// Spawn a monster (trooper) at the given position and subsector.
    fn spawn_actor(gs: &mut GameState, x: i32, y: i32, subsector: u32, health: i32) -> MobjHandle {
        let mut mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = health;
        mo.height = Fixed16_16::from_int(56);
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.subsector = subsector;
        gs.mobjslab.alloc(mo)
    }

    // =======================================================================
    // p_check_sight tests
    // =======================================================================

    #[test]
    fn sight_same_sector_returns_true() {
        let (gs, player_h) = make_gs_with_player(32, 32);
        let level = make_open_level();

        // Spawn a monster in the same subsector (subsector 0 -> sector 0).
        let mut gs = gs;
        let monster = spawn_actor(&mut gs, 64, 32, 0, 20);

        assert!(
            p_check_sight(&gs, &level, player_h, monster),
            "actors in the same sector must see each other"
        );
    }

    #[test]
    fn sight_reject_table_blocks() {
        // Build a level with 2 sectors, reject table says they can't see each other.
        let sectors = vec![make_sector(0, 128), make_sector(0, 128)];
        // All-ones reject = nothing visible.
        let reject = Reject::parse_lump(&[0xFFu8; 1], 2).expect("value must exist in test");
        let sidedefs = vec![make_sidedef(0), make_sidedef(1)];
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
                from_vertex: 1,
                to_vertex: 0,
                angle: 0,
                linedef: 0,
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
        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_two_sided_linedef(0, 1, 0, 1)],
            sidedefs,
            vertexes: vec![Vertex { x: 64, y: -128 }, Vertex { x: 64, y: 128 }],
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        };

        let (mut gs, player_h) = make_gs_with_player(32, 0);
        // Put player in subsector 0 (sector 0).
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        // Monster in subsector 1 (sector 1).
        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);

        assert!(
            !p_check_sight(&gs, &level, player_h, monster),
            "reject table says not visible -> must return false"
        );
    }

    #[test]
    fn sight_reject_visible_no_blocking_lines() {
        let level = make_open_level();
        let (mut gs, player_h) = make_gs_with_player(10, 10);
        let monster = spawn_actor(&mut gs, 100, 10, 0, 20);

        assert!(
            p_check_sight(&gs, &level, player_h, monster),
            "no blocking lines and reject allows -> must return true"
        );
    }

    #[test]
    fn sight_blocked_by_solid_wall() {
        let level = make_wall_level();
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        // Player in subsector 0 (sector 0), monster in subsector 1 (sector 1).
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);

        assert!(
            !p_check_sight(&gs, &level, player_h, monster),
            "one-sided wall between actors must block sight"
        );
    }

    #[test]
    fn sight_through_two_sided_with_opening() {
        // Two sectors with plenty of opening (floor=0, ceil=128 both sides).
        let level = make_portal_level(0, 128, 0, 128);
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);

        assert!(
            p_check_sight(&gs, &level, player_h, monster),
            "two-sided line with full opening must allow sight"
        );
    }

    #[test]
    fn sight_blocked_by_closed_two_sided_line() {
        // Two-sided line where floor meets ceiling (zero opening).
        let level = make_portal_level(0, 64, 64, 128);
        // Opening: floor=max(0,64)=64, ceil=min(64,128)=64 -> opening=0.
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);

        assert!(
            !p_check_sight(&gs, &level, player_h, monster),
            "two-sided line with zero opening must block sight"
        );
    }

    // =======================================================================
    // point_on_side tests
    // =======================================================================

    #[test]
    fn point_on_side_front() {
        // Linedef from (0, 0) to (100, 0) -- horizontal, facing north (front = right of line direction = north).
        // Point above the line (positive Y) is on the left (back) side.
        // Point below the line (negative Y) is on the right (front) side.
        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_one_sided_linedef(0, 1, 0)],
            sidedefs: vec![make_sidedef(0)],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 100, y: 0 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![make_sector(0, 128)],
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("value must exist in test"),
            blockmap: make_minimal_blockmap(),
        };

        // Point below the line: front side (0).
        assert_eq!(
            point_on_side(
                Fixed16_16::from_int(50),
                Fixed16_16::from_int(-10),
                0,
                &level
            ),
            0,
            "point below horizontal line should be on front side"
        );
    }

    #[test]
    fn point_on_side_back() {
        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_one_sided_linedef(0, 1, 0)],
            sidedefs: vec![make_sidedef(0)],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 100, y: 0 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![make_sector(0, 128)],
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("value must exist in test"),
            blockmap: make_minimal_blockmap(),
        };

        // Point above the line: back side (1).
        assert_eq!(
            point_on_side(
                Fixed16_16::from_int(50),
                Fixed16_16::from_int(10),
                0,
                &level
            ),
            1,
            "point above horizontal line should be on back side"
        );
    }

    #[test]
    fn point_on_side_exactly_on_line() {
        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_one_sided_linedef(0, 1, 0)],
            sidedefs: vec![make_sidedef(0)],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 100, y: 0 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![make_sector(0, 128)],
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("value must exist in test"),
            blockmap: make_minimal_blockmap(),
        };

        // Point on the line: returns 0 (front side).
        assert_eq!(
            point_on_side(Fixed16_16::from_int(50), Fixed16_16::ZERO, 0, &level),
            0,
            "point exactly on the line should be treated as front side"
        );
    }

    // =======================================================================
    // ray_crosses_linedef tests
    // =======================================================================

    #[test]
    fn ray_crosses_crossing_ray() {
        // Linedef: vertical wall at x=64, from (64, -128) to (64, 128).
        // Ray: from (32, 0) to (96, 0) -- horizontal, should cross.
        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_one_sided_linedef(0, 1, 0)],
            sidedefs: vec![make_sidedef(0)],
            vertexes: vec![Vertex { x: 64, y: -128 }, Vertex { x: 64, y: 128 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![make_sector(0, 128)],
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("value must exist in test"),
            blockmap: make_minimal_blockmap(),
        };

        assert!(ray_crosses_linedef(
            Fixed16_16::from_int(32),
            Fixed16_16::ZERO,
            Fixed16_16::from_int(96),
            Fixed16_16::ZERO,
            0,
            &level,
        ));
    }

    #[test]
    fn ray_misses_non_crossing() {
        // Linedef: vertical wall at x=64, from (64, -128) to (64, 128).
        // Ray: from (32, 0) to (50, 0) -- both on left of wall, should NOT cross.
        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_one_sided_linedef(0, 1, 0)],
            sidedefs: vec![make_sidedef(0)],
            vertexes: vec![Vertex { x: 64, y: -128 }, Vertex { x: 64, y: 128 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![make_sector(0, 128)],
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("value must exist in test"),
            blockmap: make_minimal_blockmap(),
        };

        assert!(!ray_crosses_linedef(
            Fixed16_16::from_int(32),
            Fixed16_16::ZERO,
            Fixed16_16::from_int(50),
            Fixed16_16::ZERO,
            0,
            &level,
        ));
    }

    #[test]
    fn ray_parallel_to_linedef() {
        // Linedef: vertical at x=64. Ray: vertical at x=32 -- parallel, no crossing.
        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_one_sided_linedef(0, 1, 0)],
            sidedefs: vec![make_sidedef(0)],
            vertexes: vec![Vertex { x: 64, y: -128 }, Vertex { x: 64, y: 128 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![make_sector(0, 128)],
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("value must exist in test"),
            blockmap: make_minimal_blockmap(),
        };

        assert!(!ray_crosses_linedef(
            Fixed16_16::from_int(32),
            Fixed16_16::from_int(-100),
            Fixed16_16::from_int(32),
            Fixed16_16::from_int(100),
            0,
            &level,
        ));
    }

    // =======================================================================
    // p_look_for_players tests
    // =======================================================================

    #[test]
    fn look_for_players_returns_none_when_dead() {
        let (mut gs, player_h) = make_gs_with_player(32, 32);
        let level = make_open_level();
        // Kill the player.
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .health = 0;

        let monster = spawn_actor(&mut gs, 64, 32, 0, 20);
        assert!(
            p_look_for_players(&gs, &level, monster).is_none(),
            "dead player must not be found"
        );
    }

    #[test]
    fn look_for_players_returns_some_when_alive_and_visible() {
        let (mut gs, _player_h) = make_gs_with_player(32, 32);
        let level = make_open_level();

        let monster = spawn_actor(&mut gs, 64, 32, 0, 20);
        let result = p_look_for_players(&gs, &level, monster);
        assert!(
            result.is_some(),
            "alive player with clear LOS must be found"
        );
    }

    #[test]
    fn look_for_players_returns_none_when_blocked() {
        let level = make_wall_level();
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);

        assert!(
            p_look_for_players(&gs, &level, monster).is_none(),
            "wall between player and monster must block sight"
        );
    }

    #[test]
    fn look_for_players_rejects_player_behind_back_outside_melee_range() {
        let level = make_open_level();
        let (mut gs, player_h) = make_gs_with_player(-200, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 0, 0, 0, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("value must exist in test")
            .angle = Bam::ZERO; // facing east

        assert!(
            p_look_for_players(&gs, &level, monster).is_none(),
            "player behind the monster and outside melee range must not be acquired"
        );
    }

    // =======================================================================
    // Z-height / sight line tests
    // =======================================================================

    #[test]
    fn sight_eye_z_computation() {
        let z = Fixed16_16::ZERO;
        let h = Fixed16_16::from_int(56);
        let eye = sight_eye_z(z, h);
        // Expected: 0 + 56 - 14 = 42
        assert_eq!(eye.to_int(), 42, "eye height should be z + 3/4 * height");
    }

    #[test]
    fn sight_check_different_heights() {
        // Two sectors: front has floor=0 ceil=128, back has floor=100 ceil=128.
        // Opening: floor=100, ceil=128, opening=28.
        // Actors at z=0 (eye=42) on left, z=100 (eye=142) on right.
        // The sight line from eye=42 to eye=142 must pass through the opening
        // at the linedef crossing. Since the portal is at the crossing point,
        // z at crossing ~ (42 + 142) / 2 = 92. Opening is [100, 128].
        // 92 < 100 -> should be blocked.
        let level = make_portal_level(0, 128, 100, 128);
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .z = Fixed16_16::ZERO;

        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("value must exist in test")
            .z = Fixed16_16::from_int(100);

        assert!(
            !p_check_sight(&gs, &level, player_h, monster),
            "sight line that passes below the opening floor should be blocked"
        );
    }

    #[test]
    fn sight_z_passes_through_opening() {
        // Both sectors: floor=0, ceil=128. Full opening.
        // Actors at z=0 on both sides. Eye height = 42.
        // Sight line at crossing = 42. Opening = [0, 128]. 42 is within.
        let level = make_portal_level(0, 128, 0, 128);
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;

        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("value must exist in test")
            .z = Fixed16_16::ZERO;

        assert!(
            p_check_sight(&gs, &level, player_h, monster),
            "sight line that passes through opening should not be blocked"
        );
    }

    // =======================================================================
    // p_aim_line_slope tests
    // =======================================================================

    #[test]
    fn aim_slope_level_ground() {
        let (mut gs, player_h) = make_gs_with_player(0, 0);
        // Both at z=0, same height -- slope should be 0.
        let monster = spawn_actor(&mut gs, 100, 0, 0, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("value must exist in test")
            .z = Fixed16_16::ZERO;
        gs.mobjslab
            .get_mut(monster)
            .expect("value must exist in test")
            .height = Fixed16_16::from_int(56);

        let slope = p_aim_line_slope(&gs, player_h, monster);
        assert_eq!(
            slope,
            Fixed16_16::ZERO,
            "same height actors -> slope should be 0"
        );
    }

    #[test]
    fn aim_slope_target_above() {
        let (mut gs, player_h) = make_gs_with_player(0, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .z = Fixed16_16::ZERO;

        let monster = spawn_actor(&mut gs, 100, 0, 0, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("value must exist in test")
            .z = Fixed16_16::from_int(100);
        gs.mobjslab
            .get_mut(monster)
            .expect("value must exist in test")
            .height = Fixed16_16::from_int(56);

        let slope = p_aim_line_slope(&gs, player_h, monster);
        assert!(
            slope.0 > 0,
            "target above source -> positive slope, got {}",
            slope.0
        );
    }

    // =======================================================================
    // Multi-sector sight checks
    // =======================================================================

    #[test]
    fn sight_across_multiple_sectors() {
        // Three sectors in a line: sector 0 | portal | sector 1 | portal | sector 2.
        // Two two-sided linedefs, both with full openings.
        let sectors = vec![
            make_sector(0, 128),
            make_sector(0, 128),
            make_sector(0, 128),
        ];
        let sidedefs = vec![
            make_sidedef(0), // 0
            make_sidedef(1), // 1
            make_sidedef(1), // 2
            make_sidedef(2), // 3
        ];
        let vertexes = vec![
            Vertex { x: 64, y: -128 },
            Vertex { x: 64, y: 128 },
            Vertex { x: 128, y: -128 },
            Vertex { x: 128, y: 128 },
        ];
        let linedefs = vec![
            make_two_sided_linedef(0, 1, 0, 1), // portal at x=64
            make_two_sided_linedef(2, 3, 2, 3), // portal at x=128
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
                from_vertex: 1,
                to_vertex: 0,
                angle: 0,
                linedef: 0,
                direction: 1,
                offset: 0,
            },
            Seg {
                from_vertex: 2,
                to_vertex: 3,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
        ];
        let ssectors = vec![
            Ssector {
                seg_count: 1,
                first_seg: 0,
            }, // subsector 0 -> sector 0
            Ssector {
                seg_count: 1,
                first_seg: 1,
            }, // subsector 1 -> sector 1
            Ssector {
                seg_count: 1,
                first_seg: 2,
            }, // subsector 2 -> sector 2
        ];
        // 3 sectors -> ceil(9/8) = 2 bytes, all visible.
        let reject = Reject::parse_lump(&[0u8; 2], 3).expect("value must exist in test");

        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        };

        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 160, 0, 2, 20);

        assert!(
            p_check_sight(&gs, &level, player_h, monster),
            "sight across multiple sectors with full openings must succeed"
        );
    }

    /// Build a two-sector level joined by a vertical two-sided portal at x=64,
    /// with a real 1-node BSP so `p_check_sight` exercises `cross_bsp_node`
    /// recursion (west subsector 0 = sector 0, east subsector 1 = sector 1).
    /// `east_floor` sets sector 1's floor so callers can raise a blocking step.
    fn make_bsp_portal_level(east_floor: i16) -> Level {
        let sectors = vec![make_sector(0, 128), make_sector(east_floor, 128)];
        // Right side of the (64,-128)->(64,128) line faces east (sector 1);
        // left side faces west (sector 0).
        let sidedefs = vec![make_sidedef(1), make_sidedef(0)];
        let vertexes = vec![Vertex { x: 64, y: -128 }, Vertex { x: 64, y: 128 }];
        let linedefs = vec![make_two_sided_linedef(0, 1, 0, 1)];
        let segs = vec![
            // subsector 0 (west, sector 0): seg faces west -> left sidedef.
            Seg {
                from_vertex: 1,
                to_vertex: 0,
                angle: 0,
                linedef: 0,
                direction: 1,
                offset: 0,
            },
            // subsector 1 (east, sector 1): seg faces east -> right sidedef.
            Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
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
        // Partition line x=64 pointing +y. P_DivlineSide sends x<64 to side 1
        // (left child) and x>64 to side 0 (right child).
        let bbox = NodeBBox {
            ymax: 128,
            ymin: -128,
            xmin: -128,
            xmax: 256,
        };
        let nodes = vec![Node {
            x: 64,
            y: -128,
            dx: 0,
            dy: 256,
            right_bbox: bbox,
            left_bbox: bbox,
            right_child: 0x8000 | 1, // side 0 (east) -> subsector 1
            left_child: 0x8000,      // side 1 (west) -> subsector 0
        }];
        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");

        Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes,
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    #[test]
    fn sight_bsp_portal_full_opening_visible() {
        // Regression: vanilla walks the BSP (cross_bsp_node -> cross_subsector).
        // Equal front/back heights make the portal a non-occluder, so a level
        // sight line passes.
        let level = make_bsp_portal_level(0);
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);

        assert!(
            p_check_sight(&gs, &level, player_h, monster),
            "clear portal with equal heights must allow sight through the BSP"
        );
    }

    #[test]
    fn sight_bsp_portal_high_step_blocks() {
        // Regression pinning vanilla's accumulate-slope floor clipping across
        // the BSP portal: the east sector floor is a 100-unit step. The player
        // on the low floor (eye z=42) cannot see the monster standing on the
        // step (feet z=100) because openbottom (100) clips bottomslope above
        // topslope -> P_CrossSubsector returns false.
        let level = make_bsp_portal_level(100);
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .z = Fixed16_16::ZERO;
        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("value must exist in test")
            .z = Fixed16_16::from_int(100);

        assert!(
            !p_check_sight(&gs, &level, player_h, monster),
            "a 100-unit floor step must block the low-eye sight line (vanilla slope clip)"
        );
    }

    #[test]
    fn sight_reject_immediate_false() {
        // Level with 2 sectors, reject table says sector 0 can't see sector 1.
        let reject_data = vec![0xFFu8; 1]; // all bits set = nothing visible.
        let reject = Reject::parse_lump(&reject_data, 2).expect("value must exist in test");

        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_two_sided_linedef(0, 1, 0, 1)],
            sidedefs: vec![make_sidedef(0), make_sidedef(1)],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 100, y: 0 }],
            segs: vec![
                Seg {
                    from_vertex: 0,
                    to_vertex: 1,
                    angle: 0,
                    linedef: 0,
                    direction: 0,
                    offset: 0,
                },
                Seg {
                    from_vertex: 1,
                    to_vertex: 0,
                    angle: 0,
                    linedef: 0,
                    direction: 1,
                    offset: 0,
                },
            ],
            ssectors: vec![
                Ssector {
                    seg_count: 1,
                    first_seg: 0,
                },
                Ssector {
                    seg_count: 1,
                    first_seg: 1,
                },
            ],
            nodes: vec![],
            sectors: vec![make_sector(0, 128), make_sector(0, 128)],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        let (mut gs, player_h) = make_gs_with_player(10, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 80, 0, 1, 20);

        assert!(
            !p_check_sight(&gs, &level, player_h, monster),
            "reject table all-ones must immediately return false"
        );
    }

    #[test]
    fn sight_trivial_same_position() {
        // Source and target at the exact same position.
        let level = make_open_level();
        let (mut gs, player_h) = make_gs_with_player(50, 50);
        let monster = spawn_actor(&mut gs, 50, 50, 0, 20);

        assert!(
            p_check_sight(&gs, &level, player_h, monster),
            "actors at the same position must see each other"
        );
    }

    #[test]
    fn sight_stale_source_returns_false() {
        let level = make_open_level();
        let (mut gs, player_h) = make_gs_with_player(32, 32);
        let _monster = spawn_actor(&mut gs, 64, 32, 0, 20);
        // Free the player -> stale handle.
        gs.mobjslab.free(player_h);

        assert!(
            !p_check_sight(&gs, &level, player_h, _monster),
            "stale source handle must return false"
        );
    }

    #[test]
    fn sight_stale_target_returns_false() {
        let level = make_open_level();
        let (mut gs, player_h) = make_gs_with_player(32, 32);
        let monster = spawn_actor(&mut gs, 64, 32, 0, 20);
        gs.mobjslab.free(monster);

        assert!(
            !p_check_sight(&gs, &level, player_h, monster),
            "stale target handle must return false"
        );
    }

    #[test]
    fn ray_does_not_cross_distant_linedef() {
        // Linedef: from (200, -10) to (200, 10).
        // Ray: from (0, 0) to (50, 0) -- completely to the left, should not cross.
        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![make_one_sided_linedef(0, 1, 0)],
            sidedefs: vec![make_sidedef(0)],
            vertexes: vec![Vertex { x: 200, y: -10 }, Vertex { x: 200, y: 10 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![make_sector(0, 128)],
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("value must exist in test"),
            blockmap: make_minimal_blockmap(),
        };

        assert!(!ray_crosses_linedef(
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Fixed16_16::from_int(50),
            Fixed16_16::ZERO,
            0,
            &level,
        ));
    }

    #[test]
    fn sight_z_above_ceiling_blocked() {
        // Vanilla only clips against a two-sided line when the front/back
        // heights DIFFER (otherwise it is not an occluder and is skipped).
        // Front ceil=128, back ceil=40 -> opentop=40. Both actors at z=20,
        // height=56 -> sightzstart=62, above the 40-unit opening top, so
        // topslope is clipped below bottomslope and sight is blocked.
        let level = make_portal_level(0, 128, 0, 40);
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .subsector = 0;
        gs.mobjslab
            .get_mut(player_h)
            .expect("value must exist in test")
            .z = Fixed16_16::from_int(20);

        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("value must exist in test")
            .z = Fixed16_16::from_int(20);

        assert!(
            !p_check_sight(&gs, &level, player_h, monster),
            "sight line above ceiling should be blocked"
        );
    }

    #[test]
    fn look_for_players_stale_actor() {
        let level = make_open_level();
        let (mut gs, _player_h) = make_gs_with_player(32, 32);
        let monster = spawn_actor(&mut gs, 64, 32, 0, 20);
        gs.mobjslab.free(monster);

        assert!(
            p_look_for_players(&gs, &level, monster).is_none(),
            "stale actor handle must return None"
        );
    }

    #[test]
    fn sector_from_subsector_valid() {
        let level = make_open_level();
        let sector = sector_from_subsector(&level, 0);
        assert_eq!(sector, Some(0));
    }

    #[test]
    fn sector_from_subsector_out_of_range() {
        let level = make_open_level();
        let sector = sector_from_subsector(&level, 999);
        assert_eq!(sector, None);
    }
}
