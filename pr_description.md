🎯 Target: `MapAnalyzer::chokepoints` in `doom-map`
💣 Risk: Possible panics when fetching a node's adjacency list from the graph during Map topological analysis.
🧪 Strategy: Added `havoc` tests. Replaced `unwrap()` with `if let Some` and `let Some(...) else { continue; }` guards.
🔬 Verification: `cargo test -p doom-map --lib analyzer`

Note: I did not fix the clippy warnings present in `doom-game` as Sentry mandates isolation of crate changes, but I have verified `doom-map` compiles and passes flawlessly.
