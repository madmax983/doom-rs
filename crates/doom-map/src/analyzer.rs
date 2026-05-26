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
        let mut finder = ChokepointFinder::new(self.graph);
        finder.find()
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
        let mut finder = IsolatedAreasFinder::new(self.graph);
        finder.find()
    }
}

struct IsolatedAreasFinder<'a> {
    graph: &'a SectorGraph,
    visited: HashSet<usize>,
    components: Vec<HashSet<usize>>,
}

impl<'a> IsolatedAreasFinder<'a> {
    fn new(graph: &'a SectorGraph) -> Self {
        let capacity = graph.adjacency_list.len();
        Self {
            graph,
            visited: HashSet::with_capacity(capacity),
            components: Vec::new(),
        }
    }

    fn find(&mut self) -> Vec<HashSet<usize>> {
        for &node in self.graph.adjacency_list.keys() {
            if !self.visited.contains(&node) {
                self.traverse_from(node);
            }
        }
        std::mem::take(&mut self.components)
    }

    fn traverse_from(&mut self, start_node: usize) {
        let mut component = HashSet::new();
        let mut queue = vec![start_node];
        self.visited.insert(start_node);

        while let Some(curr) = queue.pop() {
            component.insert(curr);
            if let Some(neighbors) = self.graph.adjacency_list.get(&curr) {
                for &n in neighbors {
                    // Only traverse edges to nodes that actually exist in the graph.
                    if self.graph.adjacency_list.contains_key(&n) && !self.visited.contains(&n) {
                        self.visited.insert(n);
                        queue.push(n);
                    }
                }
            }
        }
        self.components.push(component);
    }
}
struct ChokepointFinder<'a> {
    graph: &'a SectorGraph,
    visited: HashSet<usize>,
    discovery_time: HashMap<usize, usize>,
    low_time: HashMap<usize, usize>,
    parent: HashMap<usize, usize>,
    articulation_points: HashSet<usize>,
    time: usize,
}

impl<'a> ChokepointFinder<'a> {
    fn new(graph: &'a SectorGraph) -> Self {
        let capacity = graph.adjacency_list.len();
        Self {
            graph,
            visited: HashSet::with_capacity(capacity),
            discovery_time: HashMap::with_capacity(capacity),
            low_time: HashMap::with_capacity(capacity),
            parent: HashMap::with_capacity(capacity),
            articulation_points: HashSet::new(),
            time: 0,
        }
    }

    fn find(&mut self) -> Vec<usize> {
        for &node in self.graph.adjacency_list.keys() {
            if !self.visited.contains(&node) {
                self.traverse_from(node);
            }
        }
        let mut ap_vec: Vec<usize> = self.articulation_points.drain().collect();
        ap_vec.sort_unstable();
        ap_vec
    }

    fn traverse_from(&mut self, start_node: usize) {
        // Iterative DFS to avoid stack overflow on deep graphs.
        let mut stack = vec![(
            start_node,
            self.graph.adjacency_list.get(&start_node).unwrap().iter(),
        )];
        self.visited.insert(start_node);
        self.time += 1;
        self.discovery_time.insert(start_node, self.time);
        self.low_time.insert(start_node, self.time);

        let mut children_map: HashMap<usize, usize> = HashMap::new();

        while let Some((u, mut neighbors_iter)) = stack.pop() {
            let mut pushed_child = false;

            while let Some(&v) = neighbors_iter.next() {
                if !self.graph.adjacency_list.contains_key(&v) {
                    continue;
                }

                if !self.visited.contains(&v) {
                    *children_map.entry(u).or_default() += 1;
                    self.parent.insert(v, u);

                    self.visited.insert(v);
                    self.time += 1;
                    self.discovery_time.insert(v, self.time);
                    self.low_time.insert(v, self.time);

                    stack.push((u, neighbors_iter));
                    stack.push((v, self.graph.adjacency_list.get(&v).unwrap().iter()));
                    pushed_child = true;
                    break;
                } else if self.parent.get(&u) != Some(&v) {
                    self.update_low_time_back_edge(u, v);
                }
            }

            if !pushed_child {
                self.finish_node(u, *children_map.get(&u).unwrap_or(&0));
            }
        }
    }

    fn update_low_time_back_edge(&mut self, u: usize, v: usize) {
        let low_u = self.low_time.get(&u).copied();
        let disc_v = self.discovery_time.get(&v).copied();

        if let (Some(low_u), Some(disc_v)) = (low_u, disc_v) {
            self.low_time.insert(u, low_u.min(disc_v));
        }
    }

    fn finish_node(&mut self, u: usize, children_count: usize) {
        if let Some(&p) = self.parent.get(&u) {
            let low_u = self.low_time.get(&u).copied();
            let low_p = self.low_time.get(&p).copied();
            let disc_p = self.discovery_time.get(&p).copied();

            if let (Some(low_u), Some(low_p), Some(disc_p)) = (low_u, low_p, disc_p) {
                self.low_time.insert(p, low_p.min(low_u));
                if low_u >= disc_p && self.parent.contains_key(&p) {
                    self.articulation_points.insert(p);
                }
            }
        } else if children_count > 1 {
            self.articulation_points.insert(u);
        }
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
