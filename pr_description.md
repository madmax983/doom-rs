🎯 Target: doom-map/src/analyzer.rs and doom-tui/src/event_loop.rs / sixel.rs
💣 Risk:
- `MapAnalyzer::chokepoints` contained `unwrap()` statements on `adjacency_list.get(&v)`. While logic currently guarantees `v` is present in `adjacency_list`, a future refactor could break this invariant causing a mysterious panic.
- Warning logs for the deprecated `.set_skip(true)` usage in `doom-tui` polluted the `cargo clippy` build output with lints.
🧪 Strategy:
- Replaced `unwrap()` calls in `MapAnalyzer` with `.unwrap_or(&empty_adj)` conditional checking using a local empty set to gracefully avoid panics on missing adjacencies by bypassing them without breaking iterative DFS traversal states.
- Suppressed `ratatui` deprecation warnings by wrapping the deprecated method calls inside `#[allow(deprecated)] { ... }` blocks.
- Added `havoc_test_analyzer_does_not_panic_on_missing_adjacency` test in `doom-map` to prove `MapAnalyzer` operates safely when topological adjacencies are entirely broken without panicking.
🔬 Verification: Run `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` to verify changes pass.
