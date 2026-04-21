#![allow(missing_docs)]
use doom_map::SectorGraph;
use doom_map::analyzer::MapAnalyzer;
use std::collections::{HashMap, HashSet};

#[test]
fn havoc_test_analyzer_does_not_panic_on_unconnected_neighbors() {
    let mut adj = HashMap::new();
    adj.insert(0, HashSet::from([1, 2]));
    // Nodes 1 and 2 don't have their own adjacency entries but are referenced by 0.
    // This tests that analyzer correctly handles partial or unconnected topologies.
    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);
    let _ = analyzer.chokepoints();
}
