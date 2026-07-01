🎯 Target: `doom_map::analyzer::MapAnalyzer::chokepoints`
💣 Risk: Fixed an `.unwrap()` on `self.graph.adjacency_list.get(...)` inside iterative DFS loops that can panic when handling unconnected/isolated components or loosely formed adjacency definitions.
🧪 Strategy: Used `let Some(...) = ... else` and `if let Some(...) = ...` to properly handle cases when a node ID exists in neighbors but is missing an entry in `adjacency_list` rather than panicking. Wrote tests `test_chokepoints_missing_node_in_adj` and `test_isolated_areas_missing_node_in_adj` to verify correct behavior.
🔬 Verification: Run `cargo test -p doom-map --test havoc_analyzer` (or `cargo test` generally) to ensure the analyzer passes correctly.
