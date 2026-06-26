📖 Chapter: The `DoomEventLoop` module and `sixel` rendering
🔦 Insight: Added executable doctests for `DoomEventLoop` to show how to initialize and run the terminal game loop. Also updated `ratatui` APIs from the deprecated `set_skip(true)` to `set_diff_option(CellDiffOption::Skip)` to fix clippy warnings and keep the documentation and codebase clean.
🧪 Example: Added 1 executable doctest to `DoomEventLoop`.
🖼️ Preview: Tested via `cargo doc`.
