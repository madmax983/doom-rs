🗺️ Atlas: [Enforce module boundaries across crates]

🕸️ Tangle: The crates `doom-game`, `doom-map`, `doom-audio`, `doom-demo`, `doom-net`, `doom-renderer`, `doom-tui`, and `doom-wad` had many internal modules marked as `pub mod` in their `lib.rs`, leaking internal structures and potentially increasing coupling.

📐 Blueprint: Changed the visibility of internal modules from `pub mod` to `pub(crate) mod` in all `lib.rs` files across the workspace. Used `pub use` to explicitly define the public API. Fixed resulting compilation errors by adjusting visibility internally where necessary.

🧱 Stability: Reduced coupling, stricter module boundaries, and better encapsulation.

🔬 Verification: Builds successfully, all tests pass, and strict separation enforced.

---
🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."
