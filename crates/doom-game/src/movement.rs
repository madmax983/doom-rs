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

fn clamp_i128_to_i32(v: i128) -> i32 {
    v.clamp(i32::MIN as i128, i32::MAX as i128) as i32
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

    let current_floor = level
        .floor_at(old_x.to_int(), old_y.to_int())
        .map(|floor| Fixed16_16::from_int(floor as i32));
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

                // --- Two-sided: ML_BLOCKING always blocks, ML_BLOCKMONSTERS blocks monsters ---
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

                let open_floor =
                    Fixed16_16::from_int(front.floor_height.max(back.floor_height) as i32);
                let open_ceil =
                    Fixed16_16::from_int(front.ceil_height.min(back.ceil_height) as i32);

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
            }
        }
    }

    (true, None)
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
    let dx = (x2 - x1).to_int() as i64;
    let dy = (y2 - y1).to_int() as i64;

    // Choose the two "extreme" corners based on which quadrant the line goes.
    // Same-sign quadrant (NE or SW): use (left, top) and (right, bottom).
    // Opposite-sign quadrant (NW or SE): use (right, top) and (left, bottom).
    let (px1, py1, px2, py2) = if dx.signum() == dy.signum() {
        (left, top, right, bottom)
    } else {
        (right, top, left, bottom)
    };

    // Cross products: (x2-x1)*(py-y1) - (y2-y1)*(px-x1)
    let y1i = y1.to_int() as i64;
    let x1i = x1.to_int() as i64;
    let c1 = dx * (py1.to_int() as i64 - y1i) - dy * (px1.to_int() as i64 - x1i);
    let c2 = dx * (py2.to_int() as i64 - y1i) - dy * (px2.to_int() as i64 - x1i);

    // Different signs (one positive, one negative) means the box straddles.
    (c1 ^ c2) < 0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind, MobjSlab};
    use doom_types::Bam;

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
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).unwrap();

        let reject = doom_map::Reject::parse_lump(&[0u8], 1).unwrap();

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
                floor_height: 0,
                ceil_height: 128,
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
            floor_height: 0,
            ceil_height: 128,
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
        let blockmap = Blockmap::parse_lump(&bm_data).unwrap();
        let reject = Reject::parse_lump(&[0u8], 1).unwrap();

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
                floor_height: front_floor,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: back_floor,
                ceil_height: 128,
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
        let blockmap = Blockmap::parse_lump(&bm_data).unwrap();
        let reject = Reject::parse_lump(&[0u8], 2).unwrap();

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
        slab.get_mut(handle).unwrap().flags |= flags::MF_NOCLIP;
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
        let mo = slab.get_mut(handle).unwrap();
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
}
