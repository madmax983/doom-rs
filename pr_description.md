🕸️ Tangle: The `powers` and `psprite_slots` sub-modules were defined in `doom-game/src/player.rs` and unnecessarily re-exported. This caused UI components (`doom-app`) to depend directly on the game engine logic just for basic slot indices. Also resolved some deprecation warnings in `doom-tui`.

📐 Blueprint: Extracted `powers` and `psprite_slots` into `doom-types/src/powers.rs` as independent constants. Updated imports in `doom-app` and `doom-game` to import these primitives directly from the shared types crate, creating a clean dependency hierarchy where presentation relies on foundational types. Replaced deprecated `.set_skip()` usages in `doom-tui`.

🧱 Stability: Reduced coupling, faster compile times, clean separation of concerns.

🔬 Verification: Builds successfully, strict separation enforced. Tested with `cargo check`, `cargo test`, and `cargo clippy`.
