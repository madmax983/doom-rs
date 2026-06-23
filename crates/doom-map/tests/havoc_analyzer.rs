use doom_map::analyzer::MapAnalyzer;
use doom_map::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

#[test]
fn havoc_test_analyzer_missing_back_edges() {
    let mut adj = HashMap::new();
    // 0 has an edge to 1
    adj.insert(0, HashSet::from([1]));
    // 1 has an edge to 2
    adj.insert(1, HashSet::from([2]));

    // Note that 2 is not even in the adjacency map,
    // which simulates a badly formed level or graph.

    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);
    let _chokepoints = analyzer.chokepoints();
}
