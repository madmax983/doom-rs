💡 What: Replaced a redundant string `.clone()` with an immutable reference in `GameState::new` inside the episode reset path. Also fixed two unrelated deprecated warnings (`Cell::set_skip`) in the terminal UI render logic.
🎯 Why: Passing `&self.gs.level_name.clone()` creates an unnecessary heap allocation of a throwaway string. Rust's NLL handles this perfectly using the `&self.gs.level_name` reference.
📊 Impact: Eliminates 1 heap allocation per level restart/re-spawn sequence.
🔬 Measurement: Run `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` to verify no regressions.
