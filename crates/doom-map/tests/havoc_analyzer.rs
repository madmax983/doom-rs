use doom_map::analyzer::MapAnalyzer;
use doom_map::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

#[test]
fn havoc_test_analyzer() {
    let mut adj = HashMap::new();
    // A malformed, unconnected graph
    adj.insert(0, HashSet::from([1]));
    // Node 1 is missing entirely!
    adj.insert(2, HashSet::from([0, 1]));
    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);

    // We expect this to not panic from unwrapping missing nodes
    analyzer.chokepoints();
}
