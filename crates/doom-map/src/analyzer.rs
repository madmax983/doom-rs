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

        let empty_hashset = HashSet::new();

        for &node in self.graph.adjacency_list.keys() {
            if !visited.contains(&node) {
                // Iterative DFS to avoid stack overflow on deep graphs.
                let neighbors = self
                    .graph
                    .adjacency_list
                    .get(&node)
                    .unwrap_or(&empty_hashset);
                let mut stack = vec![(node, neighbors.iter())];

                visited.insert(node);
                time += 1;
                discovery_time.insert(node, time);
                low_time.insert(node, time);
                let mut children_map: HashMap<usize, usize> = HashMap::new();

                while let Some((u, mut neighbors_iter)) = stack.pop() {
                    let mut pushed_child = false;

                    while let Some(&v) = neighbors_iter.next() {
                        if !visited.contains(&v) {
                            *children_map.entry(u).or_default() += 1;
                            parent.insert(v, u);

                            visited.insert(v);
                            time += 1;
                            discovery_time.insert(v, time);
                            low_time.insert(v, time);

                            stack.push((u, neighbors_iter));
                            let v_neighbors =
                                self.graph.adjacency_list.get(&v).unwrap_or(&empty_hashset);
                            stack.push((v, v_neighbors.iter()));
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
            adj.get_mut(&i)
                .expect("expected node in large linear test")
                .insert(i - 1);
        }

        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);
        let chokes = analyzer.chokepoints();
        assert_eq!(chokes.len(), 9999);
    }

    #[test]
    fn havoc_test_analyzer_does_not_panic_on_asymmetric_edges() {
        let mut adj = HashMap::new();
        // 0 connects to 1, but 1 doesn't know about 0
        adj.insert(0, HashSet::from([1]));
        adj.insert(1, HashSet::from([2]));
        adj.insert(2, HashSet::new());
        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);
        // The algorithm shouldn't panic, but its exact output on an invalid graph
        // (asymmetric edges) is technically undefined and could depend on iteration order.
        // What's important is it doesn't crash or go into an infinite loop.
        let _chokes = analyzer.chokepoints();
    }

    #[test]
    fn havoc_test_analyzer_missing_back_edges() {
        let mut adj = HashMap::new();
        // 0 connects to 1, but 1 is completely missing from the graph!
        adj.insert(0, HashSet::from([1]));
        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);
        let chokes = analyzer.chokepoints();
        assert_eq!(chokes, vec![]);
    }
}
