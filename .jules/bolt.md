**Pre-allocated Vectors in render loop**
**Learning:** Found dynamically sizing vector `masked_columns` inside `crates/doom-renderer/src/render.rs` core rendering loop, causing unnecessary dynamic allocations when logical bounds are known (i.e. SCREEN_W).
**Action:** Replace `Vec::new()` with `Vec::with_capacity(...)` for frequently called render loop logic to minimize allocations.
