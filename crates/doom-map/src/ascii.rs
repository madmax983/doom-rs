//! Export map geometry to an ASCII art grid.
//!
//! This module provides the `export_map_to_ascii` function, which rasterizes
//! the 2D map layout into a text-based character grid. This is useful for
//! quick terminal debugging, logging map generation output, or providing
//! ultra-minimalistic previews without an external SVG viewer.

use crate::Level;

/// Rasterizes a `Level` to an ASCII art string.
///
/// Scales the map's geometry to fit within the requested `width` and `height` grid.
/// One-sided walls (solid) are drawn as `#`.
/// Two-sided walls (open/portals) are drawn as `.`.
/// Things are drawn as `@`.
///
/// # Examples
///
/// ```
/// use doom_map::{Level, Blockmap, Linedef, Reject, Sector, Sidedef, Thing, Vertex};
///
/// let reject = Reject::parse_lump(&[0u8], 1).unwrap();
/// let mut bm_data = vec![0u8; 14];
/// bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
/// bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
/// bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
/// bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
/// bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
/// let blockmap = Blockmap::parse_lump(&bm_data).unwrap();
///
/// let level = Level {
///     name: "TEST".to_owned(),
///     things: vec![],
///     vertexes: vec![
///         Vertex { x: 0, y: 0 },
///         Vertex { x: 100, y: 100 },
///     ],
///     linedefs: vec![
///         Linedef {
///             from_vertex: 0,
///             to_vertex: 1,
///             flags: 0,
///             special: 0,
///             tag: 0,
///             right_sidedef: 0,
///             left_sidedef: 0xFFFF,
///         },
///     ],
///     sidedefs: vec![Sidedef {
///         x_offset: 0, y_offset: 0,
///         upper_texture: *b"WALL1\0\0\0", lower_texture: *b"WALL2\0\0\0", middle_texture: *b"WALL3\0\0\0",
///         sector: 0,
///     }],
///     sectors: vec![Sector {
///         floor_height: 0, ceil_height: 128,
///         floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT2\0\0\0",
///         light_level: 192, special: 0, tag: 0,
///     }],
///     segs: vec![], ssectors: vec![], nodes: vec![], reject, blockmap,
/// };
///
/// let ascii = doom_map::export_map_to_ascii(&level, 10, 10);
/// assert!(ascii.contains('#'));
/// ```
pub fn export_map_to_ascii(level: &Level, width: usize, height: usize) -> String {
    if width == 0 || height == 0 || level.vertexes.is_empty() {
        return String::new();
    }

    let mut min_x = i16::MAX;
    let mut max_x = i16::MIN;
    let mut min_y = i16::MAX;
    let mut max_y = i16::MIN;

    for v in &level.vertexes {
        min_x = min_x.min(v.x);
        max_x = max_x.max(v.x);
        min_y = min_y.min(v.y);
        max_y = max_y.max(v.y);
    }

    // Add small padding to prevent OOB
    let range_x = (max_x as f32 - min_x as f32).max(1.0);
    let range_y = (max_y as f32 - min_y as f32).max(1.0);

    let mut grid = vec![vec![' '; width]; height];

    let to_grid = |x: i16, y: i16| -> (i32, i32) {
        let px = ((x as f32 - min_x as f32) / range_x * (width as f32 - 1.0)) as i32;
        // Flip Y since terminal Y is down
        let py = height as i32
            - 1
            - ((y as f32 - min_y as f32) / range_y * (height as f32 - 1.0)) as i32;
        (
            px.clamp(0, width as i32 - 1),
            py.clamp(0, height as i32 - 1),
        )
    };

    // Bresenham's line algorithm
    let mut draw_line = |x0: i32, y0: i32, x1: i32, y1: i32, ch: char| {
        let mut x = x0;
        let mut y = y0;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
                // Do not overwrite solid walls with open walls
                if grid[y as usize][x as usize] != '#' {
                    grid[y as usize][x as usize] = ch;
                }
            }
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    };

    // Draw linedefs
    for ld in &level.linedefs {
        let v1 = &level.vertexes[ld.from_vertex as usize];
        let v2 = &level.vertexes[ld.to_vertex as usize];
        let (x0, y0) = to_grid(v1.x, v1.y);
        let (x1, y1) = to_grid(v2.x, v2.y);

        let ch = if ld.is_two_sided() { '.' } else { '#' };
        draw_line(x0, y0, x1, y1, ch);
    }

    // Draw things
    for thing in &level.things {
        let (x, y) = to_grid(thing.x, thing.y);
        if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
            grid[y as usize][x as usize] = '@';
        }
    }

    let mut output = String::with_capacity(width * height + height);
    for row in grid {
        let s: String = row.into_iter().collect();
        output.push_str(&s);
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Thing, Vertex};

    fn make_test_level() -> Level {
        let reject = Reject::parse_lump(&[0u8], 1).expect("value must exist in test");
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("value must exist in test");

        Level {
            name: "TEST".to_owned(),
            things: vec![Thing {
                x: 50,
                y: 50,
                angle: 0,
                kind: 1,
                flags: 0,
            }],
            linedefs: vec![
                Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 0xFFFF,
                },
                Linedef {
                    from_vertex: 1,
                    to_vertex: 2,
                    flags: 0x0004,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
            ],
            sidedefs: vec![
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"WALL1\0\0\0",
                    lower_texture: *b"WALL2\0\0\0",
                    middle_texture: *b"WALL3\0\0\0",
                    sector: 0,
                },
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"WALL1\0\0\0",
                    lower_texture: *b"WALL2\0\0\0",
                    middle_texture: *b"WALL3\0\0\0",
                    sector: 0,
                },
            ],
            vertexes: vec![
                Vertex { x: 0, y: 0 },
                Vertex { x: 100, y: 0 },
                Vertex { x: 100, y: 100 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![Sector {
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

    #[test]
    fn test_export_ascii() {
        let level = make_test_level();
        let ascii = export_map_to_ascii(&level, 10, 10);
        assert!(ascii.contains('#'));
        assert!(ascii.contains('.'));
        assert!(ascii.contains('@'));
    }
}
