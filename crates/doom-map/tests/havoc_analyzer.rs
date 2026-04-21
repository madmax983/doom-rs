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
