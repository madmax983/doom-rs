🎯 Target: `MapAnalyzer::chokepoints` in `crates/doom-map/src/analyzer.rs`

💣 Risk: Usage of `.unwrap()` on adjacency list lookup of child nodes could cause a panic if a malformed graph omits the child node from the hashmap keys. This protects against corrupted map topologies.

🧪 Strategy: Replaced `.unwrap()` with `.unwrap_or(&empty_set)` to gracefully fallback to an empty iterator when missing neighbors. Added new havoc tests `havoc_test_analyzer_missing_back_edges` and `havoc_test_analyzer_does_not_panic_on_asymmetric_edges` to cover missing child nodes gracefully.

🔬 Verification: `cargo test -p doom-map --test havoc_analyzer` and `cargo test -p doom-map analyzer`
