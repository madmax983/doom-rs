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
**Eliminate Per-Ray Heap Allocations**
**Learning:** Instantiating `Vec::with_capacity` inside a hot-path function like `trace_ray` creates a heap allocation on every single ray cast. Even if the capacity is small, this overhead compounds significantly during LOS checks or shotgun blasts.
**Action:** Replace `Vec::new()` or `Vec::with_capacity()` with `smallvec::SmallVec` for temporary buffers that are typically small and short-lived. This keeps the data entirely on the stack for the vast majority of cases, resulting in zero-cost abstraction.
## 2024-04-28 - Zero-Cost Menu String Rendering
**Learning:** `clippy` correctly points out `dead_code` issues, but using `&str.chars().take(N).collect::<String>()` dynamically on hot loops allocating heap memory won't be caught by clippy. You can use standard `text.lines()` alongside tracking characters manually, using `ch.encode_utf8(&mut buf)` to translate `char` back to an ad-hoc byte buffer for functions that take `&str`, avoiding string allocations.
**Action:** When drawing strings dynamically (progressive reveals), never collect to `String`. Use line or char iterators and map individual characters to stack-allocated `[u8; 4]` buffers for API compatibility.
**SmallVec for Walk Lines Allocation**
**Learning:** `Vec::new()` is heavily used during collision detection on the hot loop (e.g. `walk_lines.sort_by`). Replacing this with `smallvec::SmallVec` stops dynamic allocations for small intersection arrays.
**Action:** Use `smallvec::SmallVec<[T; N]>` where small static allocations cover 99% of cases on performance-critical paths.

**Refactoring savegame apply**
**Learning:** Re-assigning an existing struct instance using a pointer by performing a deep clone `*gs = payload.state.clone();` when the original owner is discarded generates huge memory allocations.
**Action:** Remove `.clone()` and consume the parameter directly (by removing the reference flag `&`) so Rust transfers ownership without copying large values, `*gs = payload.state;`.
