#![no_main]

use doom_map::graph::SectorGraph;
use doom_map::analyzer::MapAnalyzer;
use libfuzzer_sys::fuzz_target;
use std::collections::{HashMap, HashSet};

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    let mut adj = HashMap::new();

    let mut i = 0;
    while i < data.len() - 1 {
        let node = data[i] as usize;
        let num_edges = data[i+1] as usize;
        i += 2;

        let mut edges = HashSet::new();
        for _ in 0..num_edges {
            if i < data.len() {
                edges.insert(data[i] as usize);
                i += 1;
            } else {
                break;
            }
        }
        adj.insert(node, edges);
    }

    let graph = SectorGraph { adjacency_list: adj };
    let analyzer = MapAnalyzer::new(&graph);

    let _ = analyzer.chokepoints();
    let _ = analyzer.isolated_areas();
});
