//! Export map sector connectivity to a Graphviz DOT graph.
//!
//! This module provides the `export_map_to_dot` function, which translates
//! the 2D map layout's sector connectivity (via two-sided linedefs) into
//! a node-edge graph format suitable for rendering with Graphviz.

use crate::Level;
use crate::lumps::SIDEDEF_NONE;

/// Exports a `Level` to a Graphviz DOT string.
///
/// This function generates a directed graph where each node is a sector,
/// and edges represent connectivity between sectors via two-sided linedefs.
/// It is useful for visualizing the topological structure and flow of a map.
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
///             flags: 0x0004, // Two-sided
///             special: 0,
///             tag: 0,
///             right_sidedef: 0,
///             left_sidedef: 1,
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
///         Sidedef {
///             x_offset: 0,
///             y_offset: 0,
///             upper_texture: *b"WALL1\0\0\0",
///             lower_texture: *b"WALL2\0\0\0",
///             middle_texture: *b"WALL3\0\0\0",
///             sector: 1,
///         },
///     ],
///     sectors: vec![
///         Sector {
///             floor_height: 0,
///             ceil_height: 128,
///             floor_flat: *b"FLAT1\0\0\0",
///             ceil_flat: *b"FLAT2\0\0\0",
///             light_level: 192,
///             special: 0,
///             tag: 0,
///         },
///         Sector {
///             floor_height: 0,
///             ceil_height: 128,
///             floor_flat: *b"FLAT1\0\0\0",
///             ceil_flat: *b"FLAT2\0\0\0",
///             light_level: 192,
///             special: 0,
///             tag: 0,
///         }
///     ],
///     segs: vec![],
///     ssectors: vec![],
///     nodes: vec![],
///     reject,
///     blockmap,
/// };
///
/// let dot = doom_map::export_map_to_dot(&level);
/// assert!(dot.contains("digraph TEST {"));
/// assert!(dot.contains("sector_0 -> sector_1 [dir=none];"));
/// ```
pub fn export_map_to_dot(level: &Level) -> String {
    let mut dot = String::new();
    dot.push_str(&format!("digraph {} {{\n", level.name));
    dot.push_str("    node [shape=box, style=filled, fillcolor=lightgray];\n");

    // Track which edges we've already added to avoid duplicates
    let mut edges = std::collections::HashSet::new();

    for ld in &level.linedefs {
        if ld.is_two_sided() && ld.right_sidedef != SIDEDEF_NONE && ld.left_sidedef != SIDEDEF_NONE
        {
            let right_side = &level.sidedefs[ld.right_sidedef as usize];
            let left_side = &level.sidedefs[ld.left_sidedef as usize];

            let sec1 = right_side.sector;
            let sec2 = left_side.sector;

            // Only add edge if it connects two different sectors
            if sec1 != sec2 {
                // To keep the graph clean, we'll store edges as ordered pairs
                let edge = if sec1 < sec2 {
                    (sec1, sec2)
                } else {
                    (sec2, sec1)
                };

                if edges.insert(edge) {
                    dot.push_str(&format!(
                        "    sector_{} -> sector_{} [dir=none];\n",
                        edge.0, edge.1
                    ));
                }
            }
        }
    }

    dot.push_str("}\n");
    dot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex};

    fn make_test_level() -> Level {
        let reject = Reject::parse_lump(&[0u8], 1).unwrap();
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).unwrap();

        Level {
            name: "TEST".to_owned(),
            things: vec![],
            linedefs: vec![Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004, // Two-sided
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            }],
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
                    sector: 1,
                },
            ],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 64, y: 0 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap,
        }
    }

    #[test]
    fn test_export_dot() {
        let level = make_test_level();
        let dot = export_map_to_dot(&level);

        assert!(dot.contains("digraph TEST {"));
        assert!(dot.contains("sector_0 -> sector_1 [dir=none];"));
        assert!(dot.contains("}"));
    }
}
