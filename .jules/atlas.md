**[Shared Audio Sync Types]**
**Tangle:** The `doom-app` crate had hard-coded references to `Arc<Mutex<SfxMixer>>` and `Arc<Mutex<MidiPlayer>>` throughout the code to access audio driver state. This directly broke when `loom` feature flag was used for concurrent testing in `doom-audio` crate since `loom::sync::Arc` != `std::sync::Arc`. This caused build failures when trying to compile `doom-app` while tests were active in the workspace or during normal `all-features` checks.
**Blueprint:** Abstracted `Arc<Mutex<T>>` into `pub type SharedSfxMixer` and `pub type SharedMidiPlayer` within `doom-audio/src/driver.rs`. Removed hardcoded types from `doom-app` variables and functions, letting type inference (`.clone()`) and the new aliases provide an opaque boundary. This fixes the compiler error and enforces looser coupling.

**[Hide internal renderer modules]**
**Tangle:** The `doom-renderer` crate leaked internal constant modules `automap_colors` and `menu_colors` as `pub mod`s in `automap.rs` and `menu_render.rs` and re-exported them in `lib.rs` even though they are only used internally within the renderer for the GUI.
**Blueprint:** Altered the visibility of these modules to `pub(crate)` and removed them from the crate's `pub use` interface. Added `#[allow(dead_code)]` to bypass rustc warning of `pub` items missing callers since they are now module-private yet some constants are unimplemented but part of a well-defined palette standard. This ensures strict internal encapsulation.

**[Deduplicate ticinput_to_ticcmd helper]**
**Tangle:** The `doom-app` crate contained duplicate implementations of the `ticinput_to_ticcmd` helper function in `src/main.rs` and `src/net_mode.rs`.
**Blueprint:** Removed the duplicate implementation from `src/main.rs` and updated references in `main.rs` and `demo_mode.rs` to use the single source of truth at `crate::net_mode::ticinput_to_ticcmd`, removing a DRY violation and ensuring better code maintainability.

**[Extract MobjKind to Shared Primitives]**
**Tangle:** The `MobjKind` enum was defined in `doom-game`, but widely used in presentation crates like `doom-app` for rendering glyphs and managing audio playback. This caused UI components to depend directly on the entire game engine crate just for a basic enum definition, creating tight coupling between rendering and core game state.
**Blueprint:** Extracted `MobjKind` into `doom-types/src/mobj_kind.rs` and added its dependencies (`strum`, `strum_macros`) to `doom-types/Cargo.toml`. `doom-game` and `doom-app` imports were updated to reference the primitive from the shared types crate, creating a clean dependency hierarchy where both presentation and logic rely on foundational types.

**[Stop TicCmd Re-export Leak]**
**Tangle:** `doom-game/src/lib.rs` was unnecessarily re-exporting `doom_types::{TicCmd, bt}` via `pub use`. This caused presentation and utility crates like `doom-app` and `doom-demo` to depend on the game engine `doom-game` just to use basic shared data types. This violated the boundary isolation.
**Blueprint:** Removed the re-export from `doom-game` and updated `doom-demo` and `doom-app` to import `TicCmd` and `bt` constants directly from the foundational `doom-types` crate. This enforces cleaner dependency arrows where higher-level crates fetch domain primitives directly from the shared types crate rather than pulling them through the game logic.
**[Fix clone on copy clippy warning]
**Tangle:** The  crate had a  on  which implements .
**Blueprint:** Removed the  method call and let the compiler figure it out, as it implements . It was causing a clippy warning.
**[Fix clone on copy clippy warning]**
**Tangle:** The `doom-renderer` crate had a `clone()` on `LumpName` which implements `Copy`.
**Blueprint:** Removed the `clone()` method call and let the compiler figure it out, as it implements `Copy`. It was causing a clippy warning.
**[Fix doom-net doctest resolution failure]**
**Tangle:** The doctests in `doom-net` were failing during `cargo test --workspace` because they attempted to `use doom_net::TicCmd` in example snippets. The `doom-net` crate did not export `TicCmd` at its root level (and properly shouldn't, to avoid re-export leaks), causing the doctest compiler to error out with `E0432: unresolved import doom_net::TicCmd`.
**Blueprint:** Updated the doctest strings inside `crates/doom-net/src/input_log.rs`, `crates/doom-net/src/packet.rs`, and `crates/doom-net/src/rollback.rs`. Replaced the direct inclusion of `TicCmd` from `doom_net` with an explicit `use doom_types::TicCmd;` line for each example, which is the foundational crate holding the core data structure. This aligns the examples with correct dependency structures and prevents compile failures during tests.

**[Stop AutomapState Re-export Leak]**
**Tangle:** `doom-renderer/src/lib.rs` was unnecessarily re-exporting `doom_game::AutomapState` via `pub use`. This caused presentation crates like `doom-app` to depend on the renderer crate just to use basic game state data. This violated the boundary isolation.
**Blueprint:** Removed the re-export from `doom-renderer` and updated `doom-app` to import `AutomapState` directly from the game engine crate `doom-game`.
**[Enforce module boundaries in doom-app]**
**Tangle:** The `doom-app` crate contained many internal structures, functions, enums, and constants (e.g. `DoomGame`, `Console`, `AudioSystem`, etc) that were marked as `pub`, leaking implementation details to the outside despite the app being a binary orchestrator and having no downstream dependants.
**Blueprint:** Modified the visibility of all internal components in `doom-app` from `pub` to `pub(crate)` where applicable (like `DoomGame`, `DemoRecordingWrapper`, `Console`, `AudioSystem`, etc). Removed leaky `pub use` from internal modules.

**[Extract WeaponType and AmmoType to Shared Primitives]**
**Tangle:** The `WeaponType` and `AmmoType` enums (along with `WEAPON_AMMO`) were defined in `doom-game/src/player.rs`, but widely used in presentation crates like `doom-app` for rendering HUD elements and managing audio playback. This caused UI components to depend directly on the entire game engine crate just for basic enum definitions, creating tight coupling between rendering and core game state.
**Blueprint:** Extracted `WeaponType` and `AmmoType` into `doom-types/src/weapons.rs`. `doom-game/src/player.rs` now re-exports them for backwards compatibility within the game logic crate, and `doom-app` was updated to import these primitives directly from the shared types crate. This creates a clean dependency hierarchy where presentation relies on foundational types instead of game logic.

**[Stop WeaponType Re-export Leak]**
**Tangle:** `doom-game/src/lib.rs` and `doom-game/src/player.rs` were unnecessarily re-exporting `doom_types::weapons::{AmmoType, WeaponType, WEAPON_AMMO}` via `pub use`. This caused presentation crates like `doom-app` and `doom-renderer` to depend on the game engine `doom-game` just to use basic shared weapon data types. This violated the boundary isolation.
**Blueprint:** Removed the re-exports from `doom-game` and updated `doom-app`, `doom-renderer`, and `doom-game` itself to import `WeaponType`, `AmmoType`, and `WEAPON_AMMO` directly from the foundational `doom-types` crate. This enforces cleaner dependency arrows where higher-level crates fetch domain primitives directly from the shared types crate rather than pulling them through the game logic.

**[Extract NUM_POWERS and NUM_PSPRITES to Shared Primitives]**
**Tangle:** The `NUM_POWERS` and `NUM_PSPRITES` constants were defined in `doom-game/src/player.rs`, causing UI and game engine components to depend directly on the entire game engine crate just for basic limits.
**Blueprint:** Extracted `NUM_POWERS` and `NUM_PSPRITES` into `doom-types/src/limits.rs`. `doom-game/src/player.rs` now re-exports them for backwards compatibility within the game logic crate, and `doom-game` components that relied on them were updated to import these primitives directly from the shared types crate. This creates a clean dependency hierarchy where presentation relies on foundational limits instead of game logic.
**[Extract GameState sub-components to reduce state.rs bloat]**
**Tangle:** The `doom-game/src/state.rs` file was a huge "God Struct" holding everything from `LevelStats` to `SectorMovers` and `SoundPropagation`, creating a sprawling mess of responsibilities.
**Blueprint:** Extracted `LevelStats` to `stats.rs`, `SectorMovers` and all related structures to `movers.rs`, and `SoundPropagation` to `sound_prop.rs`. Updated `state.rs` to import these components, significantly improving code cohesion and module boundaries.

**[Extract DoomRng to random.rs]**
**Tangle:** The `DoomRng` struct and its internal `RNG_TABLE` were defined inside `crates/doom-game/src/movers.rs`, despite `DoomRng` being a foundational engine randomness source used across many components (via `GameState`), causing unrelated files to implicitly depend on the `movers` module just to access rng components, creating low cohesion and breaking domain boundaries.
**Blueprint:** Extracted `DoomRng` and `RNG_TABLE` into `crates/doom-game/src/random.rs`, matching their responsibility domain. Updated `state.rs` and `savegame.rs` to import from `random` instead of `movers`, ensuring a clearer directed graph of dependencies.
