💡 What: Switched `Vec::new()` to `Vec::with_capacity(SCREEN_W)` for `masked_columns` in the main renderer.
🎯 Why: Pre-allocating this temporary vector eliminates per-frame dynamic heap allocations on the hot rendering path, as `SCREEN_W` is the logical bound for drawn columns.
📊 Impact: Eliminates multiple heap reallocations per frame when drawing transparent or masked textures.
🔬 Measurement: Run `cargo test` and `cargo bench "renderer_bench"`.
