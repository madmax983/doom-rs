⚡ Bolt: [Reduce heap allocations in main render loop]

💡 What: Changed the initialization of `masked_columns` from `Vec::new()` to `Vec::with_capacity(SCREEN_W)` in the main render loop.
🎯 Why: `masked_columns` stores the masked midtexture columns generated in the main wall pass. Pre-allocating it with a logical upper bound avoids dynamic heap resizing and multiple re-allocations per frame when storing masked spans.
📊 Impact: Eliminates `masked_columns` reallocation overhead in the core rendering hot path.
🔬 Measurement: Run `cargo clippy --all-targets --all-features` and `cargo test` to ensure performance check constraints pass.
