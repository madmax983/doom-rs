🎻 Bard: [Resolved missing docs and ratatui deprecation warnings]

📖 Chapter: `doom-app` crate and `doom-tui` rendering logic
🔦 Insight: Removed the empty `lib.rs` file in `doom-app` to resolve `-D missing_docs` warnings, correctly identifying it as a pure binary crate. Fixed deprecated `Cell::set_skip` usages in `doom-tui` by migrating to the new `CellDiffOption::Skip` API to keep terminal UI logic cleanly aligned with ratatui updates.
🧪 Example: Documentation rendering now succeeds without `-D warnings` failing the build on the library root. Terminal differential skips continue working exactly as before.
🖼️ Preview: No UI changes, but the build log is clean again!
