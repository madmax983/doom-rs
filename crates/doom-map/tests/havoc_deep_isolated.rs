use doom_map::analyzer::MapAnalyzer;
use doom_map::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

#[test]
fn havoc_test_isolated_areas_deep_linear() {
    let mut adj = HashMap::new();
    for i in 0..100000 {
        adj.insert(i, HashSet::from([i + 1]));
    }
    adj.insert(100000, HashSet::from([99999]));
    for i in 1..100000 {
        adj.get_mut(&i).unwrap().insert(i - 1);
    }

    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);
    let areas = analyzer.isolated_areas();
    assert_eq!(areas.len(), 1);
}
