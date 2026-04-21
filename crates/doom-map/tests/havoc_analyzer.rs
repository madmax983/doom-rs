use doom_map::SectorGraph;
use doom_map::analyzer::MapAnalyzer;
use std::collections::{HashMap, HashSet};

#[test]
fn havoc_test_analyzer_does_not_panic_on_unconnected_neighbors() {
    let mut adj = HashMap::new();

    // Simulate a malformed graph where sector 0 points to sector 1,
    // but sector 1 does not exist in the graph keys.
    adj.insert(0, HashSet::from([1]));

    let graph = SectorGraph {
        adjacency_list: adj,
    };
    let analyzer = MapAnalyzer::new(&graph);

    // This proves that ap_util correctly handles missing nodes and avoids panic
    let chokes = analyzer.chokepoints();

    // An empty graph or single node returns empty
    assert_eq!(chokes, vec![]);
}
