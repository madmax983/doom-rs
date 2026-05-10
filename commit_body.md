🎯 Target: MapAnalyzer::chokepoints depth-first search graph traversal logic.
💣 Risk: Iterative graph traversal logic incorrectly assumes that all child edges referencing neighbors will exist in the `adjacency_list` HashMap. If a malformed `SectorGraph` contains an asymmetric or disconnected sub-graph pointing to a missing node, the `.unwrap()` panics during the iterator creation on the stack.
🧪 Strategy: Replaced `.unwrap()` with a safe `.unwrap_or(&empty_set)` that preserves the required backtracking properties of iterative DFS by pushing an empty neighbor set. Additionally wrote targeted `havoc` tests proving the algorithm handles incomplete or asymmetric graph inputs.
🔭 Verification: `cargo test -p doom-map --test havoc_analyzer`
