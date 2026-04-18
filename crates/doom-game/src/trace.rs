//! Blockmap-accelerated ray traversal for hitscan attacks and LOS checks.
//!
//! Uses a DDA (Digital Differential Analyzer) algorithm to step through
//! blockmap cells along a ray, testing linedefs and actors in each cell.
//!
//! Port of Doom's `P_PathTraverse` / `P_AimLineAttack` ray-casting logic.

use doom_map::Level;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Blockmap cell size in map units (128x128).
const BLOCK_SIZE: f32 = 128.0;

/// Small epsilon to avoid floating-point edge cases.
const EPSILON: f32 = 1.0e-6;

// ---------------------------------------------------------------------------
// TraceResult / TraceHit
// ---------------------------------------------------------------------------

/// Result of a ray trace through the level geometry.
#[derive(Debug, Clone)]
pub struct TraceResult {
    /// Distance along the ray where the hit occurred (0.0 = origin, 1.0 = max range).
    pub frac: f32,
    /// What was hit.
    pub hit: TraceHit,
}

/// What a ray trace hit.
#[derive(Debug, Clone)]
pub enum TraceHit {
    /// Hit a wall (one-sided linedef or blocked two-sided).
    Wall {
        /// The index of the specific linedef the ray collided with.
        /// Useful for triggering linedef specials or spawning bullet puffs on walls.
        linedef_index: usize,
        /// X coordinate of the exact impact point in map space.
        hit_x: i32,
        /// Y coordinate of the exact impact point in map space.
        hit_y: i32,
    },
    /// Hit an actor.
    Actor {
        /// The index of the unfortunate actor in the `MobjSlab` that absorbed the hit.
        actor_index: usize,
        /// X coordinate of the exact impact point.
        hit_x: i32,
        /// Y coordinate of the exact impact point.
        hit_y: i32,
    },
    /// Ray reached max range without hitting anything.
    Nothing,
}

// ---------------------------------------------------------------------------
// Ray-linedef intersection
// ---------------------------------------------------------------------------

/// Test if a ray from `(rx, ry)` in direction `(rdx, rdy)` intersects a line segment
/// from `(v1x, v1y)` to `(v2x, v2y)`.
///
/// Returns `Some(t)` where `t >= 0` is the parametric distance along the ray
/// (in ray-direction units) and the intersection is within the segment bounds.
/// Returns `None` if the ray is parallel to the segment or misses it.
///
/// Uses the standard 2D ray-segment intersection formula:
/// - `t = ((v1 - r) x d2) / (d1 x d2)` (parametric along ray)
/// - `u = ((v1 - r) x d1) / (d1 x d2)` (parametric along segment, must be in [0, 1])
///
/// where `x` is the 2D cross product.
pub fn ray_linedef_intersection(
    rx: f32,
    ry: f32,
    rdx: f32,
    rdy: f32,
    v1x: f32,
    v1y: f32,
    v2x: f32,
    v2y: f32,
) -> Option<f32> {
    let d2x = v2x - v1x;
    let d2y = v2y - v1y;

    // Cross product of ray direction and segment direction.
    let denom = rdx * d2y - rdy * d2x;

    // Parallel (or collinear) rays never intersect for our purposes.
    if denom.abs() < EPSILON {
        return None;
    }

    let dx = v1x - rx;
    let dy = v1y - ry;

    // t = parametric distance along the ray.
    let t = (dx * d2y - dy * d2x) / denom;

    // u = parametric distance along the segment (must be in [0, 1]).
    let u = (dx * rdy - dy * rdx) / denom;

    if t >= 0.0 && (0.0..=1.0).contains(&u) {
        Some(t)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Line opening for two-sided linedefs
// ---------------------------------------------------------------------------

/// For a two-sided linedef, compute the passable floor/ceiling gap.
///
/// Returns `Some((open_bottom, open_top))` for two-sided lines:
/// - `open_bottom` = max of front and back sector floor heights
/// - `open_top` = min of front and back sector ceiling heights
///
/// Returns `None` for one-sided lines (left sidedef is `0xFFFF`).
pub fn line_opening(level: &Level, linedef: &doom_map::Linedef) -> Option<(i32, i32)> {
    if linedef.left_sidedef == doom_map::SIDEDEF_NONE {
        return None;
    }

    let right_sd = level.sidedefs.get(linedef.right_sidedef as usize)?;
    let left_sd = level.sidedefs.get(linedef.left_sidedef as usize)?;

    let front = level.sectors.get(right_sd.sector as usize)?;
    let back = level.sectors.get(left_sd.sector as usize)?;

    let open_bottom = front.floor_height.max(back.floor_height) as i32;
    let open_top = front.ceil_height.min(back.ceil_height) as i32;

    Some((open_bottom, open_top))
}

// ---------------------------------------------------------------------------
// Actors in blockmap cell
// ---------------------------------------------------------------------------

/// Find actors whose position falls within a blockmap cell.
///
/// A cell spans `[cell_x * 128 + origin_x, (cell_x + 1) * 128 + origin_x)`
/// in world coordinates. An actor is "in" the cell if its center is within
/// `radius` of the cell boundaries (bounding-box overlap).
///
/// Returns indices into `actor_positions`.
pub fn actors_in_cell(
    cell_x: i32,
    cell_y: i32,
    origin_x: i32,
    origin_y: i32,
    actor_positions: &[(i32, i32, i32, i32, bool)],
    result: &mut Vec<usize>,
) {
    let cell_min_x = cell_x * 128 + origin_x;
    let cell_min_y = cell_y * 128 + origin_y;
    let cell_max_x = cell_min_x + 128;
    let cell_max_y = cell_min_y + 128;

    result.clear();
    for (i, &(ax, ay, radius, _height, _shootable)) in actor_positions.iter().enumerate() {
        // Bounding-box overlap: actor extends from (ax - radius) to (ax + radius).
        if ax + radius >= cell_min_x
            && ax - radius < cell_max_x
            && ay + radius >= cell_min_y
            && ay - radius < cell_max_y
        {
            result.push(i);
        }
    }
}

// ---------------------------------------------------------------------------
// Ray-actor intersection
// ---------------------------------------------------------------------------

/// Test if a ray hits an actor represented as an axis-aligned bounding box.
///
/// The actor occupies a square from `(ax - radius, ay - radius)` to
/// `(ax + radius, ay + radius)`. Returns the parametric `t` along the ray
/// if the ray intersects the bounding box, `None` otherwise.
fn ray_actor_intersection(
    rx: f32,
    ry: f32,
    rdx: f32,
    rdy: f32,
    ax: f32,
    ay: f32,
    radius: f32,
) -> Option<f32> {
    // AABB intersection using slab method.
    let min_x = ax - radius;
    let max_x = ax + radius;
    let min_y = ay - radius;
    let max_y = ay + radius;

    let (mut t_min, mut t_max) = (f32::NEG_INFINITY, f32::INFINITY);

    if rdx.abs() < EPSILON {
        // Ray is vertical -- check X bounds.
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

    if rdy.abs() < EPSILON {
        // Ray is horizontal -- check Y bounds.
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

    // Return the entry point (first positive t).
    let t = if t_min >= 0.0 { t_min } else { t_max };
    if t >= 0.0 { Some(t) } else { None }
}

// ---------------------------------------------------------------------------
// trace_ray — main DDA traversal
// ---------------------------------------------------------------------------

/// Cast a ray from `(x1, y1)` in direction `(angle_cos, angle_sin)` for
/// `max_range` map units. Tests linedefs via blockmap DDA traversal and
/// optionally tests actors in each visited cell.
///
/// # Parameters
/// - `level` — the loaded level (geometry + blockmap)
/// - `x1, y1` — ray origin in map-unit integers
/// - `angle_cos, angle_sin` — unit-direction components (cos(angle), sin(angle))
/// - `max_range` — maximum ray distance in map units
/// - `actor_check` — an `ActorCheck` enum specifying if actors should be checked and providing the actor slice
///
/// # Returns
/// The closest hit along the ray (wall, actor, or nothing).
///
/// Defines how actors should be checked during a ray trace.
#[derive(Clone, Copy)]
pub enum ActorCheck<'a> {
    /// Ignore actors completely; only trace against level geometry.
    Ignore,
    /// Check for collisions with actors.
    Check {
        /// Optional index of the shooter to ignore (prevents self-hit).
        shooter_index: Option<usize>,
        /// Slice of actors to check: `(x, y, radius, height, shootable)`.
        actor_positions: &'a [(i32, i32, i32, i32, bool)],
    },
}

/// Traces a ray through the level geometry to find the first blocking collision.
///
/// This function performs line-of-sight checking and hitscan collision detection against
/// both level geometry (linedefs) and actors (if specified via `actor_check`). It uses the
/// Blockmap to efficiently traverse only the grid cells intersected by the ray.
///
/// ## Returns
/// A [`TraceResult`] indicating what the ray hit, if anything.
///
/// ## Examples
///
/// ```
/// use doom_game::trace::{trace_ray, ActorCheck};
/// use doom_map::Level;
///
/// # let level = Level::default();
/// # let x1 = 0;
/// # let y1 = 0;
/// # let angle_cos = 1.0;
/// # let angle_sin = 0.0;
/// # let max_range = 100.0;
/// // Trace a ray to the east for 100 units, ignoring actors.
/// let hit = trace_ray(
///     &level,
///     x1,
///     y1,
///     angle_cos,
///     angle_sin,
///     max_range,
///     ActorCheck::Ignore,
/// );
/// ```
pub fn trace_ray(
    level: &Level,
    x1: i32,
    y1: i32,
    angle_cos: f32,
    angle_sin: f32,
    max_range: f32,
    actor_check: ActorCheck<'_>,
) -> TraceResult {
    // Zero-range ray can't hit anything.
    if max_range <= 0.0 {
        return TraceResult {
            frac: 0.0,
            hit: TraceHit::Nothing,
        };
    }

    // Ray direction scaled to max_range.
    let rdx = angle_cos * max_range;
    let rdy = angle_sin * max_range;

    // If the ray has no meaningful direction, bail.
    if rdx.abs() < EPSILON && rdy.abs() < EPSILON {
        return TraceResult {
            frac: 0.0,
            hit: TraceHit::Nothing,
        };
    }

    let origin_x = level.blockmap.x_origin as i32;
    let origin_y = level.blockmap.y_origin as i32;
    let cols = level.blockmap.x_count as i32;
    let rows = level.blockmap.y_count as i32;

    // Starting cell in blockmap coordinates.
    let fx1 = x1 as f32;
    let fy1 = y1 as f32;
    let start_cell_x = ((fx1 - origin_x as f32) / BLOCK_SIZE).floor() as i32;
    let start_cell_y = ((fy1 - origin_y as f32) / BLOCK_SIZE).floor() as i32;

    // DDA setup.
    let step_x: i32 = if rdx > 0.0 {
        1
    } else if rdx < 0.0 {
        -1
    } else {
        0
    };
    let step_y: i32 = if rdy > 0.0 {
        1
    } else if rdy < 0.0 {
        -1
    } else {
        0
    };

    // Distance in t-units to the next cell boundary.
    let t_delta_x = if rdx.abs() > EPSILON {
        (BLOCK_SIZE / rdx.abs()).abs()
    } else {
        f32::INFINITY
    };
    let t_delta_y = if rdy.abs() > EPSILON {
        (BLOCK_SIZE / rdy.abs()).abs()
    } else {
        f32::INFINITY
    };

    // t_max: parametric distance to the first cell boundary crossing.
    // The parametric `t` here is in units where t=1.0 means we've traveled max_range.
    let t_max_x = if rdx.abs() > EPSILON {
        let cell_edge_x = if rdx > 0.0 {
            (start_cell_x + 1) as f32 * BLOCK_SIZE + origin_x as f32
        } else {
            start_cell_x as f32 * BLOCK_SIZE + origin_x as f32
        };
        ((cell_edge_x - fx1) / rdx).abs()
    } else {
        f32::INFINITY
    };

    let t_max_y = if rdy.abs() > EPSILON {
        let cell_edge_y = if rdy > 0.0 {
            (start_cell_y + 1) as f32 * BLOCK_SIZE + origin_y as f32
        } else {
            start_cell_y as f32 * BLOCK_SIZE + origin_y as f32
        };
        ((cell_edge_y - fy1) / rdy).abs()
    } else {
        f32::INFINITY
    };

    let mut cell_x = start_cell_x;
    let mut cell_y = start_cell_y;
    let mut cur_t_max_x = t_max_x;
    let mut cur_t_max_y = t_max_y;

    // Track the closest hit found so far.
    let mut best_frac: f32 = f32::INFINITY;
    let mut best_hit = TraceHit::Nothing;

    // Track which linedefs we've already tested to avoid duplicates
    // (linedefs can appear in multiple blockmap cells).
    // ⚡ Bolt: Using a small vector of visited indices instead of a level-sized
    // boolean array saves massive allocations on large maps, as a ray typically
    // tests fewer than 32 lines.
    let mut tested_lines = Vec::with_capacity(32);

    // Maximum cells to visit (safety limit against infinite loops).
    let max_cells = (cols + rows) as usize * 2 + 4;

    // Buffer for actor overlap tests to avoid per-cell allocations.
    // ⚡ Bolt: Pre-allocate a small capacity to avoid multiple reallocations.
    let mut cell_actors = Vec::with_capacity(16);

    for _step in 0..max_cells {
        // Only process cells within the blockmap grid.
        if cell_x >= 0 && cell_x < cols && cell_y >= 0 && cell_y < rows {
            // Test linedefs in this cell.
            for ld_idx_u16 in level
                .blockmap
                .block_linedefs(cell_x as usize, cell_y as usize)
            {
                let ld_idx = ld_idx_u16 as usize;
                if ld_idx >= level.linedefs.len() {
                    continue;
                }
                if tested_lines.contains(&ld_idx) {
                    continue;
                }
                tested_lines.push(ld_idx);

                let ld = &level.linedefs[ld_idx];
                let v1 = &level.vertexes[ld.from_vertex as usize];
                let v2 = &level.vertexes[ld.to_vertex as usize];

                let Some(t_val) = ray_linedef_intersection(
                    fx1,
                    fy1,
                    rdx,
                    rdy,
                    v1.x as f32,
                    v1.y as f32,
                    v2.x as f32,
                    v2.y as f32,
                ) else {
                    continue;
                };

                // t_val is in parametric units where 1.0 = max_range.
                if !(0.0..=1.0).contains(&t_val) || t_val >= best_frac {
                    continue;
                }

                // Check if this line blocks the ray.
                let blocks = if !ld.is_two_sided() {
                    // One-sided: always blocks.
                    true
                } else {
                    // Two-sided: check the opening.
                    match line_opening(level, ld) {
                        Some((open_bottom, open_top)) => {
                            // A two-sided line blocks if the opening is closed.
                            open_top <= open_bottom
                        }
                        None => true, // shouldn't happen for two-sided, but safety
                    }
                };

                if blocks {
                    let hit_x = (fx1 + rdx * t_val) as i32;
                    let hit_y = (fy1 + rdy * t_val) as i32;
                    best_frac = t_val;
                    best_hit = TraceHit::Wall {
                        linedef_index: ld_idx,
                        hit_x,
                        hit_y,
                    };
                }
            }

            // Test actors in this cell.
            if let ActorCheck::Check {
                shooter_index,
                actor_positions,
            } = actor_check
            {
                actors_in_cell(
                    cell_x,
                    cell_y,
                    origin_x,
                    origin_y,
                    actor_positions,
                    &mut cell_actors,
                );

                for &actor_idx in &cell_actors {
                    // Skip the shooter.
                    if shooter_index == Some(actor_idx) {
                        continue;
                    }

                    let (ax, ay, radius, _height, shootable) = actor_positions[actor_idx];

                    // Skip non-shootable or dead actors.
                    if !shootable {
                        continue;
                    }

                    let Some(t_val) = ray_actor_intersection(
                        fx1,
                        fy1,
                        rdx,
                        rdy,
                        ax as f32,
                        ay as f32,
                        radius as f32,
                    ) else {
                        continue;
                    };

                    if !(0.0..=1.0).contains(&t_val) || t_val >= best_frac {
                        continue;
                    }

                    let hit_x = (fx1 + rdx * t_val) as i32;
                    let hit_y = (fy1 + rdy * t_val) as i32;
                    best_frac = t_val;
                    best_hit = TraceHit::Actor {
                        actor_index: actor_idx,
                        hit_x,
                        hit_y,
                    };
                }
            }
        }

        // If we've already found a hit closer than the next cell boundary,
        // we can stop early.
        let next_t = cur_t_max_x.min(cur_t_max_y);
        if best_frac <= next_t {
            break;
        }

        // Step to the next cell.
        if cur_t_max_x < cur_t_max_y {
            cell_x += step_x;
            cur_t_max_x += t_delta_x;
        } else {
            cell_y += step_y;
            cur_t_max_y += t_delta_y;
        }

        // If we've gone past max_range (t > 1.0), stop.
        if next_t > 1.0 {
            break;
        }
    }

    if best_frac <= 1.0 {
        TraceResult {
            frac: best_frac,
            hit: best_hit,
        }
    } else {
        TraceResult {
            frac: 1.0,
            hit: TraceHit::Nothing,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helper: build a minimal level for testing
    // -----------------------------------------------------------------------

    /// Build a minimal level with specified linedefs, vertices, sectors, and sidedefs.
    ///
    /// The blockmap is built to cover the given geometry.
    fn make_test_level(
        vertexes: Vec<doom_map::Vertex>,
        linedefs: Vec<doom_map::Linedef>,
        sidedefs: Vec<doom_map::Sidedef>,
        sectors: Vec<doom_map::Sector>,
        blockmap_origin: (i16, i16),
        blockmap_size: (u16, u16),
        blockmap_cells: Vec<Vec<u16>>,
    ) -> Level {
        // Build blockmap raw bytes.
        let (bm_ox, bm_oy) = blockmap_origin;
        let (bm_cols, bm_rows) = blockmap_size;
        let n_blocks = bm_cols as usize * bm_rows as usize;

        let mut raw = Vec::new();
        // Header: origin_x, origin_y, x_count, y_count
        raw.extend_from_slice(&bm_ox.to_le_bytes());
        raw.extend_from_slice(&bm_oy.to_le_bytes());
        raw.extend_from_slice(&bm_cols.to_le_bytes());
        raw.extend_from_slice(&bm_rows.to_le_bytes());

        // Compute offsets (in 16-bit words from start of lump).
        // Header is 4 words. Then n_blocks offsets.
        let header_words = 4usize;
        let offset_table_words = n_blocks;
        let mut data_start = header_words + offset_table_words;

        // First pass: compute offsets.
        let mut offsets = Vec::with_capacity(n_blocks);
        let mut block_data: Vec<Vec<u16>> = Vec::with_capacity(n_blocks);

        for i in 0..n_blocks {
            offsets.push(data_start as u16);
            let cell_linedefs = if i < blockmap_cells.len() {
                &blockmap_cells[i]
            } else {
                &vec![]
            };
            // Each block: 0x0000 sentinel + linedef indices + 0xFFFF terminator
            let mut bd = vec![0x0000u16];
            bd.extend_from_slice(cell_linedefs);
            bd.push(0xFFFF);
            data_start += bd.len();
            block_data.push(bd);
        }

        // Write offsets.
        for off in &offsets {
            raw.extend_from_slice(&off.to_le_bytes());
        }

        // Write block data.
        for bd in &block_data {
            for val in bd {
                raw.extend_from_slice(&val.to_le_bytes());
            }
        }

        let blockmap = doom_map::Blockmap::parse_lump(&raw).expect("blockmap parse");

        // Minimal reject (all visible).
        let n_sectors = sectors.len().max(1);
        let reject_size = (n_sectors * n_sectors).div_ceil(8);
        let reject_data = vec![0u8; reject_size];
        let reject = doom_map::Reject::parse_lump(&reject_data, n_sectors).expect("reject parse");

        Level {
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

    /// Make a single sector with floor=0, ceil=128.
    fn make_sector(floor: i16, ceil: i16) -> doom_map::Sector {
        doom_map::Sector {
            floor_height: floor,
            ceil_height: ceil,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }
    }

    /// Make a sidedef pointing to a given sector.
    fn make_sidedef(sector: u16) -> doom_map::Sidedef {
        doom_map::Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: [0; 8],
            lower_texture: [0; 8],
            middle_texture: *b"WALL1\0\0\0",
            sector,
        }
    }

    /// Make a one-sided linedef.
    fn make_linedef_one_sided(from: u16, to: u16, right_sd: u16) -> doom_map::Linedef {
        doom_map::Linedef {
            from_vertex: from,
            to_vertex: to,
            flags: 0, // not two-sided
            special: 0,
            tag: 0,
            right_sidedef: right_sd,
            left_sidedef: doom_map::SIDEDEF_NONE,
        }
    }

    /// Make a two-sided linedef.
    fn make_linedef_two_sided(
        from: u16,
        to: u16,
        right_sd: u16,
        left_sd: u16,
    ) -> doom_map::Linedef {
        doom_map::Linedef {
            from_vertex: from,
            to_vertex: to,
            flags: doom_map::FLAG_TWO_SIDED,
            special: 0,
            tag: 0,
            right_sidedef: right_sd,
            left_sidedef: left_sd,
        }
    }

    /// Build a simple level with one horizontal wall (one-sided) at y=64,
    /// spanning x=[0, 128], inside a 2x2 blockmap grid starting at origin (0,0).
    fn make_wall_level() -> Level {
        let verts = vec![
            doom_map::Vertex { x: 0, y: 64 },   // v0
            doom_map::Vertex { x: 128, y: 64 }, // v1
        ];
        let sds = vec![make_sidedef(0)]; // sd0 -> sector 0
        let lds = vec![make_linedef_one_sided(0, 1, 0)]; // ld0: v0->v1
        let secs = vec![make_sector(0, 128)];

        // Blockmap: 2 cols x 2 rows, origin at (0, 0).
        // Linedef 0 spans cells (0,0) and (1,0) (at y=64 which is in row 0).
        // Actually blockmap cells are 128-wide:
        //   cell(0,0) = [0,128) x [0,128)
        //   cell(1,0) = [128,256) x [0,128)
        // The linedef goes from (0,64) to (128,64), so it's in cell(0,0) only
        // (vertex at x=128 is on the boundary).
        let cells = vec![
            vec![0u16], // cell (0,0): linedef 0
            vec![],     // cell (1,0)
            vec![],     // cell (0,1)
            vec![],     // cell (1,1)
        ];

        make_test_level(verts, lds, sds, secs, (0, 0), (2, 2), cells)
    }

    // -----------------------------------------------------------------------
    // ray_linedef_intersection tests
    // -----------------------------------------------------------------------

    #[test]
    fn ray_hits_perpendicular_wall_at_midpoint() {
        // Ray going right (+x), wall is vertical.
        // Wall from (100, -50) to (100, 50). Ray from (0, 0) direction (1, 0).
        let t = ray_linedef_intersection(0.0, 0.0, 1.0, 0.0, 100.0, -50.0, 100.0, 50.0);
        assert!(t.is_some(), "should hit perpendicular wall");
        let t = t.unwrap();
        assert!((t - 100.0).abs() < 0.01, "t should be ~100, got {t}");
    }

    #[test]
    fn ray_parallel_to_segment_no_intersection() {
        // Ray going right (+x), wall is also horizontal (parallel).
        let t = ray_linedef_intersection(0.0, 0.0, 1.0, 0.0, 50.0, 10.0, 150.0, 10.0);
        assert!(t.is_none(), "parallel ray should not intersect");
    }

    #[test]
    fn ray_hits_endpoint_of_segment() {
        // Wall from (50, -50) to (50, 0). Ray from (0, 0) direction (1, 0).
        // Ray should hit at t=50 (the endpoint at y=0).
        let t = ray_linedef_intersection(0.0, 0.0, 1.0, 0.0, 50.0, -50.0, 50.0, 0.0);
        assert!(t.is_some(), "should hit segment at endpoint");
        let t = t.unwrap();
        assert!((t - 50.0).abs() < 0.01, "t should be ~50, got {t}");
    }

    #[test]
    fn ray_in_opposite_direction_no_hit() {
        // Ray going left (-x), wall is to the right.
        let t = ray_linedef_intersection(0.0, 0.0, -1.0, 0.0, 100.0, -50.0, 100.0, 50.0);
        assert!(t.is_none(), "ray pointing away should not hit");
    }

    #[test]
    fn collinear_ray_no_hit() {
        // Ray along the wall itself (collinear).
        let t = ray_linedef_intersection(0.0, 0.0, 1.0, 0.0, 50.0, 0.0, 150.0, 0.0);
        assert!(t.is_none(), "collinear ray should not intersect");
    }

    #[test]
    fn ray_hits_diagonal_wall() {
        // Diagonal wall from (0, 100) to (100, 0). Ray from (0, 0) going (1, 1).
        let t = ray_linedef_intersection(0.0, 0.0, 1.0, 1.0, 0.0, 100.0, 100.0, 0.0);
        assert!(t.is_some(), "should hit diagonal wall");
        let t = t.unwrap();
        assert!((t - 50.0).abs() < 0.1, "t should be ~50, got {t}");
    }

    #[test]
    fn ray_misses_segment_off_to_the_side() {
        // Wall far above the ray.
        let t = ray_linedef_intersection(0.0, 0.0, 1.0, 0.0, 50.0, 100.0, 50.0, 200.0);
        assert!(t.is_none(), "ray should miss wall that's off to the side");
    }

    #[test]
    fn ray_zero_length_direction() {
        // Ray with zero direction.
        let t = ray_linedef_intersection(0.0, 0.0, 0.0, 0.0, 50.0, -50.0, 50.0, 50.0);
        assert!(t.is_none(), "zero-direction ray should not hit");
    }

    #[test]
    fn ray_hits_segment_at_angle() {
        // Ray at 45 degrees hitting a horizontal wall.
        // Wall from (0, 50) to (100, 50). Ray from (25, 0) direction (0, 1).
        let t = ray_linedef_intersection(25.0, 0.0, 0.0, 1.0, 0.0, 50.0, 100.0, 50.0);
        assert!(t.is_some(), "should hit horizontal wall");
        let t = t.unwrap();
        assert!((t - 50.0).abs() < 0.01, "t should be ~50, got {t}");
    }

    #[test]
    fn ray_hits_segment_barely_within_range() {
        // Ray going right, wall at x=99. Range direction is 100 units.
        let t = ray_linedef_intersection(0.0, 0.0, 100.0, 0.0, 99.0, -50.0, 99.0, 50.0);
        assert!(t.is_some(), "should hit wall within range");
        let t = t.unwrap();
        // t is in ray-direction units: 99.0 / 100.0 = 0.99
        assert!((t - 0.99).abs() < 0.01, "t should be ~0.99, got {t}");
    }

    // -----------------------------------------------------------------------
    // line_opening tests
    // -----------------------------------------------------------------------

    #[test]
    fn one_sided_line_returns_none() {
        let level = make_wall_level();
        let opening = line_opening(&level, &level.linedefs[0]);
        assert!(opening.is_none(), "one-sided line should return None");
    }

    #[test]
    fn two_sided_same_floor_ceiling() {
        let secs = vec![make_sector(0, 128), make_sector(0, 128)];
        let sds = vec![make_sidedef(0), make_sidedef(1)];
        let verts = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 128, y: 0 },
        ];
        let ld = make_linedef_two_sided(0, 1, 0, 1);

        let level = make_test_level(
            verts,
            vec![ld.clone()],
            sds,
            secs,
            (0, 0),
            (1, 1),
            vec![vec![0]],
        );

        let opening = line_opening(&level, &ld);
        assert!(opening.is_some());
        let (bottom, top) = opening.unwrap();
        assert_eq!(bottom, 0, "same floor heights -> bottom = 0");
        assert_eq!(top, 128, "same ceil heights -> top = 128");
    }

    #[test]
    fn two_sided_different_floor_heights() {
        // Front sector: floor=0, ceil=128. Back sector: floor=32, ceil=128.
        let secs = vec![make_sector(0, 128), make_sector(32, 128)];
        let sds = vec![make_sidedef(0), make_sidedef(1)];
        let verts = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 128, y: 0 },
        ];
        let ld = make_linedef_two_sided(0, 1, 0, 1);

        let level = make_test_level(
            verts,
            vec![ld.clone()],
            sds,
            secs,
            (0, 0),
            (1, 1),
            vec![vec![0]],
        );

        let opening = line_opening(&level, &ld);
        assert!(opening.is_some());
        let (bottom, top) = opening.unwrap();
        assert_eq!(bottom, 32, "open_bottom = max(0, 32) = 32");
        assert_eq!(top, 128, "open_top = min(128, 128) = 128");
    }

    #[test]
    fn two_sided_gap_too_small() {
        // Front floor=100, ceil=128. Back floor=0, ceil=100.
        // opening = (100, 100) -> gap = 0.
        let secs = vec![make_sector(100, 128), make_sector(0, 100)];
        let sds = vec![make_sidedef(0), make_sidedef(1)];
        let verts = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 128, y: 0 },
        ];
        let ld = make_linedef_two_sided(0, 1, 0, 1);

        let level = make_test_level(
            verts,
            vec![ld.clone()],
            sds,
            secs,
            (0, 0),
            (1, 1),
            vec![vec![0]],
        );

        let opening = line_opening(&level, &ld);
        assert!(opening.is_some());
        let (bottom, top) = opening.unwrap();
        assert_eq!(bottom, 100);
        assert_eq!(top, 100);
        assert!(top <= bottom, "gap is zero — should block");
    }

    // -----------------------------------------------------------------------
    // actors_in_cell tests
    // -----------------------------------------------------------------------

    #[test]
    fn actors_in_correct_cell() {
        // Cell (0,0) covers [0, 128) x [0, 128). Actor at (64, 64) with radius 16.
        let actors = vec![(64, 64, 16, 56, true)];
        let mut result = Vec::new();
        actors_in_cell(0, 0, 0, 0, &actors, &mut result);
        assert_eq!(result, vec![0], "actor at (64,64) should be in cell (0,0)");
    }

    #[test]
    fn actors_in_adjacent_cell_excluded() {
        // Cell (1,0) covers [128, 256) x [0, 128). Actor at (64, 64) radius 16.
        let actors = vec![(64, 64, 16, 56, true)];
        let mut result = Vec::new();
        actors_in_cell(1, 0, 0, 0, &actors, &mut result);
        assert!(
            result.is_empty(),
            "actor at (64,64) should NOT be in cell (1,0)"
        );
    }

    #[test]
    fn empty_cell_returns_empty() {
        let actors: Vec<(i32, i32, i32, i32, bool)> = vec![];
        let mut result = Vec::new();
        actors_in_cell(0, 0, 0, 0, &actors, &mut result);
        assert!(result.is_empty(), "no actors -> empty result");
    }

    #[test]
    fn actor_on_cell_boundary_overlap() {
        // Actor at (120, 64), radius 20. Extends to x=140 which overlaps cell(1,0) = [128,256).
        let actors = vec![(120, 64, 20, 56, true)];

        let mut result0 = Vec::new();
        actors_in_cell(0, 0, 0, 0, &actors, &mut result0);

        let mut result1 = Vec::new();
        actors_in_cell(1, 0, 0, 0, &actors, &mut result1);

        assert!(result0.contains(&0), "actor should be in cell(0,0)");
        assert!(
            result1.contains(&0),
            "actor should also overlap into cell(1,0)"
        );
    }

    #[test]
    fn multiple_actors_in_same_cell() {
        let actors = vec![
            (32, 32, 10, 56, true),
            (96, 96, 10, 56, true),
            (200, 200, 10, 56, true), // in cell (1,1)
        ];
        let mut result = Vec::new();
        actors_in_cell(0, 0, 0, 0, &actors, &mut result);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&0));
        assert!(result.contains(&1));
    }

    #[test]
    fn actors_with_nonzero_origin() {
        // Origin at (-128, -128). Cell(0,0) covers [-128, 0) x [-128, 0).
        let actors = vec![(-64, -64, 10, 56, true)];
        let mut result = Vec::new();
        actors_in_cell(0, 0, -128, -128, &actors, &mut result);
        assert!(
            result.contains(&0),
            "actor should be in cell with shifted origin"
        );
    }

    // -----------------------------------------------------------------------
    // trace_ray tests — wall hits
    // -----------------------------------------------------------------------

    #[test]
    fn ray_hits_one_sided_wall_directly_ahead() {
        let level = make_wall_level();
        // Ray from (64, 0) going north (up +y), wall at y=64.
        let result = trace_ray(
            &level,
            64,
            0,
            0.0,   // cos (east component)
            1.0,   // sin (north component)
            200.0, // max range
            ActorCheck::Ignore,
        );
        match &result.hit {
            TraceHit::Wall { linedef_index, .. } => {
                assert_eq!(*linedef_index, 0, "should hit linedef 0");
            }
            other => panic!("expected Wall hit, got {other:?}"),
        }
        assert!(
            result.frac > 0.0 && result.frac < 1.0,
            "frac should be between 0 and 1"
        );
    }

    #[test]
    fn ray_misses_wall_off_to_the_side() {
        let level = make_wall_level();
        // Ray from (64, 0) going east (+x), wall at y=64 is horizontal.
        let result = trace_ray(
            &level,
            64,
            0,
            1.0, // cos
            0.0, // sin
            200.0,
            ActorCheck::Ignore,
        );
        assert!(
            matches!(result.hit, TraceHit::Nothing),
            "ray going east should miss horizontal wall"
        );
    }

    #[test]
    fn ray_hits_closest_wall_when_multiple() {
        // Two walls: one at y=32 and one at y=96.
        let verts = vec![
            doom_map::Vertex { x: 0, y: 32 },   // v0
            doom_map::Vertex { x: 128, y: 32 }, // v1
            doom_map::Vertex { x: 0, y: 96 },   // v2
            doom_map::Vertex { x: 128, y: 96 }, // v3
        ];
        let sds = vec![make_sidedef(0), make_sidedef(0)];
        let lds = vec![
            make_linedef_one_sided(0, 1, 0), // ld0 at y=32
            make_linedef_one_sided(2, 3, 1), // ld1 at y=96
        ];
        let secs = vec![make_sector(0, 128)];

        let cells = vec![
            vec![0u16, 1], // cell (0,0): both linedefs
        ];

        let level = make_test_level(verts, lds, sds, secs, (0, 0), (1, 1), cells);

        // Ray from (64, 0) going north.
        let result = trace_ray(&level, 64, 0, 0.0, 1.0, 200.0, ActorCheck::Ignore);
        match &result.hit {
            TraceHit::Wall { linedef_index, .. } => {
                assert_eq!(*linedef_index, 0, "should hit closer wall (ld0 at y=32)");
            }
            other => panic!("expected Wall, got {other:?}"),
        }
    }

    #[test]
    fn ray_through_empty_space_returns_nothing() {
        // Level with no linedefs in the ray's path.
        let verts = vec![
            doom_map::Vertex { x: 200, y: 200 },
            doom_map::Vertex { x: 250, y: 200 },
        ];
        let sds = vec![make_sidedef(0)];
        let lds = vec![make_linedef_one_sided(0, 1, 0)];
        let secs = vec![make_sector(0, 128)];
        let cells = vec![
            vec![],     // cell (0,0)
            vec![],     // cell (1,0)
            vec![],     // cell (0,1)
            vec![0u16], // cell (1,1) has the linedef
        ];

        let level = make_test_level(verts, lds, sds, secs, (0, 0), (2, 2), cells);

        // Ray from (64, 64) going east, max range 100. Wall is far away at (200,200).
        let result = trace_ray(&level, 64, 64, 1.0, 0.0, 100.0, ActorCheck::Ignore);
        assert!(
            matches!(result.hit, TraceHit::Nothing),
            "should miss everything"
        );
    }

    #[test]
    fn ray_zero_range_returns_nothing() {
        let level = make_wall_level();
        let result = trace_ray(&level, 64, 0, 0.0, 1.0, 0.0, ActorCheck::Ignore);
        assert!(
            matches!(result.hit, TraceHit::Nothing),
            "zero range should return Nothing"
        );
    }

    #[test]
    fn ray_across_multiple_blockmap_cells() {
        // Wall at y=200 in cell row 1. Ray starts in cell row 0.
        let verts = vec![
            doom_map::Vertex { x: 0, y: 200 },
            doom_map::Vertex { x: 128, y: 200 },
        ];
        let sds = vec![make_sidedef(0)];
        let lds = vec![make_linedef_one_sided(0, 1, 0)];
        let secs = vec![make_sector(0, 128)];
        let cells = vec![
            vec![],     // cell (0,0)
            vec![0u16], // cell (0,1): linedef at y=200
        ];

        let level = make_test_level(verts, lds, sds, secs, (0, 0), (1, 2), cells);

        let result = trace_ray(&level, 64, 0, 0.0, 1.0, 300.0, ActorCheck::Ignore);
        match &result.hit {
            TraceHit::Wall { linedef_index, .. } => {
                assert_eq!(*linedef_index, 0);
            }
            other => panic!("expected Wall hit across cells, got {other:?}"),
        }
    }

    #[test]
    fn ray_through_passable_two_sided_line() {
        // Two-sided line with full opening (same floor/ceil on both sides).
        // Ray should pass through without hitting.
        let secs = vec![make_sector(0, 128), make_sector(0, 128)];
        let sds = vec![make_sidedef(0), make_sidedef(1)];
        let verts = vec![
            doom_map::Vertex { x: 0, y: 64 },
            doom_map::Vertex { x: 128, y: 64 },
        ];
        let lds = vec![make_linedef_two_sided(0, 1, 0, 1)];

        let cells = vec![vec![0u16]];
        let level = make_test_level(verts, lds, sds, secs, (0, 0), (1, 1), cells);

        let result = trace_ray(&level, 64, 0, 0.0, 1.0, 200.0, ActorCheck::Ignore);
        assert!(
            matches!(result.hit, TraceHit::Nothing),
            "ray should pass through open two-sided line"
        );
    }

    #[test]
    fn ray_blocked_by_closed_two_sided_line() {
        // Two-sided line with zero opening (floor=ceil on one side).
        let secs = vec![make_sector(100, 128), make_sector(0, 100)];
        let sds = vec![make_sidedef(0), make_sidedef(1)];
        let verts = vec![
            doom_map::Vertex { x: 0, y: 64 },
            doom_map::Vertex { x: 128, y: 64 },
        ];
        let lds = vec![make_linedef_two_sided(0, 1, 0, 1)];

        let cells = vec![vec![0u16]];
        let level = make_test_level(verts, lds, sds, secs, (0, 0), (1, 1), cells);

        let result = trace_ray(&level, 64, 0, 0.0, 1.0, 200.0, ActorCheck::Ignore);
        assert!(
            matches!(result.hit, TraceHit::Wall { .. }),
            "ray should be blocked by closed two-sided line"
        );
    }

    // -----------------------------------------------------------------------
    // trace_ray tests — actor hits
    // -----------------------------------------------------------------------

    #[test]
    fn ray_hits_actor_before_wall() {
        let level = make_wall_level(); // wall at y=64
        // Actor at (64, 32) with radius 10 — between shooter and wall.
        let actors = vec![(64, 32, 10, 56, true)];
        let result = trace_ray(
            &level,
            64,
            0,
            0.0,
            1.0,
            200.0,
            ActorCheck::Check {
                shooter_index: None,
                actor_positions: &actors,
            },
        );
        match &result.hit {
            TraceHit::Actor { actor_index, .. } => {
                assert_eq!(*actor_index, 0, "should hit actor 0");
            }
            other => panic!("expected Actor hit, got {other:?}"),
        }
    }

    #[test]
    fn ray_hits_wall_before_actor() {
        let level = make_wall_level(); // wall at y=64
        // Actor at (64, 100) with radius 10 — behind the wall.
        let actors = vec![(64, 100, 10, 56, true)];
        let result = trace_ray(
            &level,
            64,
            0,
            0.0,
            1.0,
            200.0,
            ActorCheck::Check {
                shooter_index: None,
                actor_positions: &actors,
            },
        );
        match &result.hit {
            TraceHit::Wall { linedef_index, .. } => {
                assert_eq!(*linedef_index, 0, "should hit wall before actor");
            }
            other => panic!("expected Wall hit, got {other:?}"),
        }
    }

    #[test]
    fn ray_skips_shooter_index() {
        let level = make_wall_level();
        // Two actors in the path: actor 0 (shooter) and actor 1 (target).
        let actors = vec![
            (64, 0, 10, 56, true),  // actor 0: the shooter
            (64, 32, 10, 56, true), // actor 1: in the line of fire
        ];
        let result = trace_ray(
            &level,
            64,
            0,
            0.0,
            1.0,
            200.0,
            ActorCheck::Check {
                shooter_index: Some(0),
                actor_positions: &actors,
            },
        );
        match &result.hit {
            TraceHit::Actor { actor_index, .. } => {
                assert_eq!(*actor_index, 1, "should skip shooter (0) and hit actor 1");
            }
            other => panic!("expected Actor 1 hit, got {other:?}"),
        }
    }

    #[test]
    fn ray_ignores_dead_non_shootable_actors() {
        let level = make_wall_level();
        // Actor with shootable=false.
        let actors = vec![(64, 32, 10, 56, false)];
        let result = trace_ray(
            &level,
            64,
            0,
            0.0,
            1.0,
            200.0,
            ActorCheck::Check {
                shooter_index: None,
                actor_positions: &actors,
            },
        );
        // Should pass through the non-shootable actor and hit the wall.
        assert!(
            matches!(result.hit, TraceHit::Wall { .. }),
            "should ignore non-shootable actor and hit wall"
        );
    }

    #[test]
    fn ray_hits_actor_in_empty_level() {
        // Level with no walls in the path.
        let secs = vec![make_sector(0, 128)];
        let level = make_test_level(
            vec![],
            vec![],
            vec![],
            secs,
            (0, 0),
            (2, 2),
            vec![vec![], vec![], vec![], vec![]],
        );

        let actors = vec![(64, 100, 20, 56, true)];
        let result = trace_ray(
            &level,
            64,
            0,
            0.0,
            1.0,
            200.0,
            ActorCheck::Check {
                shooter_index: None,
                actor_positions: &actors,
            },
        );
        match &result.hit {
            TraceHit::Actor { actor_index, .. } => {
                assert_eq!(*actor_index, 0);
            }
            other => panic!("expected Actor hit, got {other:?}"),
        }
    }

    #[test]
    fn ray_max_range_misses_distant_actor() {
        let secs = vec![make_sector(0, 128)];
        let level = make_test_level(
            vec![],
            vec![],
            vec![],
            secs,
            (0, 0),
            (4, 4),
            vec![vec![]; 16],
        );

        // Actor at (64, 500) — beyond max_range of 200.
        let actors = vec![(64, 500, 20, 56, true)];
        let result = trace_ray(
            &level,
            64,
            0,
            0.0,
            1.0,
            200.0,
            ActorCheck::Check {
                shooter_index: None,
                actor_positions: &actors,
            },
        );
        assert!(
            matches!(result.hit, TraceHit::Nothing),
            "actor beyond max range should not be hit"
        );
    }

    #[test]
    fn ray_hits_closer_of_two_actors() {
        let secs = vec![make_sector(0, 128)];
        let level = make_test_level(
            vec![],
            vec![],
            vec![],
            secs,
            (0, 0),
            (2, 2),
            vec![vec![]; 4],
        );

        let actors = vec![
            (64, 100, 20, 56, true), // actor 0: farther
            (64, 50, 20, 56, true),  // actor 1: closer
        ];
        let result = trace_ray(
            &level,
            64,
            0,
            0.0,
            1.0,
            200.0,
            ActorCheck::Check {
                shooter_index: None,
                actor_positions: &actors,
            },
        );
        match &result.hit {
            TraceHit::Actor { actor_index, .. } => {
                assert_eq!(*actor_index, 1, "should hit the closer actor");
            }
            other => panic!("expected Actor hit, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // ray_actor_intersection tests
    // -----------------------------------------------------------------------

    #[test]
    fn ray_actor_hit_direct() {
        // Ray from (0,0) going right, actor at (50, 0) radius 10.
        let t = ray_actor_intersection(0.0, 0.0, 1.0, 0.0, 50.0, 0.0, 10.0);
        assert!(t.is_some());
        let t = t.unwrap();
        assert!(
            (t - 40.0).abs() < 0.1,
            "should hit at t=~40 (50-10), got {t}"
        );
    }

    #[test]
    fn ray_actor_miss_perpendicular() {
        // Ray from (0,0) going right, actor at (50, 50) radius 10 — too far away.
        let t = ray_actor_intersection(0.0, 0.0, 1.0, 0.0, 50.0, 50.0, 10.0);
        assert!(t.is_none(), "should miss actor that's far off to the side");
    }

    #[test]
    fn ray_actor_behind_shooter() {
        // Ray from (0,0) going right, actor at (-50, 0) radius 10.
        let t = ray_actor_intersection(0.0, 0.0, 1.0, 0.0, -50.0, 0.0, 10.0);
        assert!(t.is_none(), "should not hit actor behind the shooter");
    }

    #[test]
    fn ray_actor_origin_inside_actor() {
        // Ray starts inside the actor's bounding box.
        let t = ray_actor_intersection(50.0, 0.0, 1.0, 0.0, 50.0, 0.0, 20.0);
        assert!(t.is_some(), "should hit when starting inside actor");
        let t = t.unwrap();
        assert!(t >= 0.0, "t should be >= 0");
    }

    // -----------------------------------------------------------------------
    // Diagonal ray tests
    // -----------------------------------------------------------------------

    #[test]
    fn diagonal_ray_hits_wall() {
        // Wall from (64, 0) to (64, 128), one-sided, in cell (0,0).
        let verts = vec![
            doom_map::Vertex { x: 64, y: 0 },
            doom_map::Vertex { x: 64, y: 128 },
        ];
        let sds = vec![make_sidedef(0)];
        let lds = vec![make_linedef_one_sided(0, 1, 0)];
        let secs = vec![make_sector(0, 128)];
        let cells = vec![vec![0u16]];
        let level = make_test_level(verts, lds, sds, secs, (0, 0), (1, 1), cells);

        // Ray from (0, 0) going at 45 degrees (northeast).
        let cos45 = std::f32::consts::FRAC_1_SQRT_2;
        let sin45 = std::f32::consts::FRAC_1_SQRT_2;
        let result = trace_ray(&level, 0, 0, cos45, sin45, 200.0, ActorCheck::Ignore);
        match &result.hit {
            TraceHit::Wall { linedef_index, .. } => {
                assert_eq!(*linedef_index, 0);
            }
            other => panic!("expected Wall hit on diagonal ray, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Negative direction rays
    // -----------------------------------------------------------------------

    #[test]
    fn ray_going_south_hits_wall() {
        // Wall at y=64 in cell (0,0).
        let level = make_wall_level();
        // Ray from (64, 128) going south (negative y).
        let result = trace_ray(&level, 64, 128, 0.0, -1.0, 200.0, ActorCheck::Ignore);
        match &result.hit {
            TraceHit::Wall { linedef_index, .. } => {
                assert_eq!(*linedef_index, 0);
            }
            other => panic!("expected Wall hit going south, got {other:?}"),
        }
    }

    #[test]
    fn ray_going_west_hits_vertical_wall() {
        // Vertical wall at x=32.
        let verts = vec![
            doom_map::Vertex { x: 32, y: 0 },
            doom_map::Vertex { x: 32, y: 128 },
        ];
        let sds = vec![make_sidedef(0)];
        let lds = vec![make_linedef_one_sided(0, 1, 0)];
        let secs = vec![make_sector(0, 128)];
        let cells = vec![vec![0u16]];
        let level = make_test_level(verts, lds, sds, secs, (0, 0), (1, 1), cells);

        // Ray from (100, 64) going west (-x).
        let result = trace_ray(&level, 100, 64, -1.0, 0.0, 200.0, ActorCheck::Ignore);
        match &result.hit {
            TraceHit::Wall { linedef_index, .. } => {
                assert_eq!(*linedef_index, 0);
            }
            other => panic!("expected Wall hit going west, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn very_long_ray_across_many_cells() {
        // 8x1 blockmap, wall in the last cell.
        let verts = vec![
            doom_map::Vertex { x: 900, y: 0 },
            doom_map::Vertex { x: 900, y: 128 },
        ];
        let sds = vec![make_sidedef(0)];
        let lds = vec![make_linedef_one_sided(0, 1, 0)];
        let secs = vec![make_sector(0, 128)];
        let mut cells = vec![vec![]; 8];
        cells[7] = vec![0u16]; // wall in cell 7

        let level = make_test_level(verts, lds, sds, secs, (0, 0), (8, 1), cells);

        let result = trace_ray(&level, 0, 64, 1.0, 0.0, 1024.0, ActorCheck::Ignore);
        match &result.hit {
            TraceHit::Wall { linedef_index, .. } => {
                assert_eq!(*linedef_index, 0);
            }
            other => panic!("expected Wall hit on long ray, got {other:?}"),
        }
    }

    #[test]
    fn ray_starting_outside_blockmap() {
        let level = make_wall_level(); // 2x2, origin (0,0)
        // Ray starts at (-100, 32), going east. Should eventually enter blockmap.
        let result = trace_ray(&level, -100, 32, 1.0, 0.0, 300.0, ActorCheck::Ignore);
        // The wall is at y=64, ray goes east at y=32 — should miss.
        assert!(
            matches!(result.hit, TraceHit::Nothing),
            "ray at y=32 going east should miss wall at y=64"
        );
    }

    #[test]
    fn check_actors_false_ignores_actors() {
        let level = make_wall_level();
        // ActorCheck::Ignore should skip actor testing even though one is in path.
        let result = trace_ray(&level, 64, 0, 0.0, 1.0, 200.0, ActorCheck::Ignore);
        // Should hit the wall at y=64 instead of the actor at y=32.
        match &result.hit {
            TraceHit::Wall { linedef_index, .. } => {
                assert_eq!(*linedef_index, 0);
            }
            other => panic!("expected Wall hit when actors disabled, got {other:?}"),
        }
    }
}
