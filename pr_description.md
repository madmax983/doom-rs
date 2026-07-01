**[Stop TestCanvas Re-export Leak]**
**Tangle:** `doom-game/src/lib.rs` was unnecessarily re-exporting `TestCanvas` via `pub use`, which was also marked as a `pub struct` in `doom-game/src/automap.rs`. `TestCanvas` is only used internally for testing the automap drawing. This violated boundary isolation by exposing testing utilities to the public API.
**Blueprint:** Modified the visibility of `TestCanvas` in `doom-game/src/automap.rs` from `pub` to `#[cfg(test)] pub(crate)` and gated its impl blocks with `#[cfg(test)]`. Removed the `TestCanvas` re-export from `doom-game/src/lib.rs`.
**Stability:** Cleaned up the public API by hiding internal test implementation details.
**Verification:** Code builds successfully and tests pass under `--all-targets --all-features`.
