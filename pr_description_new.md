🕸️ **Tangle:**
- The `doom-tui` crate used a deprecated ratatui method `ratatui::buffer::Cell::set_skip`, triggering warnings during the build. Attempting to suppress it using `.map(|c| c.set_skip(true))` then triggered a `clippy::option_map_unit_fn` warning.
- The `doom-app` crate lacked a module-level doc comment, causing `cargo doc` to fail when running with `-D missing_docs`.
- The `doom-renderer/src/sprite_clip.rs` file was lacking documentation, causing `cargo doc` to fail.

📐 **Blueprint:**
- Wrapped the deprecated `set_skip` call in `doom-tui/src/event_loop.rs` and `doom-tui/src/sixel.rs` with `#[allow(deprecated)]` and refactored the `.map` into an idiomatic `if let Some` block to satisfy clippy.
- Added `//! Main application crate for the Doom Engine.` to `doom-app/src/lib.rs`.
- Added missing documentation comments to `sprite_clip.rs` in `doom-renderer`.

🧱 **Stability:**
The workspace builds perfectly cleanly without any compiler warnings, clippy warnings, or documentation errors. This keeps our CI pipelines green and our codebase strictly compliant.

🔬 **Verification:**
Ran `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --workspace --no-deps --document-private-items` successfully.
