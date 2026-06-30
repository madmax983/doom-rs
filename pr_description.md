💡 What: Pre-allocate the `masked_columns` vector with `Vec::with_capacity(SCREEN_W)`.
🎯 Why: The `masked_columns` vector is allocated dynamically every frame in the core `render_level` hot path using `Vec::new()`. Pre-allocating it using `SCREEN_W` (the logical upper bound of on-screen masked columns per frame without overdraw) eliminates a heap allocation per frame.
📊 Impact: Eliminates per-frame heap allocations related to masked column rendering, improving rendering performance and reducing memory fragmentation.
🔬 Measurement: Run `cargo bench` and observe reduced allocation churn and faster execution in the rendering loop.
