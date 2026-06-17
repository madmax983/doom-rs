🎯 **Target:** `toggle_graphics_protocol` in `crates/doom-tui/src/event_loop.rs`

💣 **Risk:** The previous lack of coverage on `toggle_graphics_protocol` meant we were not properly validating its branches or return values, which could result in a UI mode failure to update gracefully. We also had deprecated method `set_skip` that needed fixing.

🧪 **Strategy:**
- Added a robust unit test for `toggle_graphics_protocol` that forces a simulated initialization state by leveraging mocked properties of the Picker to hit branching fallback logic properly.
- Switched deprecated `set_skip(true)` calls to `set_diff_option(ratatui::buffer::CellDiffOption::Skip)` which fixes compilation deprecations warnings and ensures safe terminal transitions.

🔬 **Verification:** `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --package doom-tui`
