🕸️ Tangle: The `Skill` enum was defined in `doom-game/src/spawn.rs` to represent difficulty levels for thing filtering. Simultaneously, a `SkillLevel(u8)` wrapper struct was defined in `doom-types/src/primitives.rs` with `pub const` definitions. This redundancy caused domain confusion, violating boundaries since presentation crates like `doom-app` and `doom-demo` needed access to skill level definitions.

📐 Blueprint: Extracted the strongly-typed `Skill` enum into `doom-types/src/skill.rs`. Removed the redundant `SkillLevel` wrapper from `doom-types/src/primitives.rs`. Updated `doom-game`, `doom-app`, and `doom-demo` to import and use the new, single source of truth `doom_types::Skill`.

🧱 Stability: Reduced coupling by ensuring presentation and utility crates rely on shared primitives instead of the core engine game state crate for basic enum definitions, faster compile times.

🔬 Verification: Builds successfully, strict separation enforced. Tested with `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo fmt --all`.
