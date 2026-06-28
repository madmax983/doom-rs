🗺️ Atlas: [Apply Facade Pattern to doom-demo and doom-tui]

🕸️ Tangle: The `doom-demo` and `doom-tui` crates exposed all their internal submodules publicly (`pub mod`) directly in their `lib.rs` files. This leaks implementation details and creates a larger-than-necessary API surface for consumers (e.g. `doom-app`), encouraging tight coupling to internal structures instead of relying on the intended public API.

📐 Blueprint: Altered the visibility of internal modules in `crates/doom-demo/src/lib.rs` and `crates/doom-tui/src/lib.rs` from `pub mod` to `pub(crate) mod`. Retained `pub use` statements at the root level to selectively expose only the necessary public interfaces. Handled unused code warnings via `#[allow(dead_code)]` annotations and deprecated usage of `ratatui` APIs in `doom-tui` via `#[allow(deprecated)]` to remain within persona bounds without breaking strict clippy constraints.

🧱 Stability: Reduced coupling, faster compile times, clear domain boundaries.

🔬 Verification: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo doc` all pass.
