//! Export map geometry to an ASCII STL 3D model.
//!
//! This module provides the `export_map_to_stl` function, which translates
//! the 2D map layout and sector heights into a 3D mesh format for 3D printing or rendering.

use crate::Level;
use crate::lumps::SIDEDEF_NONE;

/// Exports a `Level` to an ASCII STL string containing vertical walls.
///
/// This generates a 3D mesh of the level's walls (one-sided walls, plus
/// the upper and lower steps of two-sided walls). It maps Doom's (X, Y)
/// coordinates and Z heights into the standard 3D coordinate space.
///
/// # Examples
///
/// ```
/// use doom_map::{Level, lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex}};
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
///         Vertex { x: 64, y: 0 },
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
///     sidedefs: vec![
///         Sidedef {
///             x_offset: 0,
///             y_offset: 0,
///             upper_texture: *b"WALL1\0\0\0",
///             lower_texture: *b"WALL2\0\0\0",
///             middle_texture: *b"WALL3\0\0\0",
///             sector: 0,
///         },
///     ],
///     sectors: vec![Sector {
///         floor_height: doom_types::Fixed16_16::from_int(0),
///         ceil_height: doom_types::Fixed16_16::from_int(128),
///         floor_flat: *b"FLAT1\0\0\0",
///         ceil_flat: *b"FLAT2\0\0\0",
///         light_level: 192,
///         special: 0,
///         tag: 0,
///     }],
///     segs: vec![],
///     ssectors: vec![],
///     nodes: vec![],
///     reject,
///     blockmap,
/// };
///
/// let stl = doom_map::export_map_to_stl(&level);
/// assert!(stl.contains("solid TEST"));
/// assert!(stl.contains("endsolid TEST"));
/// assert!(stl.contains("vertex 0.000000 0.000000 0.000000"));
/// assert!(stl.contains("vertex 64.000000 0.000000 0.000000"));
/// assert!(stl.contains("vertex 64.000000 128.000000 0.000000"));
/// assert!(stl.contains("vertex 0.000000 128.000000 0.000000"));
/// ```
pub fn export_map_to_stl(level: &Level) -> String {
    let mut stl = String::new();
    stl.push_str(&format!("solid {}\n", level.name));

    for ld in &level.linedefs {
        let v1 = &level.vertexes[ld.from_vertex as usize];
        let v2 = &level.vertexes[ld.to_vertex as usize];

        if ld.right_sidedef == SIDEDEF_NONE {
            continue;
        }

        let right_side = &level.sidedefs[ld.right_sidedef as usize];
        let front_sector = &level.sectors[right_side.sector as usize];

        let mut quads = Vec::new();

        if ld.left_sidedef == SIDEDEF_NONE {
            quads.push((front_sector.floor_height, front_sector.ceil_height, false));
        } else {
            let left_side = &level.sidedefs[ld.left_sidedef as usize];
            let back_sector = &level.sectors[left_side.sector as usize];

            if front_sector.floor_height < back_sector.floor_height {
                quads.push((front_sector.floor_height, back_sector.floor_height, false));
            } else if back_sector.floor_height < front_sector.floor_height {
                quads.push((back_sector.floor_height, front_sector.floor_height, true));
            }

            if front_sector.ceil_height > back_sector.ceil_height {
                quads.push((back_sector.ceil_height, front_sector.ceil_height, false));
            } else if back_sector.ceil_height > front_sector.ceil_height {
                quads.push((front_sector.ceil_height, back_sector.ceil_height, true));
            }
        }

        for (z_bottom, z_top, inverted) in quads {
            if z_bottom >= z_top {
                continue;
            }

            let z_bottom = z_bottom.to_int() as f32;
            let z_top = z_top.to_int() as f32;

            let x1 = v1.x as f32;
            let y1 = -v1.y as f32;
            let x2 = v2.x as f32;
            let y2 = -v2.y as f32;

            let dx = x2 - x1;
            let dy = y2 - y1;
            let nx = -dy;
            let ny = 0.0;
            let nz = dx;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            let (mut nx, mut ny, mut nz) = if len > 0.0 {
                (nx / len, ny / len, nz / len)
            } else {
                (0.0, 0.0, 0.0)
            };

            if inverted {
                nx = -nx;
                ny = -ny;
                nz = -nz;
            }

            let (p1, p2, p3, p4) = if inverted {
                // If inverted, swap winding order so the normal faces the back sector.
                ((x2, z_bottom, y2), (x1, z_bottom, y1), (x1, z_top, y1), (x2, z_top, y2))
            } else {
                ((x1, z_bottom, y1), (x2, z_bottom, y2), (x2, z_top, y2), (x1, z_top, y1))
            };

            // Triangle 1
            stl.push_str(&format!("  facet normal {:.6} {:.6} {:.6}\n", nx, ny, nz));
            stl.push_str("    outer loop\n");
            stl.push_str(&format!(
                "      vertex {:.6} {:.6} {:.6}\n",
                p1.0, p1.1, p1.2
            ));
            stl.push_str(&format!(
                "      vertex {:.6} {:.6} {:.6}\n",
                p2.0, p2.1, p2.2
            ));
            stl.push_str(&format!("      vertex {:.6} {:.6} {:.6}\n", p3.0, p3.1, p3.2));
            stl.push_str("    endloop\n");
            stl.push_str("  endfacet\n");

            // Triangle 2
            stl.push_str(&format!("  facet normal {:.6} {:.6} {:.6}\n", nx, ny, nz));
            stl.push_str("    outer loop\n");
            stl.push_str(&format!(
                "      vertex {:.6} {:.6} {:.6}\n",
                p1.0, p1.1, p1.2
            ));
            stl.push_str(&format!("      vertex {:.6} {:.6} {:.6}\n", p3.0, p3.1, p3.2));
            stl.push_str(&format!("      vertex {:.6} {:.6} {:.6}\n", p4.0, p4.1, p4.2));
            stl.push_str("    endloop\n");
            stl.push_str("  endfacet\n");
        }
    }

    stl.push_str(&format!("endsolid {}\n", level.name));
    stl
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex};

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
            things: vec![],
            linedefs: vec![Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            }],
            sidedefs: vec![Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"WALL1\0\0\0",
                lower_texture: *b"WALL2\0\0\0",
                middle_texture: *b"WALL3\0\0\0",
                sector: 0,
            }],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 64, y: 0 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![Sector {
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

    #[test]
    fn test_export_stl() {
        let level = make_test_level();
        let stl = export_map_to_stl(&level);

        assert!(stl.contains("solid TEST"));
        assert!(stl.contains("endsolid TEST"));
        assert!(stl.contains("vertex 0.000000 0.000000 0.000000"));
        assert!(stl.contains("vertex 64.000000 0.000000 0.000000"));
        assert!(stl.contains("vertex 64.000000 128.000000 0.000000"));
        assert!(stl.contains("vertex 0.000000 128.000000 0.000000"));
    }
}
