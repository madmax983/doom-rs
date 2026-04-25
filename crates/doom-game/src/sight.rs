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
// Parametric intersection fraction
// ---------------------------------------------------------------------------

/// Compute the parametric `t` (fraction along the ray) where the ray from
/// `(rx1, ry1)` to `(rx2, ry2)` crosses the linedef.
///
/// Returns a value in `[0.0, 1.0]` (as a pair `(numerator, denominator)` in i64)
/// if the ray intersects the linedef, or `None` if the lines are parallel.
fn ray_linedef_frac(
    rx1: i64,
    ry1: i64,
    rx2: i64,
    ry2: i64,
    lx1: i64,
    ly1: i64,
    lx2: i64,
    ly2: i64,
) -> Option<(i64, i64)> {
    let rdx = rx2 - rx1;
    let rdy = ry2 - ry1;
    let ldx = lx2 - lx1;
    let ldy = ly2 - ly1;

    let denom = rdx * ldy - rdy * ldx;
    if denom == 0 {
        return None; // parallel
    }

    let num = ldx * (ry1 - ly1) - ldy * (rx1 - lx1);

    // We want 0 <= num/denom <= 1 (accounting for sign of denom).
    // Instead of dividing, check sign conditions.
    Some((num, denom))
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

    // Step 1: Reject table quick-reject.
    if let (Some(ss), Some(ts)) = (src_sector, tgt_sector) {
        if !level.reject.visible(ss, ts) {
            return false;
        }

        // Step 2: Trivial acceptance -- same sector, skip line traversal.
        if ss == ts {
            return true;
        }
    }

    // Compute sight Z heights (eye height = z + 3/4 * height).
    let src_eye_z = sight_eye_z(src_z, src_height);
    let tgt_eye_z = sight_eye_z(tgt_z, tgt_height);

    // Step 3: Line traversal -- check every linedef the ray crosses.
    let rx1 = src_x.to_int() as i64;
    let ry1 = src_y.to_int() as i64;
    let rx2 = tgt_x.to_int() as i64;
    let ry2 = tgt_y.to_int() as i64;

    for (ld_idx, ld) in level.linedefs.iter().enumerate() {
        let v1 = &level.vertexes[ld.from_vertex as usize];
        let v2 = &level.vertexes[ld.to_vertex as usize];

        let lx1 = v1.x as i64;
        let ly1 = v1.y as i64;
        let lx2 = v2.x as i64;
        let ly2 = v2.y as i64;

        // Quick check: does the ray cross this linedef at all?
        if !ray_crosses_linedef(src_x, src_y, tgt_x, tgt_y, ld_idx, level) {
            continue;
        }

        // One-sided line: always blocks LOS.
        if !ld.is_two_sided() {
            return false;
        }

        // Two-sided line: check the opening.
        let Some(sd) = level.sidedefs.get(ld.right_sidedef as usize) else {
            return false;
        };
        let right_sd = sd;
        let Some(sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
            return false;
        };
        let left_sd = sd;

        let front_sector = &level.sectors[right_sd.sector as usize];
        let back_sector = &level.sectors[left_sd.sector as usize];

        let open_floor = front_sector.floor_height.max(back_sector.floor_height) as i64;
        let open_ceil = front_sector.ceil_height.min(back_sector.ceil_height) as i64;

        // If the opening is closed (floor >= ceiling), LOS is blocked.
        if open_ceil <= open_floor {
            return false;
        }

        // Check if the sight line's Z at this crossing passes through the opening.
        // Compute the parametric fraction `t` along the ray where it crosses.
        let frac = ray_linedef_frac(rx1, ry1, rx2, ry2, lx1, ly1, lx2, ly2);

        if let Some((num, denom)) = frac {
            // Compute sight Z at the crossing point.
            // sight_z = src_eye_z + t * (tgt_eye_z - src_eye_z)
            // where t = num / denom.
            // To avoid floating point: sight_z_frac = src_eye_z * denom + num * dz
            let src_z_raw = src_eye_z.to_int() as i64;
            let tgt_z_raw = tgt_eye_z.to_int() as i64;
            let dz = tgt_z_raw - src_z_raw;

            // sight_z = src_z_raw + (num * dz) / denom
            // We need to handle the sign of denom carefully.
            let sight_z = if denom != 0 {
                // Normalize: ensure denom is positive for comparison
                let (n, d) = if denom < 0 {
                    (-num, -denom)
                } else {
                    (num, denom)
                };
                // Clamp t to [0, 1] -- but we know the ray crosses, so t should be valid.
                // sight_z = src_z_raw + n * dz / d
                src_z_raw + (n * dz) / d
            } else {
                src_z_raw // degenerate -- parallel, shouldn't happen since we crossed
            };

            // Check: does the sight line Z pass through the opening?
            if sight_z <= open_floor || sight_z >= open_ceil {
                return false;
            }
        }
    }

    // Ray reached target without being blocked.
    true
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
    use doom_map::{Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Vertex};
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
        Blockmap::parse_lump(&bm_data).expect("Expected successful result in test")
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
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("Expected successful result in test"),
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
        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("Expected successful result in test");

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
        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("Expected successful result in test");

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
        let reject =
            Reject::parse_lump(&[0xFFu8; 1], 2).expect("Expected successful result in test");
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
            .expect("Expected successful result in test")
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
            .expect("Expected successful result in test")
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
            .expect("Expected successful result in test")
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
            .expect("Expected successful result in test")
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
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("Expected successful result in test"),
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
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("Expected successful result in test"),
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
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("Expected successful result in test"),
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
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("Expected successful result in test"),
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
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("Expected successful result in test"),
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
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("Expected successful result in test"),
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
            .expect("Expected successful result in test")
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
            .expect("Expected successful result in test")
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
            .expect("Expected successful result in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 0, 0, 0, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("Expected successful result in test")
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
            .expect("Expected successful result in test")
            .subsector = 0;
        gs.mobjslab
            .get_mut(player_h)
            .expect("Expected successful result in test")
            .z = Fixed16_16::ZERO;

        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("Expected successful result in test")
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
            .expect("Expected successful result in test")
            .subsector = 0;

        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("Expected successful result in test")
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
            .expect("Expected successful result in test")
            .z = Fixed16_16::ZERO;
        gs.mobjslab
            .get_mut(monster)
            .expect("Expected successful result in test")
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
            .expect("Expected successful result in test")
            .z = Fixed16_16::ZERO;

        let monster = spawn_actor(&mut gs, 100, 0, 0, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("Expected successful result in test")
            .z = Fixed16_16::from_int(100);
        gs.mobjslab
            .get_mut(monster)
            .expect("Expected successful result in test")
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
        let reject = Reject::parse_lump(&[0u8; 2], 3).expect("Expected successful result in test");

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
            .expect("Expected successful result in test")
            .subsector = 0;
        let monster = spawn_actor(&mut gs, 160, 0, 2, 20);

        assert!(
            p_check_sight(&gs, &level, player_h, monster),
            "sight across multiple sectors with full openings must succeed"
        );
    }

    #[test]
    fn sight_reject_immediate_false() {
        // Level with 2 sectors, reject table says sector 0 can't see sector 1.
        let reject_data = vec![0xFFu8; 1]; // all bits set = nothing visible.
        let reject =
            Reject::parse_lump(&reject_data, 2).expect("Expected successful result in test");

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
            .expect("Expected successful result in test")
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
            reject: Reject::parse_lump(&[0u8; 1], 1).expect("Expected successful result in test"),
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
        // Two sectors: front floor=0 ceil=50, back floor=0 ceil=50.
        // Actors at z=0, height=56 -> eye = 42.
        // Opening: floor=0, ceil=50. Eye=42 is within [0, 50] -> should pass.
        // Now raise z so eye exceeds ceiling:
        // z=20, eye = 20 + 42 = 62. 62 >= 50 -> blocked.
        let level = make_portal_level(0, 50, 0, 50);
        let (mut gs, player_h) = make_gs_with_player(32, 0);
        gs.mobjslab
            .get_mut(player_h)
            .expect("Expected successful result in test")
            .subsector = 0;
        gs.mobjslab
            .get_mut(player_h)
            .expect("Expected successful result in test")
            .z = Fixed16_16::from_int(20);

        let monster = spawn_actor(&mut gs, 96, 0, 1, 20);
        gs.mobjslab
            .get_mut(monster)
            .expect("Expected successful result in test")
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
