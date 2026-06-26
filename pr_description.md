💡 **What:** Replaced `Vec::new()` with `Vec::with_capacity(128)` for `masked_columns` initialization inside `render_level_with_view_height_and_extra_light_and_fixed_colormap`.
🎯 **Why:** The `masked_columns` vector was reallocated repeatedly on the hot path during each frame render when encountering two-sided lines and masked geometry.
📊 **Impact:** Eliminates several vector heap reallocations per frame when pushing masked columns, providing a zero-cost initialization boost.
🔬 **Measurement:** Run `cargo test` and observe identical rendering output with lower memory pressure per frame.
