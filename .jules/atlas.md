**[Shared Audio Sync Types]**
**Tangle:** The `doom-app` crate had hard-coded references to `Arc<Mutex<SfxMixer>>` and `Arc<Mutex<MidiPlayer>>` throughout the code to access audio driver state. This directly broke when `loom` feature flag was used for concurrent testing in `doom-audio` crate since `loom::sync::Arc` != `std::sync::Arc`. This caused build failures when trying to compile `doom-app` while tests were active in the workspace or during normal `all-features` checks.
**Blueprint:** Abstracted `Arc<Mutex<T>>` into `pub type SharedSfxMixer` and `pub type SharedMidiPlayer` within `doom-audio/src/driver.rs`. Removed hardcoded types from `doom-app` variables and functions, letting type inference (`.clone()`) and the new aliases provide an opaque boundary. This fixes the compiler error and enforces looser coupling.

**[Hide internal renderer modules]**
**Tangle:** The `doom-renderer` crate leaked internal constant modules `automap_colors` and `menu_colors` as `pub mod`s in `automap.rs` and `menu_render.rs` and re-exported them in `lib.rs` even though they are only used internally within the renderer for the GUI.
**Blueprint:** Altered the visibility of these modules to `pub(crate)` and removed them from the crate's `pub use` interface. Added `#[allow(dead_code)]` to bypass rustc warning of `pub` items missing callers since they are now module-private yet some constants are unimplemented but part of a well-defined palette standard. This ensures strict internal encapsulation.

## 2024-03-26 - Removed duplicate `savegame` modules
**Tangle:** Both `doom-app` and `doom-game` had their own `savegame` modules, and `doom-app`'s version was essentially doing exactly what `doom-game`'s did but with duplication. `doom-game` already implements saving and loading `GameState` using its own byte format, but `doom-app` was using `bincode` directly to save the exact same data to disk, leading to duplication and an inconsistent save format.
**Blueprint:** Removed `crates/doom-app/src/savegame.rs` and modified `crates/doom-app/src/main.rs` to use the serialization mechanisms correctly provided by `doom-game::savegame` (`save_game` and `load_game`).
