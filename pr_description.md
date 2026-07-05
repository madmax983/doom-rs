🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."

🕸️ Tangle: `doom-game/src/player.rs` unnecessarily re-exports `doom_types::limits::{NUM_POWERS, NUM_PSPRITES}` via `pub use`. This breaks strict module boundaries and leaks basic limit constants through the game logic crate rather than enforcing direct usage of the foundational `doom-types` crate.
📐 Blueprint: Removed the `pub use` re-exports from `doom-game/src/player.rs` and replaced them with internal `use` statements.
🧱 Stability: Clean dependency tree, eliminated re-export leak.
🔬 Verification: Built with `cargo check --all-targets --all-features`.
