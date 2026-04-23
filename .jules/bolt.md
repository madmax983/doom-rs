## 2025-02-12 - Performance Improvements in doom-app and doom-renderer
**Learning:** Returning references and slicing avoids `.clone()` where ownership isn't needed. Using pre-allocated buffers like `String::new()` and manually concatenating with `.push_str()` completely eliminates intermediate `.collect::<Vec<_>>()` chains and avoids heavy memory allocations, and using the `std::collections::hash_map::Entry` API prevents duplicate HashMap lookups.
**Action:** Use references and borrowing semantics whenever the data outlives its scope. Pre-allocate `Vec` and `String` with `with_capacity()` or just iteratively write to them, avoiding `.collect()` or multiple `.join()`s, minimizing heap allocations. Use `.entry()` for single-pass HashMap access.
**[Avoid Reallocations in Game Loop]
**Learning:** Pre-allocating `Vec` capacity based on screen/grid dimensions inside the main rendering loop dramatically reduces heap reallocations on a very hot path.
**Action:** When creating a `Vec` that collects on-screen entities every frame, estimate maximum capacity instead of using `Vec::new()`.
