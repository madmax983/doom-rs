💡 What: Switched `masked_columns = Vec::new()` to `Vec::with_capacity(SCREEN_W)` in `render_scene`.
🎯 Why: Avoids heap reallocation overhead during the core rendering loop.
📊 Impact: Eliminates multiple allocations per frame on maps with many masked walls.
🔭 Measurement: Run bench `renderer_bench` or observe allocator activity during gameplay.
