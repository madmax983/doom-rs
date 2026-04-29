🕸️ Tangle: The `doom-app` crate maintained a separate `CheatDetector` structure inside `cheats.rs` while `doom-game` already had a fully implemented `CheatBuffer` and cheat detection logic (`check_cheats`, `apply_cheat`). This resulted in dead code warnings and a violation of the DRY principle, keeping two separate sources of truth for game cheats.
📐 Blueprint: Removed `crates/doom-app/src/cheats.rs` completely and integrated the console/chat cheat detection directly into `doom-game`'s `CheatBuffer` implementation inside `doom-app/src/main.rs`. Removed the `cheat_detector` field from the main game struct in favor of the existing `cheat_buffer`.
🧱 Stability: Reduced duplicate code and fixed clippy warnings, improving maintainability.
🔬 Verification: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo fmt --all` pass successfully.

**Assumptions**: I assumed that removing the `cheats.rs` logic in `doom-app` was the best structural improvement since it completely eliminated redundant code and `doom-game`'s `CheatBuffer` covered all functionality (including `IDDT`).
