Title: 🗺️ Atlas: [Fix ratatui::buffer::Cell::set_skip Deprecation Warning]

🕸️ Tangle: The `doom-tui` crate used the deprecated method `Cell::set_skip(true)` during sixel/image rendering, which was causing a `cargo clippy` error when compiling with `--all-targets --all-features`.
📐 Blueprint: Replaced `set_skip(true)` with `set_diff_option(ratatui::buffer::CellDiffOption::Skip)`. This modernizes the `ratatui` usage and clears the `cargo clippy` warnings.
🧱 Stability: Maintained API compatibility and improved code hygiene by adhering to modern `ratatui` conventions.
🔭 Verification: Builds successfully, all tests pass, and warnings are resolved.
