
**Use LumpName as Map keys to prevent allocations**
**Learning:** Lookups into string dictionaries using a sequence of characters usually causes overhead from String allocation or UTF-8 matching overhead. By using the new type `LumpName` (which wraps `[u8; 8]`) directly as a Map key instead of converting it to a `String`, we avoid all related overheads while preserving `Hash` and `Eq` correctness.
**Action:** Use fixed-size, byte-wrapping types like `LumpName` over `String` or `Vec<u8>` when defining lookup maps.
