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
**[Enforce Facade pattern on `doom-game` simulation loops]**
**Tangle:** Internal game loops and states (like `automap`, `tic`, `weapon_fire`, `projectile`) were marked `pub mod` and fully exposed to the workspace, creating a high risk of "The Leaky Abstraction". External crates could bypass the game logic facade and depend on deep internal implementations.
**Blueprint:** Altered the visibility of completely internal loop and math modules to `pub(crate) mod` within `doom-game/src/lib.rs`. Retained explicit `pub use` exports for intended public types, ensuring the facade boundaries remain intact. Applied `#[allow(dead_code)]` to unused `weapon_refire_tics` to satisfy clippy while maintaining structural boundaries.
