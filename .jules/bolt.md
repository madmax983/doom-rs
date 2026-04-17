
**Use LumpName as Map keys to prevent allocations**
**Learning:** Lookups into string dictionaries using a sequence of characters usually causes overhead from String allocation or UTF-8 matching overhead. By using the new type `LumpName` (which wraps `[u8; 8]`) directly as a Map key instead of converting it to a `String`, we avoid all related overheads while preserving `Hash` and `Eq` correctness.
**Action:** Use fixed-size, byte-wrapping types like `LumpName` over `String` or `Vec<u8>` when defining lookup maps.

**[SmallVec for Hitscan Intercepts and Sector Traversal]
**Learning:** Returning `Vec<T>` or mutating `&mut Vec<T>` on hot paths like `adjacent_sectors` and `p_line_attack` causes many tiny heap allocations. `smallvec` avoids allocations for arrays up to a chosen size, dropping them on the stack. Changing `Vec::new()` to `SmallVec::new()` and setting up a sensible default capacity (e.g. 16 for `HitscanIntercept` or 8 for adjacent sectors) speeds up traversal and weapon firing significantly.
**Action:** When a function collects a small, bounded number of items (like adjacent level geometry or raycast intercepts) and is called very frequently, use `smallvec::SmallVec` instead of `Vec`.
**[VecDeque for rolling logs]**
**Learning:** Fixed-capacity rolling logs built with `Vec` must call `.remove(0)` when full, shifting all elements `O(N)` times. Using `std::collections::VecDeque` provides `O(1)` `.pop_front()`, eliminating that overhead entirely.
**Action:** Use `VecDeque` instead of `Vec` for small fixed-capacity ring buffers or scrolling text logs.

**[Lazily evaluated impl Iterator instead of Vec]**
**Learning:** Functions that query lists of things based on a condition (like `sectors_by_tag`) often `.collect()` into a `Vec` for convenience, but the caller usually just iterates over them immediately. Returning an `impl Iterator` avoids allocating the intermediate `Vec`.
**Action:** When filtering a collection to loop over the matches, return `impl Iterator` instead of `.collect::<Vec<_>>()` to eliminate heap allocations.
**[Console History Optimization]
**Learning:** Using  with  for fixed-capacity rolling logs introduces an (N)$ shift penalty on every eviction.  is the mathematically correct structure.
**Action:** Replace  with  and use  for rolling logs to ensure (1)$ updates and zero initial resize allocations.
**[Console History Optimization]**
**Learning:** Using `Vec` with `remove(0)` for fixed-capacity rolling logs introduces an O(N) shift penalty on every eviction. `VecDeque` is the mathematically correct structure.
**Action:** Replace `Vec` with `VecDeque::with_capacity(max)` and use `pop_front()` for rolling logs to ensure O(1) updates and zero initial resize allocations.
