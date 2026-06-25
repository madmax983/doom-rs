#![allow(missing_docs)]

use doom_map::analyzer::MapAnalyzer;
use doom_map::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

#[test]
fn havoc_test_analyzer_graph_with_cycle() {
    let mut adj = HashMap::new();

    // A cyclic graph
    // 0 <-> 1
    // 0 <-> 2
    // 1 <-> 2
    adj.insert(0, HashSet::from([1, 2]));
    adj.insert(1, HashSet::from([0, 2]));
    adj.insert(2, HashSet::from([0, 1]));

    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);
    let _ = analyzer.chokepoints();
}
