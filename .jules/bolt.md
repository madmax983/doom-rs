
**Use LumpName as Map keys to prevent allocations**
**Learning:** Lookups into string dictionaries using a sequence of characters usually causes overhead from String allocation or UTF-8 matching overhead. By using the new type `LumpName` (which wraps `[u8; 8]`) directly as a Map key instead of converting it to a `String`, we avoid all related overheads while preserving `Hash` and `Eq` correctness.
**Action:** Use fixed-size, byte-wrapping types like `LumpName` over `String` or `Vec<u8>` when defining lookup maps.

**[SmallVec for Hitscan Intercepts and Sector Traversal]
**Learning:** Returning `Vec<T>` or mutating `&mut Vec<T>` on hot paths like `adjacent_sectors` and `p_line_attack` causes many tiny heap allocations. `smallvec` avoids allocations for arrays up to a chosen size, dropping them on the stack. Changing `Vec::new()` to `SmallVec::new()` and setting up a sensible default capacity (e.g. 16 for `HitscanIntercept` or 8 for adjacent sectors) speeds up traversal and weapon firing significantly.
**Action:** When a function collects a small, bounded number of items (like adjacent level geometry or raycast intercepts) and is called very frequently, use `smallvec::SmallVec` instead of `Vec`.
**[Console History Optimization]
**Learning:** Using  with  for fixed-capacity rolling logs introduces an (N)$ shift penalty on every eviction.  is the mathematically correct structure.
**Action:** Replace  with  and use  for rolling logs to ensure (1)$ updates and zero initial resize allocations.
**[Console History Optimization]**
**Learning:** Using `Vec` with `remove(0)` for fixed-capacity rolling logs introduces an O(N) shift penalty on every eviction. `VecDeque` is the mathematically correct structure.
**Action:** Replace `Vec` with `VecDeque::with_capacity(max)` and use `pop_front()` for rolling logs to ensure O(1) updates and zero initial resize allocations.

**[LumpName in HashMap Keys]**
**Learning:** Lookups into string dictionaries using a sequence of characters usually causes overhead from String allocation or UTF-8 matching overhead. Specifically,  forces a heap-allocated  on every single cache lookup. By using  (which parses into an 8-byte stack value), we eliminate a very common heap allocation on the rendering hot path.
**Action:** Use fixed-size, byte-wrapping types like  over  or  when defining lookup maps. When passing these keys to APIs expecting string slices, convert them back using .

**[LumpName in HashMap Keys]**
**Learning:** Lookups into string dictionaries using a sequence of characters usually causes overhead from String allocation or UTF-8 matching overhead. Specifically, `to_uppercase()` forces a heap-allocated `String` on every single cache lookup. By using `LumpName` (which parses into an 8-byte stack value), we eliminate a very common heap allocation on the rendering hot path.
**Action:** Use fixed-size, byte-wrapping types like `LumpName` over `String` or `Vec<u8>` when defining lookup maps. When passing these keys to APIs expecting string slices, convert them back using `.as_str()`.
**[format! macro allocations in hot paths]**
**Learning:** Using `format!` in a hot path (like rendering loops) forces heap allocations for every call. Even for simple strings like `"STTNUM1"`, it allocates and deallocates a `String`.
**Action:** Replace `format!` macros generating bounded sequences of strings (like digit names `0-9` or key indices) with static string arrays and index into them (e.g., `["STTNUM0", "STTNUM1", ...][digit as usize]`). Ensure bounds are respected or clamped. Avoid `to_string()` for digits in hot paths as well.
