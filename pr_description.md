💡 What: Replaced an intermediate `Vec` allocation with direct appending to a pre-allocated vector during BSP traversal.
🎯 Why: The previous `ordered_subsector_segs` allocated a `Vec` using `.collect()` for every subsector processed during the rendering pass, creating thousands of unnecessary heap allocations per frame.
📊 Impact: Eliminates a `Vec` allocation for every subsector in the view cone per frame.
🔬 Measurement: Run `cargo bench` on `renderer_bench` to observe reduced allocator pressure.
