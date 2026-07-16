//! A topological graph representing the connectivity of sectors in a map.
//!
//! This module provides a `SectorGraph` which builds an adjacency list of sectors,
//! primarily useful for pathfinding, topological sorting, and mapping sector relationships.
//!
//! # The Story
//!
//! In the Doom engine, levels are not continuous 3D meshes but rather 2D plans carved into discrete
//! regions called `Sector`s. However, sectors don't inherently know who their neighbors are. To determine
//! if a monster in one sector can hear a sound or walk into an adjacent sector, we must traverse the map
//! topologically.
//!
//! The `SectorGraph` solves this by tracing the boundary lines (`Linedef`s) of every sector. When a
//! boundary line is two-sided (meaning it has both a front and back `Sidedef`), it acts as a portal connecting
//! the two sectors that own those sidedefs. By scanning the entire map and linking these portals, we build
//! an adjacency list—a web representing how every room connects to the rest of the level.
//!
//! # Examples
//!
//! To build a graph and query the shortest topological path (minimum sector transitions) between two sectors:
//!
//! ```rust,no_run
//! use doom_map::{graph::SectorGraph, Level};
//!
//! // Assuming we have loaded a parsed level `level`...
//! # let level: Level = unimplemented!();
//!
//! // 1. Build the topological web of sectors
//! let graph = SectorGraph::build(&level);
//!
//! // 2. Find the shortest number of rooms to traverse from sector 0 to sector 2
//! if let Some(path) = graph.shortest_path(0, 2) {
//!     println!("Path found! Traversing {} sectors.", path.len());
//! } else {
//!     println!("No connection exists between the sectors.");
//! }
//! ```

use crate::Level;
use std::collections::{HashMap, HashSet, VecDeque};

/// A topological graph representing the connectivity of sectors in a map.
/// Sectors are nodes, and two-sided linedefs acting as portals are edges.
pub struct SectorGraph {
    /// Adjacency list: sector_index -> list of connected sector_indices
    pub adjacency_list: HashMap<usize, HashSet<usize>>,
}

impl SectorGraph {
    /// Builds a topological graph of sectors from the given Level.
    /// Connections are established by finding two-sided linedefs that connect
    /// one sector to another via their front and back sidedefs.
    #[must_use]
    pub fn build(level: &Level) -> Self {
        let mut adjacency_list: HashMap<usize, HashSet<usize>> = HashMap::new();

        // Initialize empty sets for all sectors
        for i in 0..level.sectors.len() {
            adjacency_list.insert(i, HashSet::new());
        }

        for ld in &level.linedefs {
            if ld.is_two_sided() {
                if let (Some(right_sd), Some(left_sd)) = (
                    level.sidedefs.get(ld.right_sidedef as usize),
                    level.sidedefs.get(ld.left_sidedef as usize),
                ) {
                    let s1 = right_sd.sector as usize;
                    let s2 = left_sd.sector as usize;

                    if s1 != s2 {
                        if let Some(edges) = adjacency_list.get_mut(&s1) {
                            edges.insert(s2);
                        }
                        if let Some(edges) = adjacency_list.get_mut(&s2) {
                            edges.insert(s1);
                        }
                    }
                }
            }
        }

        Self { adjacency_list }
    }

    /// Finds the shortest topological path (minimum number of sector transitions)
    /// between two sectors using Breadth-First Search (BFS).
    #[must_use]
    pub fn shortest_path(&self, start_sector: usize, end_sector: usize) -> Option<Vec<usize>> {
        if start_sector == end_sector {
            return Some(vec![start_sector]);
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent_map = HashMap::new();

        queue.push_back(start_sector);
        visited.insert(start_sector);

        while let Some(current) = queue.pop_front() {
            if current == end_sector {
                // Reconstruct path
                let mut path = vec![current];
                let mut node = current;
                while let Some(&parent) = parent_map.get(&node) {
                    path.push(parent);
                    node = parent;
                }
                path.reverse();
                return Some(path);
            }

            if let Some(neighbors) = self.adjacency_list.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        parent_map.insert(neighbor, current);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        None
    }

    /// Exports the sector graph to the Graphviz DOT format for visualization.
    #[must_use]
    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph SectorGraph {\n");
        dot.push_str("    node [shape=circle, style=filled, fillcolor=lightblue];\n");

        for (&node, neighbors) in &self.adjacency_list {
            if neighbors.is_empty() {
                dot.push_str(&format!("    {};\n", node));
            } else {
                for &neighbor in neighbors {
                    // To avoid duplicating undirected edges in DOT, we only add the edge
                    // if the source node index is less than the target node index.
                    if node < neighbor {
                        dot.push_str(&format!("    {} -> {};\n", node, neighbor));
                    }
                }
            }
        }

        dot.push_str("}\n");
        dot
    }
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
            linedefs: vec![
                Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0x0004, // Two-sided
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
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
                    sector: 1,
                },
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"WALL1\0\0\0",
                    lower_texture: *b"WALL2\0\0\0",
                    middle_texture: *b"WALL3\0\0\0",
                    sector: 2,
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
                    floor_height: doom_types::Fixed16_16::from_int(0),
                    ceil_height: doom_types::Fixed16_16::from_int(128),
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                Sector {
                    floor_height: doom_types::Fixed16_16::from_int(0),
                    ceil_height: doom_types::Fixed16_16::from_int(128),
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                Sector {
                    floor_height: doom_types::Fixed16_16::from_int(0),
                    ceil_height: doom_types::Fixed16_16::from_int(128),
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
    fn test_sector_graph() {
        let level = make_test_level();
        let graph = SectorGraph::build(&level);

        // Path from 0 to 2 should be [0, 1, 2]
        let path = graph.shortest_path(0, 2).expect("value must exist in test");
        assert_eq!(path, vec![0, 1, 2]);

        let dot = graph.to_dot();
        assert!(dot.contains("digraph SectorGraph"));
        assert!(dot.contains("0 -> 1"));
        assert!(dot.contains("1 -> 2"));
    }
}
