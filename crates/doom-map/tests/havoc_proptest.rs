use doom_map::analyzer::MapAnalyzer;
use doom_map::graph::SectorGraph;
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

proptest! {
    #[test]
    fn havoc_test_random_graphs(edges in prop::collection::vec((any::<usize>(), any::<usize>()), 0..1000)) {
        let mut adj: HashMap<usize, HashSet<usize>> = HashMap::new();
        for (u, v) in edges {
            adj.entry(u).or_default().insert(v);
            adj.entry(v).or_default().insert(u);
        }
        let graph = SectorGraph { adjacency_list: adj };
        let analyzer = MapAnalyzer::new(&graph);
        let _ = analyzer.chokepoints();
        let _ = analyzer.isolated_areas();
    }
}
