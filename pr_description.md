💡 What: Replaced `Vec::new()` with `Vec::with_capacity(SCREEN_W)` for the `masked_columns` buffer inside the hot render loop.
🎯 Why: Instantiating an empty vector and then sequentially pushing masked columns into it dynamically caused repeated heap allocations every single frame. `SCREEN_W` is the physical limit of horizontal screen columns, so allocating it immediately is mathematically sound.
📊 Impact: Eliminates `n` dynamic heap allocations per frame where `n` is proportional to the number of masked walls rendered, moving `masked_columns` to a zero-cost initial allocation strategy.
🔬 Measurement: Run bench `cargo test` and `cargo bench` if criterion available, or use a profiler tracing `alloc::raw_vec` during heavy sprite or portal scenes.
