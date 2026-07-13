🕸️ Tangle:
The `doom-game/src/state.rs` file was unnecessarily re-exporting internal structs and enums from `movers` and `sound_prop` modules using `pub use crate::movers::*;` and `pub use crate::sound_prop::{SoundPropagation, SoundRequest};`. This leaked internal details into the top-level `state` namespace, causing other files within `doom-game` (like `specials.rs`, `savegame.rs`, `weapon_fire.rs`, `combat.rs`) to depend on `state::SectorMovers` or `state::SoundRequest` instead of reaching for their proper domain boundaries (`movers::SectorMovers` and `sound_prop::SoundRequest`). This violates strict module boundaries and cohesion since `state.rs` should be an aggregator, not a pass-through module for internal utilities.

📐 Blueprint:
Removed the `pub use` re-exports from `doom-game/src/state.rs`, turning them into module-local imports (`use crate::movers::SectorMovers;` and `use crate::sound_prop::SoundPropagation;`). Updated all internal dependents across `doom-game` to import these domain types directly from `movers::` and `sound_prop::`.

🧱 Stability:
Reduced coupling inside the `doom-game` crate. Internal subsystems now import dependencies from their correct sources rather than routing through the centralized `state` module, flattening the internal dependency graph and enforcing clearer domains.

🔭 Verification:
Builds and tests successfully with `cargo check --workspace --all-features` and `cargo test --workspace --all-features`.
