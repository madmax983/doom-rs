**[Optimized HashMap Lookups in Texture Caches]
**Learning:** Replaced `String` keys with `LumpName` in `SpriteCache`, `PatchCache`, and `FlatCache` to avoid hasher initialization overhead and heap allocations during cache lookups. `LumpName` implements `Copy` and avoids `String` allocations, saving memory and reducing time spent hashing.
**Action:** Always prefer `LumpName` over `String` for dictionary keys in WAD lookup tables to prevent hot-path `.to_uppercase()` and `String` allocations.

**Use LumpName as Map keys to prevent allocations**
**Learning:** Lookups into string dictionaries using a sequence of characters usually causes overhead from String allocation or UTF-8 matching overhead. By using the new type `LumpName` (which wraps `[u8; 8]`) directly as a Map key instead of converting it to a `String`, we avoid all related overheads while preserving `Hash` and `Eq` correctness.
**Action:** Use fixed-size, byte-wrapping types like `LumpName` over `String` or `Vec<u8>` when defining lookup maps.

**[SmallVec for Hitscan Intercepts and Sector Traversal]
**Learning:** Returning `Vec<T>` or mutating `&mut Vec<T>` on hot paths like `adjacent_sectors` and `p_line_attack` causes many tiny heap allocations. `smallvec` avoids allocations for arrays up to a chosen size, dropping them on the stack. Changing `Vec::new()` to `SmallVec::new()` and setting up a sensible default capacity (e.g. 16 for `HitscanIntercept` or 8 for adjacent sectors) speeds up traversal and weapon firing significantly.
**Action:** When a function collects a small, bounded number of items (like adjacent level geometry or raycast intercepts) and is called very frequently, use `smallvec::SmallVec` instead of `Vec`.
