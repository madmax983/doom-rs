## 2025-05-12 - `unwrap()` panics in Doom Map Analyzer on missing map nodes
**Learning:** Found an edge case in `MapAnalyzer::chokepoints()` and `isolated_areas()` where referencing non-existent nodes in `adjacency_list` triggers a panic via `unwrap()` or enters infinite loops/panics during iteration because they assume all nodes referenced by edges exist in the graph.
**Action:** Replace `unwrap()` with safe fallbacks and test missing node connections using test cases for both `MapAnalyzer::chokepoints` and `MapAnalyzer::isolated_areas`.
## 2025-05-12 - MapAnalyzer missing node panics
**Learning:** `MapAnalyzer::chokepoints()` expected that all node IDs listed as values in `adjacency_list` had corresponding entries in the hash map. Passing invalid/dangling edges causes `unwrap()` panics during the DFS search iteration. `isolated_areas()` safely handles this due to `contains_key` checks.
**Action:** Replaced `unwrap()` with `unwrap_or(&empty_set)` before creating iterators, preventing the panic and allowing the DFS to naturally backtrack. Added robust testing.
