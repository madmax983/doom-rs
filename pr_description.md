🚮 Smell: The TUI crate is using the deprecated `ratatui::buffer::Cell::set_skip(true)` method, which triggers a `-D warnings` compilation failure.
✨ Solution: Replaced deprecated `set_skip(true)` calls with `set_diff_option(CellDiffOption::Skip)` across `event_loop.rs` and `sixel.rs`, ensuring compatibility with recent `ratatui` updates.
🧼 Benefit: Resolves the build error and keeps the codebase idiomatically aligned with the latest `ratatui` APIs without changing runtime behavior.
🛡️ Verification: Tests passed. No logic changed. Compiled successfully with `cargo clippy`.
