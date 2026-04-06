//! Topological map analysis for spatial awareness.
//!
//! While [`crate::Level`] describes the exact geometric coordinates of walls and floors,
//! game logic (like monster pathfinding or sound propagation) rarely cares about
//! exact coordinates. Instead, it cares about *connectivity*: "Can I get from Room A to Room B?"
//!
//! The [`SectorGraph`] translates raw coordinate geometry into a clean graph of interconnected
//! rooms. It discovers portals (two-sided linedefs) and builds an adjacency list.

use crate::Level;
use std::collections::{HashMap, HashSet, VecDeque};

/// A topological graph representing the connectivity of sectors in a map.
///
/// Sectors are nodes, and two-sided linedefs acting as portals are edges.
/// This graph is entirely unweighted and undirected. If Sector A shares a two-sided
/// linedef with Sector B, they are considered adjacent.
///
/// # Examples
///
/// ```
/// use doom_map::{Level, lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex}};
/// use doom_map::graph::SectorGraph;
///
/// // Construct a minimal level with two connected sectors
/// let reject = Reject::parse_lump(&[], 0).unwrap();
/// let blockmap = Blockmap::parse_lump(&[0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
/// let level = Level {
///     name: "TEST".to_owned(),
///     things: vec![],
///     vertexes: vec![],
///     linedefs: vec![
///         // A two-sided linedef connecting Sector 0 and Sector 1
///         Linedef {
///             from_vertex: 0, to_vertex: 1, flags: 0x0004, special: 0, tag: 0,
///             right_sidedef: 0, left_sidedef: 1,
///         }
///     ],
///     sidedefs: vec![
///         Sidedef { x_offset: 0, y_offset: 0, upper_texture: *b"        ", lower_texture: *b"        ", middle_texture: *b"        ", sector: 0 },
///         Sidedef { x_offset: 0, y_offset: 0, upper_texture: *b"        ", lower_texture: *b"        ", middle_texture: *b"        ", sector: 1 },
///     ],
///     sectors: vec![
///         Sector { floor_height: 0, ceil_height: 128, floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT1\0\0\0", light_level: 192, special: 0, tag: 0 },
///         Sector { floor_height: 0, ceil_height: 128, floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT1\0\0\0", light_level: 192, special: 0, tag: 0 },
///     ],
///     segs: vec![], ssectors: vec![], nodes: vec![], reject, blockmap,
/// };
///
/// let graph = SectorGraph::build(&level);
/// assert!(graph.adjacency_list.get(&0).unwrap().contains(&1));
/// assert!(graph.adjacency_list.get(&1).unwrap().contains(&0));
/// ```
pub struct SectorGraph {
    /// Adjacency list: sector_index -> list of connected sector_indices
    pub adjacency_list: HashMap<usize, HashSet<usize>>,
}

impl SectorGraph {
    /// Scans a [`Level`] and builds a topological graph of its sectors.
    ///
    /// Connections are established exclusively by finding two-sided linedefs that connect
    /// one sector to another via their front and back sidedefs. One-sided linedefs
    /// are treated as solid walls and do not create connections.
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

    /// Finds the shortest topological path between two sectors.
    ///
    /// The path is represented as an ordered list of sector indices from start to end.
    /// This uses Breadth-First Search (BFS) to find the path with the minimum number
    /// of sector transitions.
    ///
    /// Returns `None` if there is no valid path (e.g., the map has isolated rooms).
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_map::{Level, lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex}};
    /// use doom_map::graph::SectorGraph;
    ///
    /// let reject = Reject::parse_lump(&[], 0).unwrap();
    /// let blockmap = Blockmap::parse_lump(&[0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
    /// let level = Level {
    ///     name: "TEST".to_owned(),
    ///     things: vec![], vertexes: vec![], segs: vec![], ssectors: vec![], nodes: vec![],
    ///     linedefs: vec![
    ///         Linedef { from_vertex: 0, to_vertex: 0, flags: 0x0004, special: 0, tag: 0, right_sidedef: 0, left_sidedef: 1 },
    ///         Linedef { from_vertex: 0, to_vertex: 0, flags: 0x0004, special: 0, tag: 0, right_sidedef: 1, left_sidedef: 2 },
    ///     ],
    ///     sidedefs: vec![
    ///         Sidedef { x_offset: 0, y_offset: 0, upper_texture: *b"        ", lower_texture: *b"        ", middle_texture: *b"        ", sector: 0 },
    ///         Sidedef { x_offset: 0, y_offset: 0, upper_texture: *b"        ", lower_texture: *b"        ", middle_texture: *b"        ", sector: 1 },
    ///         Sidedef { x_offset: 0, y_offset: 0, upper_texture: *b"        ", lower_texture: *b"        ", middle_texture: *b"        ", sector: 2 },
    ///     ],
    ///     sectors: vec![
    ///         Sector { floor_height: 0, ceil_height: 128, floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT1\0\0\0", light_level: 192, special: 0, tag: 0 },
    ///         Sector { floor_height: 0, ceil_height: 128, floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT1\0\0\0", light_level: 192, special: 0, tag: 0 },
    ///         Sector { floor_height: 0, ceil_height: 128, floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT1\0\0\0", light_level: 192, special: 0, tag: 0 },
    ///     ],
    ///     reject, blockmap,
    /// };
    ///
    /// let graph = SectorGraph::build(&level);
    /// let path = graph.shortest_path(0, 2).unwrap();
    /// assert_eq!(path, vec![0, 1, 2]);
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

    /// Exports the sector graph to the Graphviz `DOT` format for visualization.
    ///
    /// This is an incredible debugging tool! You can pipe the output into `dot -Tpng > graph.png`
    /// to get a visual flowchart of how rooms are connected in your level.
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
        let path = graph.shortest_path(0, 2).unwrap();
        assert_eq!(path, vec![0, 1, 2]);

        let dot = graph.to_dot();
        assert!(dot.contains("digraph SectorGraph"));
        assert!(dot.contains("0 -> 1"));
        assert!(dot.contains("1 -> 2"));
    }
}
