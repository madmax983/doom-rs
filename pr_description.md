⚡ Bolt: Pre-allocate vector for masked columns in main render loop

💡 What: Changed the instantiation of `masked_columns` from `Vec::new()` to `Vec::with_capacity(SCREEN_W)`.
🎯 Why: `masked_columns` is allocated in the main rendering pass (`render_level_with_view_height_and_extra_light_and_fixed_colormap`), which is an extremely hot path called once per frame. Because we know that the maximum number of masked columns generated cannot exceed the screen width (`SCREEN_W` = 320), pre-allocating the vector eliminates dynamic memory reallocations during the frame loop.
📊 Impact: Removes heap reallocations for `masked_columns` per frame.
🔬 Measurement: Run `cargo test` and `cargo run` to verify rendering logic remains unaffected and frame consistency improves.
