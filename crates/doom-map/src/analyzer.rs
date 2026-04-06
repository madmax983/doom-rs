use crate::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

/// Analyzes map topology for tactical features.
pub struct MapAnalyzer<'a> {
    graph: &'a SectorGraph,
}

impl<'a> MapAnalyzer<'a> {
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
                self.ap_util(
                    node,
                    &mut visited,
                    &mut discovery_time,
                    &mut low_time,
                    &mut parent,
                    &mut articulation_points,
                    &mut time,
                );
            }
        }

        let mut ap_vec: Vec<usize> = articulation_points.into_iter().collect();
        ap_vec.sort_unstable();
        ap_vec
    }

    fn ap_util(
        &self,
        u: usize,
        visited: &mut HashSet<usize>,
        discovery_time: &mut HashMap<usize, usize>,
        low_time: &mut HashMap<usize, usize>,
        parent: &mut HashMap<usize, usize>,
        ap: &mut HashSet<usize>,
        time: &mut usize,
    ) {
        let mut children = 0;
        visited.insert(u);
        *time += 1;
        discovery_time.insert(u, *time);
        low_time.insert(u, *time);

        if let Some(neighbors) = self.graph.adjacency_list.get(&u) {
            for &v in neighbors {
                if !visited.contains(&v) {
                    children += 1;
                    parent.insert(v, u);
                    self.ap_util(v, visited, discovery_time, low_time, parent, ap, time);

                    let low_v = *low_time.get(&v).unwrap();
                    let low_u = *low_time.get(&u).unwrap();
                    low_time.insert(u, low_u.min(low_v));

                    if parent.get(&u).is_none() && children > 1 {
                        ap.insert(u);
                    }
                    if parent.get(&u).is_some() && low_v >= *discovery_time.get(&u).unwrap() {
                        ap.insert(u);
                    }
                } else if parent.get(&u) != Some(&v) {
                    let low_u = *low_time.get(&u).unwrap();
                    let disc_v = *discovery_time.get(&v).unwrap();
                    low_time.insert(u, low_u.min(disc_v));
                }
            }
        }
    }

    /// Finds distinct disconnected areas of the map.
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
                            if !visited.contains(&n) {
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
}
