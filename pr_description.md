🕸️ Tangle: The `KEY_*` and `PW_*` (powers) constants were defined in `doom-game/src/player.rs`, creating implicit, tight coupling between the core game engine crate and presentation crates like `doom-app` and `doom-renderer` that relied on them to display HUD indicators or power-ups. This leaked domain primitives through the game state module.

📐 Blueprint: Extracted the key and power-up constants into `doom-types/src/keys.rs` and `doom-types/src/powers.rs`. Updated callers in `doom-game`, `doom-app`, and `doom-renderer` to consume these foundational primitives directly from `doom-types`, rather than coupling to the `player` state module. Also fixed a `c.set_skip(true)` deprecation warning in `doom-tui` to keep the build green.

🧱 Stability: Reduced coupling, faster compile times. The presentation logic now depends only on shared primitive values, rather than the entire `doom-game` orchestration crate.

🔬 Verification: Builds successfully, strict separation enforced. Tested with `cargo check`, `cargo test`, and `cargo clippy`.
