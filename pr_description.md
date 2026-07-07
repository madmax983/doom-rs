💡 What: Replaced `let mut masked_columns = Vec::new();` with `Vec::with_capacity(128)` inside the core rendering loop (`render_level_with_view_height_and_extra_light_and_fixed_colormap`).

🎯 Why: The `masked_columns` vector dynamically grows to hold metadata for every visible sprite column and 2-sided mid-texture column on screen. Since it was initialized without capacity, it triggered multiple heap reallocations per frame on the hottest execution path in the engine.

📊 Impact: Reduces heap allocations by providing enough pre-allocated capacity (128 elements) to handle typical sprite/mid-texture density per frame without dynamic reallocation. This lowers memory pressure and speeds up the main render thread.

🔭 Measurement: Run `cargo bench -p doom-renderer` or profile frame rendering times.
