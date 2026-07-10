//! Tactical analysis tools for map graphs.

use crate::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

/// Provides tactical analysis capabilities for a graph of connected map regions.
pub trait TacticalGraph {
    /// Finds "chokepoints" (articulation points) in the map.
    /// A chokepoint is a region whose removal increases the number of connected
    /// components, effectively splitting the map or a section of it.
    fn find_chokepoints(&self) -> HashSet<usize>;
}

impl TacticalGraph for SectorGraph {
    fn find_chokepoints(&self) -> HashSet<usize> {
        let mut chokepoints = HashSet::new();
        let mut visited = HashSet::new();
        let mut discovery_time = HashMap::new();
        let mut low_time = HashMap::new();
        let mut parent_map = HashMap::new();
        let mut time = 0;

        for &sector in self.adjacency_list.keys() {
            if !visited.contains(&sector) {
                dfs_articulation(
                    self,
                    sector,
                    &mut visited,
                    &mut discovery_time,
                    &mut low_time,
                    &mut parent_map,
                    &mut chokepoints,
                    &mut time,
                );
            }
        }

        chokepoints
    }
}

#[allow(clippy::too_many_arguments)]
fn dfs_articulation(
    graph: &SectorGraph,
    u: usize,
    visited: &mut HashSet<usize>,
    discovery_time: &mut HashMap<usize, usize>,
    low_time: &mut HashMap<usize, usize>,
    parent_map: &mut HashMap<usize, usize>,
    chokepoints: &mut HashSet<usize>,
    time: &mut usize,
) {
    visited.insert(u);
    *time += 1;
    discovery_time.insert(u, *time);
    low_time.insert(u, *time);

    let mut children = 0;

    if let Some(neighbors) = graph.adjacency_list.get(&u) {
        for &v in neighbors {
            if !visited.contains(&v) {
                children += 1;
                parent_map.insert(v, u);

                dfs_articulation(
                    graph,
                    v,
                    visited,
                    discovery_time,
                    low_time,
                    parent_map,
                    chokepoints,
                    time,
                );

                // Update low value of u for function calls.
                let low_v = *low_time.get(&v).unwrap_or(&usize::MAX);
                let low_u = low_time.get_mut(&u).expect("must exist");
                *low_u = std::cmp::min(*low_u, low_v);

                // u is an articulation point in following cases:
                // (1) u is root of DFS tree and has two or more children.
                let parent_u = parent_map.get(&u);
                if parent_u.is_none() && children > 1 {
                    chokepoints.insert(u);
                }

                // (2) If u is not root and low value of one of its child is more
                // than discovery value of u.
                if parent_u.is_some() && low_v >= *discovery_time.get(&u).unwrap() {
                    chokepoints.insert(u);
                }
            } else if parent_map.get(&u) != Some(&v) {
                // Update low value of u for parent function calls.
                let disc_v = *discovery_time.get(&v).unwrap_or(&usize::MAX);
                let low_u = low_time.get_mut(&u).expect("must exist");
                *low_u = std::cmp::min(*low_u, disc_v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Level;
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
    fn test_find_chokepoints() {
        let level = make_test_level();
        let graph = SectorGraph::build(&level);

        // In a 0 <-> 1 <-> 2 map, sector 1 is the only chokepoint (articulation point)
        // because removing it disconnects 0 and 2.
        let chokepoints = graph.find_chokepoints();

        assert_eq!(chokepoints.len(), 1);
        assert!(chokepoints.contains(&1));
    }
}
