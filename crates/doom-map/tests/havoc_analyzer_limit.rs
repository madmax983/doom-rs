use doom_map::analyzer::MapAnalyzer;
use doom_map::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

#[test]
fn havoc_test_analyzer_depth_limit() {
    let mut adj = HashMap::new();
    // Havoc: Trigger limit or loop by pushing 200,000 deep path
    for i in 0..200000 {
        adj.insert(i, HashSet::from([i + 1]));
    }
    adj.insert(200000, HashSet::from([199999]));
    for i in 1..200000 {
        adj.get_mut(&i).unwrap().insert(i - 1);
    }

    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);
    let _ = analyzer.chokepoints();
    let _ = analyzer.isolated_areas();
}
