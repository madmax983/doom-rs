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

#[derive(Default)]
struct ChokepointState {
    visited: HashSet<usize>,
    discovery_time: HashMap<usize, usize>,
    low_time: HashMap<usize, usize>,
    parent: HashMap<usize, usize>,
    children_map: HashMap<usize, usize>,
    articulation_points: HashSet<usize>,
    time: usize,
}

impl ChokepointState {
    fn visit_node(&mut self, node: usize) {
        self.visited.insert(node);
        self.time += 1;
        self.discovery_time.insert(node, self.time);
        self.low_time.insert(node, self.time);
    }

    fn register_child(&mut self, parent: usize, child: usize) {
        *self.children_map.entry(parent).or_default() += 1;
        self.parent.insert(child, parent);
        self.visit_node(child);
    }
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
        let mut state = ChokepointState::default();

        for &node in self.graph.adjacency_list.keys() {
            if !state.visited.contains(&node) {
                self.process_component(node, &mut state);
            }
        }

        let mut ap_vec: Vec<usize> = state.articulation_points.into_iter().collect();
        ap_vec.sort_unstable();
        ap_vec
    }

    fn process_component(&self, root: usize, state: &mut ChokepointState) {
        state.visit_node(root);
        let mut stack = vec![(root, self.graph.adjacency_list.get(&root).unwrap().iter())];

        while let Some((u, mut neighbors_iter)) = stack.pop() {
            let mut pushed_child = false;

            while let Some(&v) = neighbors_iter.next() {
                if !self.graph.adjacency_list.contains_key(&v) {
                    continue;
                }

                if !state.visited.contains(&v) {
                    state.register_child(u, v);

                    stack.push((u, neighbors_iter));
                    stack.push((v, self.graph.adjacency_list.get(&v).unwrap().iter()));
                    pushed_child = true;
                    break;
                }

                self.process_back_edge(u, v, state);
            }

            if !pushed_child {
                self.finish_node(u, state);
            }
        }
    }

    fn process_back_edge(&self, u: usize, v: usize, state: &mut ChokepointState) {
        if state.parent.get(&u) == Some(&v) {
            return;
        }

        let Some(&low_u) = state.low_time.get(&u) else {
            return;
        };
        let Some(&disc_v) = state.discovery_time.get(&v) else {
            return;
        };

        state.low_time.insert(u, low_u.min(disc_v));
    }

    fn finish_node(&self, u: usize, state: &mut ChokepointState) {
        let Some(&p) = state.parent.get(&u) else {
            // Root node
            if *state.children_map.get(&u).unwrap_or(&0) > 1 {
                state.articulation_points.insert(u);
            }
            return;
        };

        // Non-root node
        let (low_u, low_p, disc_p) = (
            state.low_time.get(&u).copied(),
            state.low_time.get(&p).copied(),
            state.discovery_time.get(&p).copied(),
        );

        if let (Some(low_u), Some(low_p), Some(disc_p)) = (low_u, low_p, disc_p) {
            state.low_time.insert(p, low_p.min(low_u));

            if low_u >= disc_p && state.parent.contains_key(&p) {
                state.articulation_points.insert(p);
            }
        }
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
            adj.get_mut(&i).unwrap().insert(i - 1);
        }

        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);
        let chokes = analyzer.chokepoints();
        assert_eq!(chokes.len(), 9999);
    }
}
