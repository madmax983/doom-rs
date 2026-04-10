//! Tile grid: rasterizes a Doom `Level` onto a discrete grid for cogmind-mode rendering.
//!
//! Each grid cell is `CELL_SIZE` map units across.  Phase 1 classifies cells by
//! BSP sector lookup; Phase 2 rasterizes linedefs (walls, doors, height changes)
//! via Bresenham line drawing.

use doom_map::{FLAG_TWO_SIDED, Level, SIDEDEF_NONE, Ssector};

use super::glyphs::{Rgb, TileKind, sector_floor_kind};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Map units per grid cell.
pub(crate) const CELL_SIZE: i32 = 24;

// ---------------------------------------------------------------------------
// Tile
// ---------------------------------------------------------------------------

/// A single cell in the tile grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Tile {
    /// What kind of map element this cell represents.
    pub kind: TileKind,
    /// Index of the Doom sector that owns this cell, if any.
    pub sector_idx: Option<usize>,
    /// Light level (0-255) inherited from the sector.
    pub light: u8,
    /// Hazard glow tint from adjacent nukage/lava tiles, if any.
    pub glow: Option<Rgb>,
}

impl Default for Tile {
    fn default() -> Self {
        Self {
            kind: TileKind::Void,
            sector_idx: None,
            light: 0,
            glow: None,
        }
    }
}

// ---------------------------------------------------------------------------
// TileGrid
// ---------------------------------------------------------------------------

/// A 2-D grid of tiles covering the map bounding box.
pub(crate) struct TileGrid {
    tiles: Vec<Tile>,
    /// Width of the grid in cells.
    pub grid_w: usize,
    /// Height of the grid in cells.
    pub grid_h: usize,
    /// Map-unit X coordinate of the grid origin (lower-left corner).
    pub origin_x: i32,
    /// Map-unit Y coordinate of the grid origin (lower-left corner).
    pub origin_y: i32,
}

impl TileGrid {
    /// Build a tile grid from a loaded `Level`.
    ///
    /// - Phase 1: for each cell, BSP-lookup the sector and classify the floor.
    /// - Phase 2: rasterize every linedef onto the grid (walls, doors, height changes).
    #[must_use]
    pub(crate) fn from_level(level: &Level) -> Self {
        let (min_x, max_x, min_y, max_y) = vertex_bounds(level);

        // Pad by one cell on each side so edge geometry is captured.
        let origin_x = min_x - CELL_SIZE;
        let origin_y = min_y - CELL_SIZE;
        let grid_w = ((max_x - origin_x) / CELL_SIZE + 2) as usize;
        let grid_h = ((max_y - origin_y) / CELL_SIZE + 2) as usize;

        let mut tiles = vec![Tile::default(); grid_w * grid_h];
        let bsp = level.bsp();

        // ---- Phase 1: classify every cell by sector ----
        for gy in 0..grid_h {
            for gx in 0..grid_w {
                let map_x = origin_x + gx as i32 * CELL_SIZE + CELL_SIZE / 2;
                let map_y = origin_y + gy as i32 * CELL_SIZE + CELL_SIZE / 2;

                if let Some(ss) = bsp.point_in_subsector(map_x, map_y) {
                    if let Some(si) = subsector_sector(level, ss) {
                        if let Some(sector) = level.sectors.get(si) {
                            let kind = sector_floor_kind(sector.special);
                            let light = sector.light_level.clamp(0, 255) as u8;
                            tiles[gy * grid_w + gx] = Tile {
                                kind,
                                sector_idx: Some(si),
                                light,
                                glow: None,
                            };
                        }
                    }
                }
            }
        }

        // ---- Phase 2: rasterize linedefs ----
        for linedef in &level.linedefs {
            let v1 = match level.vertexes.get(linedef.from_vertex as usize) {
                Some(v) => v,
                None => continue,
            };
            let v2 = match level.vertexes.get(linedef.to_vertex as usize) {
                Some(v) => v,
                None => continue,
            };

            let cells = rasterize_line(
                i32::from(v1.x),
                i32::from(v1.y),
                i32::from(v2.x),
                i32::from(v2.y),
                origin_x,
                origin_y,
                CELL_SIZE,
                grid_w,
                grid_h,
            );

            let is_two_sided = linedef.flags & FLAG_TWO_SIDED != 0;

            if !is_two_sided {
                // One-sided line -> wall.
                for (gx, gy) in cells {
                    tiles[gy * grid_w + gx].kind = TileKind::Wall;
                }
            } else {
                // Two-sided line: check for door or height change.
                let kind = classify_two_sided(level, linedef);
                if kind != TileKind::Floor {
                    for (gx, gy) in cells {
                        // Don't overwrite walls.
                        if tiles[gy * grid_w + gx].kind != TileKind::Wall {
                            tiles[gy * grid_w + gx].kind = kind;
                        }
                    }
                }
            }
        }

        compute_hazard_glow(&mut tiles, grid_w, grid_h);

        Self {
            tiles,
            grid_w,
            grid_h,
            origin_x,
            origin_y,
        }
    }

    /// Bounds-checked tile access.
    #[must_use]
    pub(crate) fn get(&self, gx: usize, gy: usize) -> Option<&Tile> {
        if gx < self.grid_w && gy < self.grid_h {
            Some(&self.tiles[gy * self.grid_w + gx])
        } else {
            None
        }
    }

    /// Convert map coordinates to grid coordinates.
    #[must_use]
    pub(crate) fn map_to_grid(&self, map_x: i32, map_y: i32) -> (i32, i32) {
        (
            (map_x - self.origin_x) / CELL_SIZE,
            (map_y - self.origin_y) / CELL_SIZE,
        )
    }

    /// Return a 4-bit neighbor mask for wall box-drawing.
    ///
    /// Bit layout: 0 = north, 1 = east, 2 = south, 3 = west.
    /// A set bit means the neighbor in that direction is also a wall.
    #[must_use]
    pub(crate) fn wall_neighbors(&self, gx: usize, gy: usize) -> u8 {
        let mut mask: u8 = 0;
        // North (gy+1)
        if gy + 1 < self.grid_h && self.tiles[(gy + 1) * self.grid_w + gx].kind == TileKind::Wall {
            mask |= 0x01;
        }
        // East (gx+1)
        if gx + 1 < self.grid_w && self.tiles[gy * self.grid_w + gx + 1].kind == TileKind::Wall {
            mask |= 0x02;
        }
        // South (gy-1)
        if gy > 0 && self.tiles[(gy - 1) * self.grid_w + gx].kind == TileKind::Wall {
            mask |= 0x04;
        }
        // West (gx-1)
        if gx > 0 && self.tiles[gy * self.grid_w + gx - 1].kind == TileKind::Wall {
            mask |= 0x08;
        }
        mask
    }
}

// ---------------------------------------------------------------------------
// Hazard glow
// ---------------------------------------------------------------------------

/// Nukage glow color (green).
const NUKAGE_GLOW: Rgb = (0, 180, 0);
/// Lava glow color (orange-red).
const LAVA_GLOW: Rgb = (200, 80, 0);

fn compute_hazard_glow(tiles: &mut [Tile], grid_w: usize, grid_h: usize) {
    // Snapshot kinds to avoid aliasing.
    let kinds: Vec<TileKind> = tiles.iter().map(|t| t.kind).collect();
    for gy in 0..grid_h {
        for gx in 0..grid_w {
            let idx = gy * grid_w + gx;
            // Only floors/height-changes/open-doors can receive glow.
            if !matches!(
                kinds[idx],
                TileKind::Floor | TileKind::DoorOpen | TileKind::HeightChange
            ) {
                continue;
            }
            let neighbors = [
                if gy + 1 < grid_h {
                    Some((gy + 1) * grid_w + gx)
                } else {
                    None
                },
                if gx + 1 < grid_w {
                    Some(gy * grid_w + gx + 1)
                } else {
                    None
                },
                if gy > 0 {
                    Some((gy - 1) * grid_w + gx)
                } else {
                    None
                },
                if gx > 0 {
                    Some(gy * grid_w + gx - 1)
                } else {
                    None
                },
            ];
            for ni in neighbors.into_iter().flatten() {
                match kinds[ni] {
                    TileKind::Nukage => {
                        tiles[idx].glow = Some(NUKAGE_GLOW);
                        break;
                    }
                    TileKind::Lava => {
                        tiles[idx].glow = Some(LAVA_GLOW);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Bounding box of all vertices in the level.
fn vertex_bounds(level: &Level) -> (i32, i32, i32, i32) {
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for v in &level.vertexes {
        let x = i32::from(v.x);
        let y = i32::from(v.y);
        if x < min_x {
            min_x = x;
        }
        if x > max_x {
            max_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if y > max_y {
            max_y = y;
        }
    }
    // Fallback for empty vertex list.
    if min_x > max_x {
        return (0, 0, 0, 0);
    }
    (min_x, max_x, min_y, max_y)
}

/// Find the sector index that owns a subsector via its first seg's linedef/sidedef.
fn subsector_sector(level: &Level, ss: &Ssector) -> Option<usize> {
    let seg = level.segs.get(ss.first_seg as usize)?;
    let linedef = level.linedefs.get(seg.linedef as usize)?;
    let sidedef_idx = if seg.direction == 0 {
        linedef.right_sidedef
    } else {
        linedef.left_sidedef
    };
    if sidedef_idx == SIDEDEF_NONE {
        return None;
    }
    let sidedef = level.sidedefs.get(sidedef_idx as usize)?;
    Some(sidedef.sector as usize)
}

/// Classify a two-sided linedef as door, height-change, or plain floor.
fn classify_two_sided(level: &Level, linedef: &doom_map::Linedef) -> TileKind {
    let right_sector = linedef
        .right_sidedef
        .checked_sub(0) // always valid for right
        .and_then(|idx| level.sidedefs.get(idx as usize))
        .and_then(|sd| level.sectors.get(sd.sector as usize));
    let left_sector = if linedef.left_sidedef != SIDEDEF_NONE {
        level
            .sidedefs
            .get(linedef.left_sidedef as usize)
            .and_then(|sd| level.sectors.get(sd.sector as usize))
    } else {
        None
    };

    match (right_sector, left_sector) {
        (Some(r), Some(l)) => {
            // Door: either side has ceiling == floor.
            if r.ceil_height == r.floor_height || l.ceil_height == l.floor_height {
                TileKind::DoorClosed
            } else if r.floor_height != l.floor_height || r.ceil_height != l.ceil_height {
                TileKind::HeightChange
            } else {
                TileKind::Floor
            }
        }
        _ => TileKind::Floor,
    }
}

/// Rasterize a line segment onto the grid using Bresenham's algorithm.
///
/// Returns grid coordinates `(gx, gy)` for each cell the line crosses.
/// Out-of-bounds cells are excluded.
pub(crate) fn rasterize_line(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    origin_x: i32,
    origin_y: i32,
    cell_size: i32,
    grid_w: usize,
    grid_h: usize,
) -> Vec<(usize, usize)> {
    let gx1 = (x1 - origin_x) / cell_size;
    let gy1 = (y1 - origin_y) / cell_size;
    let gx2 = (x2 - origin_x) / cell_size;
    let gy2 = (y2 - origin_y) / cell_size;

    let mut result = Vec::new();

    let mut cx = gx1;
    let mut cy = gy1;
    let dx = (gx2 - gx1).abs();
    let dy = -(gy2 - gy1).abs();
    let sx: i32 = if gx1 < gx2 { 1 } else { -1 };
    let sy: i32 = if gy1 < gy2 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if cx >= 0 && cy >= 0 && (cx as usize) < grid_w && (cy as usize) < grid_h {
            result.push((cx as usize, cy as usize));
        }

        if cx == gx2 && cy == gy2 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }

    result
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterize_horizontal_line() {
        // Horizontal line from grid (0,2) to (5,2).
        let cells = rasterize_line(0, 48, 120, 48, 0, 0, CELL_SIZE, 10, 10);
        // Should include cells (0,2), (1,2), (2,2), (3,2), (4,2), (5,2).
        assert_eq!(cells.len(), 6);
        for (i, &(gx, gy)) in cells.iter().enumerate() {
            assert_eq!(gx, i);
            assert_eq!(gy, 2);
        }
    }

    #[test]
    fn rasterize_diagonal_line() {
        // Diagonal from grid (0,0) to (3,3).
        let cells = rasterize_line(0, 0, 72, 72, 0, 0, CELL_SIZE, 10, 10);
        // Bresenham on a perfect diagonal produces 4 cells.
        assert_eq!(cells.len(), 4);
        for (i, &(gx, gy)) in cells.iter().enumerate() {
            assert_eq!(gx, i, "gx mismatch at step {i}");
            assert_eq!(gy, i, "gy mismatch at step {i}");
        }
    }

    #[test]
    fn rasterize_clamps_to_grid() {
        // Line that starts outside the grid (negative coords) and enters it.
        let cells = rasterize_line(-48, 0, 48, 0, 0, 0, CELL_SIZE, 4, 4);
        // Grid coords: (-2, 0) to (2, 0). Only (0,0), (1,0), (2,0) are in bounds.
        let in_bounds: Vec<_> = cells.iter().filter(|&&(gx, gy)| gx < 4 && gy < 4).collect();
        assert!(!in_bounds.is_empty());
        for &(gx, gy) in &cells {
            assert!(gx < 4, "gx={gx} out of bounds");
            assert!(gy < 4, "gy={gy} out of bounds");
        }
    }

    #[test]
    fn tile_default_is_void() {
        let tile = Tile::default();
        assert_eq!(tile.kind, TileKind::Void);
        assert_eq!(tile.sector_idx, None);
        assert_eq!(tile.light, 0);
    }

    #[test]
    fn hazard_glow_adjacent_to_nukage() {
        let mut tiles = vec![Tile::default(); 4]; // 2x2
        tiles[0] = Tile {
            kind: TileKind::Floor,
            sector_idx: Some(0),
            light: 128,
            glow: None,
        };
        tiles[1] = Tile {
            kind: TileKind::Nukage,
            sector_idx: Some(1),
            light: 128,
            glow: None,
        };
        tiles[2] = Tile {
            kind: TileKind::Floor,
            sector_idx: Some(0),
            light: 128,
            glow: None,
        };
        tiles[3] = Tile {
            kind: TileKind::Floor,
            sector_idx: Some(0),
            light: 128,
            glow: None,
        };
        compute_hazard_glow(&mut tiles, 2, 2);
        assert!(tiles[0].glow.is_some());
        assert_eq!(tiles[0].glow.unwrap(), NUKAGE_GLOW);
        assert!(tiles[1].glow.is_none()); // nukage itself doesn't get glow
    }

    #[test]
    fn hazard_glow_not_on_walls() {
        let mut tiles = vec![Tile::default(); 4];
        tiles[0] = Tile {
            kind: TileKind::Wall,
            sector_idx: Some(0),
            light: 128,
            glow: None,
        };
        tiles[1] = Tile {
            kind: TileKind::Lava,
            sector_idx: Some(1),
            light: 128,
            glow: None,
        };
        tiles[2] = Tile::default();
        tiles[3] = Tile::default();
        compute_hazard_glow(&mut tiles, 2, 2);
        assert!(tiles[0].glow.is_none(), "walls should not receive glow");
    }

    #[test]
    fn hazard_glow_lava() {
        let mut tiles = vec![Tile::default(); 4];
        tiles[0] = Tile {
            kind: TileKind::Floor,
            sector_idx: Some(0),
            light: 128,
            glow: None,
        };
        tiles[1] = Tile {
            kind: TileKind::Lava,
            sector_idx: Some(1),
            light: 128,
            glow: None,
        };
        tiles[2] = Tile::default();
        tiles[3] = Tile::default();
        compute_hazard_glow(&mut tiles, 2, 2);
        assert_eq!(tiles[0].glow, Some(LAVA_GLOW));
    }

    #[test]
    fn tile_default_has_no_glow() {
        let tile = Tile::default();
        assert_eq!(tile.glow, None);
    }
}
