🗺️ Atlas: Extract Skill and GameMode to Shared Primitives

🕸️ Tangle: The `Skill` and `GameMode` enums were defined in `doom-game/src/spawn.rs` and widely used by presentation crates like `doom-app` and `doom-renderer`, creating tight coupling to the game engine for basic runtime configurations. Additionally, `doom-types/src/primitives.rs` defined a duplicate concept `SkillLevel` as a newtype, violating DRY.

📐 Blueprint: Extracted `Skill` and `GameMode` into `doom-types/src/primitives.rs`, replacing the redundant `SkillLevel` struct. Removed the re-exports from `doom-game` and updated presentation crates to import them directly from the shared types crate. Maintained `strum_macros` definition per repository guidelines.

🧱 Stability: Reduced coupling, resolved DRY violations, and eliminated re-export leaks from `doom-game`.

🔬 Verification: Builds successfully, strict separation enforced. `cargo clippy` and `cargo test` pass.
