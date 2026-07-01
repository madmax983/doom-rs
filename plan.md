1. **Refactor `SaveError` in `doom-app/src/savegame.rs` to wrap `doom_game::savegame::SaveError`:**
   - Modify the `SaveError` enum in `crates/doom-app/src/savegame.rs` to wrap the engine's `SaveError` rather than redefining its variants.
   - Specifically, replace `BadMagic`, `BadVersion`, `Truncated`, and `UnsupportedVanillaDsg` variants with a single `Engine(#[from] doom_game::savegame::SaveError)` variant.
   - Retain the `Io` and `FormatMismatch` variants since they are specific to the application's file I/O layer.
2. **Remove the `From<doom_game::savegame::SaveError>` block:**
   - The manual `From` implementation is no longer necessary as `thiserror` handles the `#[from]` derivation for the new `Engine` variant.
3. **Update Error handling at call sites:**
   - Update tests in `doom-app/src/savegame.rs` that check for specific engine errors to match against the wrapped `SaveError::Engine` variant.
4. **Complete pre-commit steps:**
   - Complete pre commit steps to make sure proper testing, verifications, reviews and reflections are done.
5. **Submit the change.**
