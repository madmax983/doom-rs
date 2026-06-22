//! Map topology analyzer for finding chokepoints and isolated areas.
//!
//! The `MapAnalyzer` uses standard graph algorithms to detect critical map features.
//! **Chokepoints**: (Articulation Points) Ssectors that, if removed, would split the map into two disconnected halves.
//! **Isolated Areas**: Finds distinct disconnected clusters of sectors within the map.
//!
//! # Examples
//! ```
//! use doom_map::SectorGraph;
//! use doom_map::analyzer::MapAnalyzer;
//! use std::collections::{HashMap, HashSet};
//!
//! // Construct a manual graph where sector 2 connects {0, 1} and {3}
//! let mut adj = HashMap::new();
//! adj.insert(0, HashSet::from([1, 2]));
//! adj.insert(1, HashSet::from([0, 2]));
//! adj.insert(2, HashSet::from([0, 1, 3]));
//! adj.insert(3, HashSet::from([2]));
//! let graph = SectorGraph { adjacency_list: adj };
//!
//! let analyzer = MapAnalyzer::new(&graph);
//!
//! // Sector 2 is a chokepoint because its removal disconnects {0, 1} from {3}
//! assert_eq!(analyzer.chokepoints(), vec![2]);
//! ```

use crate::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

/// Analyzes map topology for tactical features.
pub struct MapAnalyzer<'a> {
    graph: &'a SectorGraph,
}

impl<'a> MapAnalyzer<'a> {
    /// The MapAnalyzer is the cartographer's lens for finding tactical advantages.
    ///
    /// By supplying a `SectorGraph`, this struct can traverse the connections between
    /// map areas to discover chokepoints and isolated zones. This is vital for
    /// understanding the flow of a map and predicting where players might get trapped.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_map::SectorGraph;
    /// use doom_map::analyzer::MapAnalyzer;
    /// use std::collections::{HashMap, HashSet};
    ///
    /// // A simple linear map: 0 <-> 1 <-> 2
    /// let mut adj = HashMap::new();
    /// adj.insert(0, HashSet::from([1]));
    /// adj.insert(1, HashSet::from([0, 2]));
    /// adj.insert(2, HashSet::from([1]));
    /// let graph = SectorGraph { adjacency_list: adj };
    ///
    /// let analyzer = MapAnalyzer::new(&graph);
    /// assert_eq!(analyzer.chokepoints(), vec![1]); // Sector 1 is a chokepoint!
    /// ```
    pub fn new(graph: &'a SectorGraph) -> Self {
        Self { graph }
    }

    /// Finds articulation points (sectors that, if removed, disconnect parts of the map).
    pub fn chokepoints(&self) -> Vec<usize> {
        let mut visited = HashSet::new();
        let mut discovery_time = HashMap::new();
        let mut low_time = HashMap::new();
        let mut parent = HashMap::new();
        let mut articulation_points = HashSet::new();
        let mut time = 0;

        for &node in self.graph.adjacency_list.keys() {
            if !visited.contains(&node) {
                // Iterative DFS to avoid stack overflow on deep graphs.
                let mut stack = vec![(node, self.graph.adjacency_list.get(&node).unwrap().iter())];

                visited.insert(node);
                time += 1;
                discovery_time.insert(node, time);
                low_time.insert(node, time);
                let mut children_map: HashMap<usize, usize> = HashMap::new();

                while let Some((u, mut neighbors_iter)) = stack.pop() {
                    let mut pushed_child = false;

                    while let Some(&v) = neighbors_iter.next() {
                        if !self.graph.adjacency_list.contains_key(&v) {
                            continue;
                        }
                        if !visited.contains(&v) {
                            *children_map.entry(u).or_default() += 1;
                            parent.insert(v, u);

                            visited.insert(v);
                            time += 1;
                            discovery_time.insert(v, time);
                            low_time.insert(v, time);

                            stack.push((u, neighbors_iter));
                            stack.push((v, self.graph.adjacency_list.get(&v).unwrap().iter()));
                            pushed_child = true;
                            break;
                        } else if parent.get(&u) != Some(&v) {
                            let (low_u, disc_v) =
                                (low_time.get(&u).copied(), discovery_time.get(&v).copied());
                            if let (Some(low_u), Some(disc_v)) = (low_u, disc_v) {
                                let new_low = low_u.min(disc_v);
                                low_time.insert(u, new_low);
                            }
                        }
                    }

                    if !pushed_child {
                        // After visiting all neighbors of u, if u is not root, update parent's low_time
                        if let Some(&p) = parent.get(&u) {
                            let (low_u, low_p, disc_p) = (
                                low_time.get(&u).copied(),
                                low_time.get(&p).copied(),
                                discovery_time.get(&p).copied(),
                            );
                            if let (Some(low_u), Some(low_p), Some(disc_p)) = (low_u, low_p, disc_p)
                            {
                                let new_low = low_p.min(low_u);
                                low_time.insert(p, new_low);

                                if low_u >= disc_p && parent.contains_key(&p) {
                                    articulation_points.insert(p);
                                }
                            }
                        } else if *children_map.get(&u).unwrap_or(&0) > 1 {
                            articulation_points.insert(u);
                        }
                    }
                }
            }
        }

        let mut ap_vec: Vec<usize> = articulation_points.into_iter().collect();
        ap_vec.sort_unstable();
        ap_vec
    }

    /// Finds distinct disconnected areas of the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_map::SectorGraph;
    /// use doom_map::analyzer::MapAnalyzer;
    /// use std::collections::{HashMap, HashSet};
    ///
    /// // Two disconnected rooms: 0 <-> 1 and 2 <-> 3
    /// let mut adj = HashMap::new();
    /// adj.insert(0, HashSet::from([1]));
    /// adj.insert(1, HashSet::from([0]));
    /// adj.insert(2, HashSet::from([3]));
    /// adj.insert(3, HashSet::from([2]));
    /// let graph = SectorGraph { adjacency_list: adj };
    ///
    /// let analyzer = MapAnalyzer::new(&graph);
    /// let areas = analyzer.isolated_areas();
    /// assert_eq!(areas.len(), 2);
    /// ```
    pub fn isolated_areas(&self) -> Vec<HashSet<usize>> {
        let mut visited = HashSet::new();
        let mut components = Vec::new();

        for &node in self.graph.adjacency_list.keys() {
            if !visited.contains(&node) {
                let mut component = HashSet::new();
                let mut queue = vec![node];
                visited.insert(node);

                while let Some(curr) = queue.pop() {
                    component.insert(curr);
                    if let Some(neighbors) = self.graph.adjacency_list.get(&curr) {
                        for &n in neighbors {
                            // Only traverse edges to nodes that actually exist in the graph.
                            if self.graph.adjacency_list.contains_key(&n) && !visited.contains(&n) {
                                visited.insert(n);
                                queue.push(n);
                            }
                        }
                    }
                }
                components.push(component);
            }
        }
        components
    }

    /// Exports the chokepoints and isolated areas to a GeoJSON FeatureCollection string.
    /// The `Level` is used to determine the coordinates of each sector.
    pub fn export_analysis_to_geojson(&self, level: &crate::Level) -> String {
        let chokepoints = self.chokepoints();
        let areas = self.isolated_areas();

        // Precompute a center point for each sector
        let mut sector_centers = HashMap::new();
        for ld in &level.linedefs {
            let v1 = &level.vertexes[ld.from_vertex as usize];
            if ld.right_sidedef != crate::lumps::SIDEDEF_NONE {
                let sd = &level.sidedefs[ld.right_sidedef as usize];
                sector_centers
                    .entry(sd.sector as usize)
                    .or_insert((v1.x, v1.y));
            }
            if ld.left_sidedef != crate::lumps::SIDEDEF_NONE {
                let sd = &level.sidedefs[ld.left_sidedef as usize];
                sector_centers
                    .entry(sd.sector as usize)
                    .or_insert((v1.x, v1.y));
            }
        }

        let mut features = Vec::new();

        for &choke in &chokepoints {
            if let Some(&(x, y)) = sector_centers.get(&choke) {
                features.push(format!(
                    r#"    {{
      "type": "Feature",
      "geometry": {{
        "type": "Point",
        "coordinates": [{}, {}]
      }},
      "properties": {{
        "type": "chokepoint",
        "sector": {}
      }}
    }}"#,
                    x, y, choke
                ));
            }
        }

        for (i, area) in areas.iter().enumerate() {
            let mut coords = Vec::new();
            for &sector in area {
                if let Some(&(x, y)) = sector_centers.get(&sector) {
                    coords.push(format!("[{}, {}]", x, y));
                }
            }
            if !coords.is_empty() {
                features.push(format!(
                    r#"    {{
      "type": "Feature",
      "geometry": {{
        "type": "MultiPoint",
        "coordinates": [{}]
      }},
      "properties": {{
        "type": "isolated_area",
        "id": {}
      }}
    }}"#,
                    coords.join(", "),
                    i
                ));
            }
        }

        let features_str = features.join(",\n");
        format!(
            r#"{{
  "type": "FeatureCollection",
  "features": [
{}
  ]
}}"#,
            features_str
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::SectorGraph;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_chokepoints() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1, 2]));
        adj.insert(1, HashSet::from([0, 2]));
        adj.insert(2, HashSet::from([0, 1, 3])); // 2 connects {0,1} and {3}
        adj.insert(3, HashSet::from([2, 4])); // 3 connects {2} and {4}
        adj.insert(4, HashSet::from([3]));
        let graph = SectorGraph {
            adjacency_list: adj,
        };

        let analyzer = MapAnalyzer::new(&graph);
        let chokes = analyzer.chokepoints();
        // 2 and 3 are both chokepoints because removing either splits the graph.
        assert_eq!(chokes, vec![2, 3]);
    }

    #[test]
    fn test_isolated_areas() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1]));
        adj.insert(1, HashSet::from([0]));
        adj.insert(2, HashSet::from([3]));
        adj.insert(3, HashSet::from([2]));
        let graph = SectorGraph {
            adjacency_list: adj,
        };

        let analyzer = MapAnalyzer::new(&graph);
        let areas = analyzer.isolated_areas();
        assert_eq!(areas.len(), 2);
    }

    #[test]
    fn test_chokepoints_empty() {
        let graph = SectorGraph {
            adjacency_list: HashMap::new(),
        };
        let analyzer = MapAnalyzer::new(&graph);
        assert_eq!(analyzer.chokepoints(), vec![]);
    }

    #[test]
    fn test_chokepoints_fully_connected() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1, 2]));
        adj.insert(1, HashSet::from([0, 2]));
        adj.insert(2, HashSet::from([0, 1]));
        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);
        assert_eq!(analyzer.chokepoints(), vec![]);
    }

    #[test]
    fn test_chokepoints_disconnected() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1]));
        adj.insert(1, HashSet::from([0]));
        adj.insert(2, HashSet::from([3]));
        adj.insert(3, HashSet::from([2]));
        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);
        assert_eq!(analyzer.chokepoints(), vec![]);
    }

    #[test]
    fn test_isolated_areas_single() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1]));
        adj.insert(1, HashSet::from([0]));
        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);
        let areas = analyzer.isolated_areas();
        assert_eq!(areas.len(), 1);
        assert!(areas[0].contains(&0));
        assert!(areas[0].contains(&1));
    }

    #[test]
    fn test_chokepoints_large_linear() {
        // Havoc: Trigger stack overflow without iterative rewrite
        let mut adj = HashMap::new();
        for i in 0..10000 {
            adj.insert(i, HashSet::from([i + 1]));
        }
        adj.insert(10000, HashSet::from([9999]));
        for i in 1..10000 {
            adj.get_mut(&i).unwrap().insert(i - 1);
        }

        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);
        let chokes = analyzer.chokepoints();
        assert_eq!(chokes.len(), 9999);
    }

    #[test]
    fn test_export_analysis_to_geojson() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1]));
        adj.insert(1, HashSet::from([0]));
        let graph = SectorGraph {
            adjacency_list: adj,
        };

        let analyzer = MapAnalyzer::new(&graph);

        let level = crate::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![
                crate::lumps::Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: crate::lumps::SIDEDEF_NONE,
                },
                crate::lumps::Linedef {
                    from_vertex: 1,
                    to_vertex: 0,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 1,
                    left_sidedef: crate::lumps::SIDEDEF_NONE,
                },
            ],
            sidedefs: vec![
                crate::lumps::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"WALL1\0\0\0",
                    lower_texture: *b"WALL2\0\0\0",
                    middle_texture: *b"WALL3\0\0\0",
                    sector: 0,
                },
                crate::lumps::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"WALL1\0\0\0",
                    lower_texture: *b"WALL2\0\0\0",
                    middle_texture: *b"WALL3\0\0\0",
                    sector: 1,
                },
            ],
            vertexes: vec![
                crate::lumps::Vertex { x: 0, y: 0 },
                crate::lumps::Vertex { x: 10, y: 10 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                crate::lumps::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                crate::lumps::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject: crate::lumps::Reject::parse_lump(&[0u8], 1).unwrap(),
            blockmap: crate::lumps::Blockmap::parse_lump(&[
                0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])
            .unwrap(),
        };

        let geojson = analyzer.export_analysis_to_geojson(&level);
        assert!(geojson.contains(r#"FeatureCollection"#));
        assert!(geojson.contains(r#"isolated_area"#));
        assert!(geojson.contains(r#"[0, 0]"#));
        assert!(geojson.contains(r#"[10, 10]"#));
    }
}
