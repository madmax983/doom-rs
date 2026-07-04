💡 What: Replaced `let mut masked_columns = Vec::new()` with `Vec::with_capacity(SCREEN_W)` in `render_level_with_view_height_and_extra_light_and_fixed_colormap`.
🎯 Why: `masked_columns` is allocated in the hot loop of the wall pass for every frame. Using `Vec::new()` causes repeated dynamic heap allocations when columns are pushed during rendering. `SCREEN_W` is the logical upper bound for the number of screen columns.
📊 Impact: Removes initial dynamic memory reallocations for `masked_columns` per frame.
🔬 Measurement: Run `cargo clippy` and `cargo test` to verify changes, test rendering performance visually or by checking allocations.
