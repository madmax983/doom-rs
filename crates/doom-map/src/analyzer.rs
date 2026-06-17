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
//! assert_eq!(analyzer.chokepoints().unwrap(), vec![2]);
//! ```

use crate::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

/// Errors that can occur during graph analysis.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MapAnalyzerError {
    /// Graph traversal exceeded the complexity limit (e.g. infinite loops or huge node counts)
    #[error("Node limit exceeded. Graph too complex.")]
    LimitExceeded,
}

/// Analyzes map topology for tactical features.
pub struct MapAnalyzer<'a> {
    graph: &'a SectorGraph,
}

impl<'a> MapAnalyzer<'a> {
    /// Create a new analyzer for the given sector graph.
    ///
    /// # Examples
    /// ```
    /// use doom_map::SectorGraph;
    /// use doom_map::analyzer::MapAnalyzer;
    /// use std::collections::{HashMap, HashSet};
    ///
    /// let mut adj = HashMap::new();
    /// adj.insert(0, HashSet::from([1]));
    /// adj.insert(1, HashSet::from([0]));
    /// let graph = SectorGraph { adjacency_list: adj };
    ///
    /// let analyzer = MapAnalyzer::new(&graph);
    /// assert_eq!(analyzer.chokepoints().unwrap(), vec![]); // Only 2 sectors, no chokepoint
    /// ```
    pub fn new(graph: &'a SectorGraph) -> Self {
        Self { graph }
    }

    /// Finds articulation points (sectors that, if removed, disconnect parts of the map).
    pub fn chokepoints(&self) -> Result<Vec<usize>, MapAnalyzerError> {
        if self.graph.adjacency_list.len() > 65536 {
            return Err(MapAnalyzerError::LimitExceeded);
        }

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

                                // Articulation point condition 1: u is not root and low_u >= disc_p
                                if low_u >= disc_p && parent.contains_key(&p) {
                                    articulation_points.insert(p);
                                }
                            }
                        }
                    }
                }

                // Articulation point condition 2: root has > 1 children
                if children_map.get(&node).copied().unwrap_or(0) > 1 {
                    articulation_points.insert(node);
                }
            }
        }

        let mut sorted = articulation_points.into_iter().collect::<Vec<_>>();
        sorted.sort_unstable();
        Ok(sorted)
    }

    /// Finds distinct isolated components in the map.
    ///
    /// # Examples
    /// ```
    /// use doom_map::SectorGraph;
    /// use doom_map::analyzer::MapAnalyzer;
    /// use std::collections::{HashMap, HashSet};
    ///
    /// let mut adj = HashMap::new();
    /// adj.insert(0, HashSet::from([1]));
    /// adj.insert(1, HashSet::from([0]));
    /// adj.insert(2, HashSet::from([3]));
    /// adj.insert(3, HashSet::from([2]));
    /// let graph = SectorGraph { adjacency_list: adj };
    ///
    /// let analyzer = MapAnalyzer::new(&graph);
    /// let areas = analyzer.isolated_areas().unwrap();
    /// assert_eq!(areas.len(), 2);
    /// ```
    pub fn isolated_areas(&self) -> Result<Vec<HashSet<usize>>, MapAnalyzerError> {
        if self.graph.adjacency_list.len() > 65536 {
            return Err(MapAnalyzerError::LimitExceeded);
        }
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

        Ok(components)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chokepoints_empty() {
        let graph = SectorGraph {
            adjacency_list: HashMap::new(),
        };
        let analyzer = MapAnalyzer::new(&graph);
        assert_eq!(analyzer.chokepoints().unwrap(), vec![]);
    }

    #[test]
    fn test_chokepoints() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1, 2]));
        adj.insert(1, HashSet::from([0, 2]));
        adj.insert(2, HashSet::from([0, 1, 3]));
        adj.insert(3, HashSet::from([2]));

        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);

        let cps = analyzer.chokepoints().unwrap();
        assert_eq!(cps, vec![2]);
    }

    #[test]
    fn test_chokepoints_disconnected() {
        let mut adj = HashMap::new();
        // Component 1
        adj.insert(0, HashSet::from([1, 2]));
        adj.insert(1, HashSet::from([0, 2]));
        adj.insert(2, HashSet::from([0, 1, 3]));
        adj.insert(3, HashSet::from([2]));
        // Component 2
        adj.insert(4, HashSet::from([5, 6]));
        adj.insert(5, HashSet::from([4]));
        adj.insert(6, HashSet::from([4]));

        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);

        let cps = analyzer.chokepoints().unwrap();
        assert_eq!(cps, vec![2, 4]); // 2 is CP for C1, 4 is CP for C2
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

        let areas = analyzer.isolated_areas().unwrap();
        assert_eq!(areas.len(), 2);
    }

    #[test]
    fn test_chokepoints_fully_connected() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1, 2, 3]));
        adj.insert(1, HashSet::from([0, 2, 3]));
        adj.insert(2, HashSet::from([0, 1, 3]));
        adj.insert(3, HashSet::from([0, 1, 2]));

        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);

        let cps = analyzer.chokepoints().unwrap();
        assert_eq!(cps, vec![]); // No single sector removes connectivity
    }

    #[test]
    fn test_isolated_areas_single() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1, 2, 3]));
        adj.insert(1, HashSet::from([0, 2, 3]));
        adj.insert(2, HashSet::from([0, 1, 3]));
        adj.insert(3, HashSet::from([0, 1, 2]));

        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);

        let areas = analyzer.isolated_areas().unwrap();
        assert_eq!(areas.len(), 1);
        assert_eq!(areas[0].len(), 4);
    }

    #[test]
    fn test_chokepoints_large_linear() {
        let mut adj = HashMap::new();
        let num_nodes = 10_000;

        for i in 0..num_nodes {
            let mut edges = HashSet::new();
            if i > 0 {
                edges.insert(i - 1);
            }
            if i < num_nodes - 1 {
                edges.insert(i + 1);
            }
            adj.insert(i, edges);
        }

        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);

        let res = analyzer.chokepoints().unwrap();
        // In a line graph of N nodes, all N-2 interior nodes are chokepoints.
        assert_eq!(res.len(), num_nodes - 2);
    }
}
