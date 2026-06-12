use doom_map::analyzer::MapAnalyzer;
use doom_map::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

#[test]
fn havoc_test_analyzer_depth_limit() {
    let mut adj = HashMap::new();
    // Use an ungodly number of nodes to force exhaustion
    let num_nodes = 500_000;
    for i in 0..num_nodes {
        adj.insert(i, HashSet::from([i + 1]));
    }
    adj.insert(num_nodes, HashSet::from([num_nodes - 1]));
    for i in 1..num_nodes {
        adj.get_mut(&i).unwrap().insert(i - 1);
    }

    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);
    let _ = analyzer.chokepoints();
    let _ = analyzer.isolated_areas();
}
