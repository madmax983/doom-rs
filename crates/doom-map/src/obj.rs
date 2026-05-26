//! Export map geometry to a Wavefront OBJ 3D model.
//!
//! This module provides the `export_map_to_obj` function, which translates
//! the 2D map layout and sector heights into a 3D mesh format.

use crate::Level;
use crate::lumps::SIDEDEF_NONE;

/// Exports a `Level` to a Wavefront OBJ string containing vertical walls.
///
/// This generates a 3D mesh of the level's walls (one-sided walls, plus
/// the upper and lower steps of two-sided walls). It maps Doom's (X, Y)
/// coordinates and Z heights into the standard 3D coordinate space used
/// by OBJ (where Y is typically up).
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
///         floor_height: 0,
///         ceil_height: 128,
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
/// let obj = doom_map::export_map_to_obj(&level);
/// assert!(obj.contains("v 0 0 0"));
/// assert!(obj.contains("f 1 2 3 4"));
/// ```
pub fn export_map_to_obj(level: &Level) -> String {
    use std::fmt::Write;

    let capacity = 100 + level.linedefs.len() * 150;
    let mut obj = String::with_capacity(capacity);
    let _ = writeln!(obj, "# Doom Level exported by doom-rs");
    let _ = writeln!(obj, "o {}", level.name);

    let mut vertex_count = 1;

    for ld in &level.linedefs {
        let v1 = &level.vertexes[ld.from_vertex as usize];
        let v2 = &level.vertexes[ld.to_vertex as usize];

        if ld.right_sidedef == SIDEDEF_NONE {
            continue;
        }

        let right_side = &level.sidedefs[ld.right_sidedef as usize];
        let front_sector = &level.sectors[right_side.sector as usize];

        // Instead of allocating a Vec<()>, we use a fixed size array
        // A linedef can generate at most 3 vertical quads (lower, middle, upper)
        let mut quads = [(0i16, 0i16); 3];
        let mut quads_len = 0;

        if ld.left_sidedef == SIDEDEF_NONE {
            quads[quads_len] = (front_sector.floor_height, front_sector.ceil_height);
            quads_len += 1;
        } else {
            let left_side = &level.sidedefs[ld.left_sidedef as usize];
            let back_sector = &level.sectors[left_side.sector as usize];

            if front_sector.floor_height < back_sector.floor_height {
                quads[quads_len] = (front_sector.floor_height, back_sector.floor_height);
                quads_len += 1;
            } else if back_sector.floor_height < front_sector.floor_height {
                quads[quads_len] = (back_sector.floor_height, front_sector.floor_height);
                quads_len += 1;
            }

            if front_sector.ceil_height > back_sector.ceil_height {
                quads[quads_len] = (back_sector.ceil_height, front_sector.ceil_height);
                quads_len += 1;
            } else if back_sector.ceil_height > front_sector.ceil_height {
                quads[quads_len] = (front_sector.ceil_height, back_sector.ceil_height);
                quads_len += 1;
            }
        }

        for &(z_bottom, z_top) in quads.iter().take(quads_len) {
            if z_bottom >= z_top {
                continue;
            }

            // Doom coords: X is East/West, Y is North/South.
            // 3D coords: X = X, Y = Up (Doom Z), Z = -Doom Y
            let _ = writeln!(obj, "v {} {} {}", v1.x, z_bottom, -v1.y);
            let _ = writeln!(obj, "v {} {} {}", v2.x, z_bottom, -v2.y);
            let _ = writeln!(obj, "v {} {} {}", v2.x, z_top, -v2.y);
            let _ = writeln!(obj, "v {} {} {}", v1.x, z_top, -v1.y);

            let v_start = vertex_count;
            let _ = writeln!(
                obj,
                "f {} {} {} {}",
                v_start,
                v_start + 1,
                v_start + 2,
                v_start + 3
            );
            vertex_count += 4;
        }
    }

    obj
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
    fn test_export_obj() {
        let level = make_test_level();
        let obj = export_map_to_obj(&level);

        assert!(obj.contains("o TEST"));
        assert!(obj.contains("v 0 0 0"));
        assert!(obj.contains("v 64 0 0"));
        assert!(obj.contains("v 64 128 0"));
        assert!(obj.contains("v 0 128 0"));
        assert!(obj.contains("f 1 2 3 4"));
    }
}
