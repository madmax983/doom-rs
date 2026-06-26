🛡️ Sentry: Replace `unwrap()` and `unwrap_err()` with `expect()` and `expect_err()`

🎯 Target: `crates/doom-map/src/analyzer.rs`, `crates/doom-map/src/lumps.rs`, `crates/doom-app/src/main.rs`, `crates/doom-net/src/transport.rs` and `crates/doom-tui/src/event_loop.rs` & `crates/doom-tui/src/sixel.rs`
💣 Risk: Obscured test failure context or uninformative panics.
🧪 Strategy: Replaced `.unwrap()` and `.unwrap_err()` with `.expect()` or `.expect_err()` providing explicit failure messages, and fixed deprecation warnings with `ratatui::buffer::CellDiffOption::Skip`.
🔬 Verification: `cargo test` and `cargo doc` have been successfully run.
