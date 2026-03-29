//! Export map geometry to a Graphviz DOT file.
//!
//! This module provides the `export_map_to_dot` function, which translates
//! the map's sector connectivity graph into a DOT format string.
//! Sectors are represented as nodes, and two-sided linedefs are represented as edges.

use crate::Level;
use crate::lumps::SIDEDEF_NONE;
use std::collections::HashSet;

/// Exports a `Level`'s sector connectivity graph to a Graphviz DOT string.
///
/// In the resulting graph:
/// - Each sector is a node.
/// - Each two-sided linedef that connects two different sectors is an edge.
/// - Parallel edges (multiple linedefs connecting the same two sectors) are drawn.
pub fn export_map_to_dot(level: &Level) -> String {
    let mut dot = String::new();
    dot.push_str(&format!(
        "// Doom Level {} Sector Connectivity\n",
        level.name
    ));
    dot.push_str("graph SectorConnectivity {\n");
    dot.push_str("    node [shape=circle];\n");

    // To keep track of which sectors we've mentioned
    let mut active_sectors = HashSet::new();
    let mut edges = Vec::new();

    for (ld_idx, ld) in level.linedefs.iter().enumerate() {
        if !ld.is_two_sided() || ld.right_sidedef == SIDEDEF_NONE || ld.left_sidedef == SIDEDEF_NONE
        {
            continue;
        }

        let right_side = &level.sidedefs[ld.right_sidedef as usize];
        let left_side = &level.sidedefs[ld.left_sidedef as usize];

        let s1 = right_side.sector as usize;
        let s2 = left_side.sector as usize;

        // Skip linedefs that connect a sector to itself
        if s1 == s2 {
            continue;
        }

        active_sectors.insert(s1);
        active_sectors.insert(s2);
        edges.push((s1, s2, ld_idx));
    }

    // Add node definitions for sectors that are connected to something
    let mut sorted_sectors: Vec<_> = active_sectors.into_iter().collect();
    sorted_sectors.sort_unstable();
    for sector_idx in sorted_sectors {
        dot.push_str(&format!(
            "    S{} [label=\"Sector {}\"];\n",
            sector_idx, sector_idx
        ));
    }

    // Add edges
    for (s1, s2, ld_idx) in edges {
        dot.push_str(&format!(
            "    S{} -- S{} [label=\"Linedef {}\"];\n",
            s1, s2, ld_idx
        ));
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
                    flags: 0x0004, // Two-sided
                    special: 0,
                    tag: 0,
                    right_sidedef: 1,
                    left_sidedef: 2,
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
                    sector: 0, // Belongs to sector 0
                },
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"WALL1\0\0\0",
                    lower_texture: *b"WALL2\0\0\0",
                    middle_texture: *b"WALL3\0\0\0",
                    sector: 1, // Belongs to sector 1
                },
            ],
            vertexes: vec![
                Vertex { x: 0, y: 0 },
                Vertex { x: 64, y: 0 },
                Vertex { x: 64, y: 64 },
            ],
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

        assert!(dot.contains("graph SectorConnectivity {"));
        assert!(dot.contains("node [shape=circle];"));
        assert!(dot.contains("S0 [label=\"Sector 0\"];"));
        assert!(dot.contains("S1 [label=\"Sector 1\"];"));
        assert!(dot.contains("S0 -- S1 [label=\"Linedef 1\"];"));
        assert!(dot.contains("}"));
    }
}
