## 2025-02-12 - Performance Improvements in doom-app and doom-renderer
**Learning:** Returning references and slicing avoids `.clone()` where ownership isn't needed. Using pre-allocated buffers like `String::new()` and manually concatenating with `.push_str()` completely eliminates intermediate `.collect::<Vec<_>>()` chains and avoids heavy memory allocations, and using the `std::collections::hash_map::Entry` API prevents duplicate HashMap lookups.
**Action:** Use references and borrowing semantics whenever the data outlives its scope. Pre-allocate `Vec` and `String` with `with_capacity()` or just iteratively write to them, avoiding `.collect()` or multiple `.join()`s, minimizing heap allocations. Use `.entry()` for single-pass HashMap access.

**Pre-allocating Vecs on Hot Paths based on Logical Limits**
**Learning:** Initializing `Vec::new()` in rendering loops that always append elements up to a mathematical bound (like `term_w * term_h` for screen rendering or `dx + dy + 1` for line tracing) causes repeated unnecessary heap allocations, degrading frame performance.
**Action:** Always calculate the physical or mathematical limit of elements to be pushed and use `Vec::with_capacity(limit)` to pre-allocate memory.

**Eliminate intermediate heap allocations using zero-cost abstractions**
**Learning:** The `DoomFramebufferWidget::render` logic was pre-computing rendering coordinates every frame into vectors (`Vec<usize>`). Since coordinate evaluation inside the render loop only relies on `cy` or `cx` variables natively bound to iterative loops (`0..term_h` and `0..term_w`), the overhead of dynamic arithmetic was zero cost compared to allocating three dynamic map `Vec` every frame.
**Action:** Replace `let map: Vec<_> = ... .collect()` intermediate maps with inline arithmetic, or wrap iterators inside outer loops to zip dynamically rather than iterating through statically generated heap arrays.


**[Sprite Clip Optimization]
**Learning:** Re-allocating empty `Vec`s 640 times per frame via `[const { Vec::new() }; 320]` causes performance drag in the hot render path. `SmallVec` or custom `ArrayVec` cannot easily be initialized using array repeat syntax without `const fn new()` implementation issues on external traits. A manual `ArrayVec`-like struct using `Default` inside `std::array::from_fn` works perfectly to maintain bounds without heap allocations.
**Action:** Use fixed-size stack arrays wrapped in custom tracker structs instead of raw `Vec`s for short-lived, bounds-known data structures instantiated repeatedly in hot loops.
