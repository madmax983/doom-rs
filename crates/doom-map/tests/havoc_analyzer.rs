use doom_map::analyzer::MapAnalyzer;
use doom_map::graph::SectorGraph;
use std::collections::{HashMap, HashSet};

#[test]
fn havoc_test_analyzer_does_not_panic_on_unconnected_neighbors() {
    let mut adj = HashMap::new();
    // 0 connects to 1 and 2, but 1 and 2 don't exist in the map
    adj.insert(0, HashSet::from([1, 2]));
    // 3 connects to 0
    adj.insert(3, HashSet::from([0]));
    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);
    // This should not panic
    let _ = analyzer.chokepoints();
    let _ = analyzer.isolated_areas();
}

#[test]
fn havoc_test_analyzer_does_not_panic_on_asymmetric_edges() {
    let mut adj = HashMap::new();
    adj.insert(0, HashSet::from([1, 2]));
    adj.insert(1, HashSet::from([0, 2]));
    adj.insert(2, HashSet::from([0, 1])); // Connected triangle

    // Asymmetric edge pointing to 0 from an unconnected node 3
    adj.insert(3, HashSet::from([0]));
    // Node 0 does not have an edge to 3

    // Add a malformed asymmetric connection
    adj.insert(4, HashSet::from([2]));
    // Let's assume 2 connects to 4 but 4 doesn't exist? (already tested)

    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);
    let _ = analyzer.chokepoints();
}

#[test]
fn havoc_test_analyzer_missing_back_edges() {
    let mut adj = HashMap::new();
    adj.insert(0, HashSet::from([1, 2]));
    adj.insert(1, HashSet::from([0, 2]));
    // Node 2 missing from adj!

    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);
    let _ = analyzer.chokepoints();
}
