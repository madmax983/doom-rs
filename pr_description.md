🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."

🚮 Smell: The `doom-tui` crate was failing the `clippy` checks due to a deprecation warning on `ratatui::buffer::Cell::set_skip(true)` usage, preventing a clean compilation under `-D warnings`. Simply suppressing it inline inside a `.map()` closure triggers an experimental E0658 compiler error.
✨ Solution: Safely wrapped the specific `set_skip` method calls within an explicit `#[allow(deprecated)] { ... }` block, rather than naively applying `set_diff_option(CellDiffOption::Skip)` which can cause compatibility errors in certain versions.
🧼 Benefit: Ensures clean compilation with strictly enforced warning-free builds without breaking backward compatibility or relying on experimental compiler features.
🛡️ Verification: Tests passed. No logic changed. `cargo clippy --all-targets --all-features -- -D warnings` now runs cleanly.
