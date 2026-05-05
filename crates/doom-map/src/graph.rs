//! A topological graph representing the connectivity of sectors in a map.
//!
//! This module provides a `SectorGraph` which builds an adjacency list of sectors,
//! primarily useful for pathfinding, topological sorting, and mapping sector relationships.
//! It establishes connections by finding two-sided linedefs that connect one sector
//! to another via their front and back sidedefs.

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
    ///
    /// The map geometry contains nodes called [`crate::lumps::Sector`]s and edges called [`crate::lumps::Linedef`]s.
    /// This method identifies connections between sectors by finding two-sided linedefs
    /// that connect one sector to another via their front and back sidedefs.
    ///
    /// By building this graph, we transform geometry data into a pathable network
    /// where enemies can navigate through doorways (portals) using BFS.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_map::{Level, lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex}};
    /// use doom_map::graph::SectorGraph;
    ///
    /// // 1. Construct a minimal Level with 2 connected sectors
    /// let level = Level {
    ///     name: "TEST".to_owned(),
    ///     things: vec![],
    ///     vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 64, y: 0 }],
    ///     // A two-sided linedef acting as a portal between Sector 0 and Sector 1
    ///     linedefs: vec![
    ///         Linedef {
    ///             from_vertex: 0,
    ///             to_vertex: 1,
    ///             flags: 0x0004, // Two-sided flag is required to traverse
    ///             special: 0,
    ///             tag: 0,
    ///             right_sidedef: 0,
    ///             left_sidedef: 1,
    ///         },
    ///     ],
    ///     sidedefs: vec![
    ///         Sidedef { x_offset: 0, y_offset: 0, upper_texture: *b"WALL1\0\0\0", lower_texture: *b"WALL2\0\0\0", middle_texture: *b"WALL3\0\0\0", sector: 0 },
    ///         Sidedef { x_offset: 0, y_offset: 0, upper_texture: *b"WALL1\0\0\0", lower_texture: *b"WALL2\0\0\0", middle_texture: *b"WALL3\0\0\0", sector: 1 },
    ///     ],
    ///     sectors: vec![
    ///         Sector { floor_height: 0, ceil_height: 128, floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT2\0\0\0", light_level: 192, special: 0, tag: 0 },
    ///         Sector { floor_height: 0, ceil_height: 128, floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT2\0\0\0", light_level: 192, special: 0, tag: 0 },
    ///     ],
    ///     segs: vec![], ssectors: vec![], nodes: vec![],
    ///     reject: Reject::parse_lump(&[0u8], 1).unwrap(),
    ///     // Blockmap is required for a complete Level struct
    ///     blockmap: Blockmap::parse_lump(&{
    ///         let mut data = vec![0u8; 14];
    ///         data[4..6].copy_from_slice(&1u16.to_le_bytes());
    ///         data[6..8].copy_from_slice(&1u16.to_le_bytes());
    ///         data[8..10].copy_from_slice(&5u16.to_le_bytes());
    ///         data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
    ///         data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
    ///         data
    ///     }).unwrap(),
    /// };
    ///
    /// // 2. Build the adjacency graph
    /// let graph = SectorGraph::build(&level);
    ///
    /// // 3. Verify Sector 0 and 1 are bidirectionally connected
    /// assert_eq!(graph.adjacency_list.len(), 2);
    /// assert!(graph.adjacency_list.get(&0).unwrap().contains(&1));
    /// assert!(graph.adjacency_list.get(&1).unwrap().contains(&0));
    /// ```
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
    ///
    /// Why BFS? Because topological distances are unweighted, BFS guarantees
    /// we find the route with the fewest sector transitions. This path does not
    /// consider physical distance or line-of-sight constraints, but helps find
    /// a general connectivity path through the level.
    ///
    /// ## Returns
    /// An `Option<Vec<usize>>` containing an ordered list of sector indices from
    /// the start sector to the end sector. Returns `None` if no path exists.
    ///
    /// ## Examples
    ///
    /// ```
    /// use std::collections::{HashMap, HashSet};
    /// use doom_map::graph::SectorGraph;
    ///
    /// // 1. Set up a simple hallway of sectors: 0 <-> 1 <-> 2 <-> 3
    /// let mut adjacency_list = HashMap::new();
    /// adjacency_list.insert(0, HashSet::from([1]));
    /// adjacency_list.insert(1, HashSet::from([0, 2]));
    /// adjacency_list.insert(2, HashSet::from([1, 3]));
    /// adjacency_list.insert(3, HashSet::from([2]));
    ///
    /// let graph = SectorGraph { adjacency_list };
    ///
    /// // 2. Traverse the path
    /// let path = graph.shortest_path(0, 3).unwrap();
    /// assert_eq!(path, vec![0, 1, 2, 3]);
    ///
    /// // Searching for the path to the same sector is an immediate return
    /// assert_eq!(graph.shortest_path(1, 1).unwrap(), vec![1]);
    ///
    /// // Completely disconnected sectors will return None
    /// assert!(graph.shortest_path(0, 4).is_none());
    /// ```
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
    ///
    /// Graphviz is an excellent tool for visualizing complex spatial connectivity.
    /// This method translates the bidirectional adjacency map into an undirected
    /// topological representation for visual map analysis, helping human cartographers
    /// find isolated areas and chokepoints visually.
    ///
    /// ## Examples
    ///
    /// ```
    /// use std::collections::{HashMap, HashSet};
    /// use doom_map::graph::SectorGraph;
    ///
    /// // 1. Create a graph where 0 and 1 are connected, but 2 is isolated
    /// let mut adjacency_list = HashMap::new();
    /// adjacency_list.insert(0, HashSet::from([1]));
    /// adjacency_list.insert(1, HashSet::from([0]));
    /// adjacency_list.insert(2, HashSet::new());
    ///
    /// let graph = SectorGraph { adjacency_list };
    ///
    /// // 2. Export to DOT string format
    /// let dot = graph.to_dot();
    ///
    /// // 3. The export includes Graphviz headers and edge definitions
    /// assert!(dot.contains("digraph SectorGraph {"));
    /// assert!(dot.contains("0 -> 1;")); // The edge is correctly added
    /// assert!(dot.contains("2;"));      // Isolated nodes are still rendered
    /// ```
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
