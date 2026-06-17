//! Sector-based pathfinding for intelligent monster navigation.

use doom_map::graph::SectorGraph;

/// Provides tactical navigation assistance for monsters.
pub struct MonsterPathfinder<'a> {
    graph: &'a SectorGraph,
}

impl<'a> MonsterPathfinder<'a> {
    /// Creates a new `MonsterPathfinder` from a `SectorGraph`.
    pub fn new(graph: &'a SectorGraph) -> Self {
        Self { graph }
    }

    /// Determines the next sector a monster should move towards to reach a target sector.
    /// Returns the sector index of the next step, or `None` if no path exists.
    pub fn next_step(&self, current_sector: usize, target_sector: usize) -> Option<usize> {
        if current_sector == target_sector {
            return Some(target_sector);
        }

        if let Some(path) = self.graph.shortest_path(current_sector, target_sector) {
            // path[0] is current_sector, path[1] is the next step
            if path.len() > 1 {
                return Some(path[1]);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_next_step() {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1]));
        adj.insert(1, HashSet::from([0, 2]));
        adj.insert(2, HashSet::from([1, 3]));
        adj.insert(3, HashSet::from([2]));

        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let pathfinder = MonsterPathfinder::new(&graph);

        assert_eq!(pathfinder.next_step(0, 3), Some(1));
        assert_eq!(pathfinder.next_step(1, 3), Some(2));
        assert_eq!(pathfinder.next_step(2, 3), Some(3));
        assert_eq!(pathfinder.next_step(3, 3), Some(3));
    }
}
