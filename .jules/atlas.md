**[Shared Audio Sync Types]**
**Tangle:** The `doom-app` crate had hard-coded references to `Arc<Mutex<SfxMixer>>` and `Arc<Mutex<MidiPlayer>>` throughout the code to access audio driver state. This directly broke when `loom` feature flag was used for concurrent testing in `doom-audio` crate since `loom::sync::Arc` != `std::sync::Arc`. This caused build failures when trying to compile `doom-app` while tests were active in the workspace or during normal `all-features` checks.
**Blueprint:** Abstracted `Arc<Mutex<T>>` into `pub type SharedSfxMixer` and `pub type SharedMidiPlayer` within `doom-audio/src/driver.rs`. Removed hardcoded types from `doom-app` variables and functions, letting type inference (`.clone()`) and the new aliases provide an opaque boundary. This fixes the compiler error and enforces looser coupling.

**[Hide internal renderer modules]**
**Tangle:** The `doom-renderer` crate leaked internal constant modules `automap_colors` and `menu_colors` as `pub mod`s in `automap.rs` and `menu_render.rs` and re-exported them in `lib.rs` even though they are only used internally within the renderer for the GUI.
**Blueprint:** Altered the visibility of these modules to `pub(crate)` and removed them from the crate's `pub use` interface. Added `#[allow(dead_code)]` to bypass rustc warning of `pub` items missing callers since they are now module-private yet some constants are unimplemented but part of a well-defined palette standard. This ensures strict internal encapsulation.

**[Implement Facade pattern across workspace]**
**Tangle:** Internal sub-modules like `doom_types::limits`, `doom_map::lumps`, and `doom_game::player` were publicly accessible (`pub mod`), leaking implementation details and leading to tight coupling with deep namespace imports like `doom_types::limits::MAX_AMMO` or `doom_map::lumps::FLAG_BLOCKING`.
**Blueprint:** Modified all `lib.rs` files across the workspace to use `pub(crate) mod` for internal modules. Enforced the Facade pattern by exclusively re-exporting necessary items via `pub use` directly at the crate root, forcing downstream crates to import `doom_types::MAX_AMMO` or `doom_map::FLAG_BLOCKING`.
