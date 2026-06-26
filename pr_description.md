⚒️ Forge: Replace deprecated set_skip with set_diff_option

🚮 Smell: `ratatui::buffer::Cell::set_skip(true)` is deprecated in ratatui 0.30+, raising `clippy` errors.
✨ Solution: Replaced deprecated calls with `set_diff_option(ratatui::buffer::CellDiffOption::Skip)` in `event_loop.rs` and `sixel.rs`, taking care to import `CellDiffOption`.
🧼 Benefit: Resolves the build error and conforms to the latest idiomatic ratatui API guidelines.
🛡️ Verification: Tests passed. No logic changed.
