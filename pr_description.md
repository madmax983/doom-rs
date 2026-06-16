🕸️ Tangle: `doom-game/src/state.rs` and `doom-game/src/lib.rs` are re-exporting (`pub use`) `SoundPropagation`, `SoundRequest` and many structs/enums from `movers.rs` into `crate::state`. This is a "Re-export Leak". It causes downstream dependent modules and crates to import internal mover logic and sound types from `crate::state` rather than their actual defining modules (`crate::movers` and `crate::sound_prop`), creating a tangled dependency graph and blurring the boundaries of `state.rs`.

📐 Blueprint: Removed the `pub use` re-exports of `sound_prop` and `movers` from `state.rs`. Updated all references across `doom-game` to import these types directly from their true defining modules (`crate::sound_prop` and `crate::movers`). Replaced the wildcard `use crate::movers::*` with an explicit `use crate::movers::SectorMovers` in `state.rs` for clear encapsulation.

🧱 Stability: Reduced coupling, clearer module boundaries, and a cleaner dependency graph.

🔬 Verification: Builds successfully (`cargo check`), tests pass (`cargo test`), and strict separation is enforced.
