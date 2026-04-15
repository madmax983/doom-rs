**[Optimized HashMap Lookups in Texture Caches]
**Learning:** Replaced `String` keys with `LumpName` in `SpriteCache`, `PatchCache`, and `FlatCache` to avoid hasher initialization overhead and heap allocations during cache lookups. `LumpName` implements `Copy` and avoids `String` allocations, saving memory and reducing time spent hashing.
**Action:** Always prefer `LumpName` over `String` for dictionary keys in WAD lookup tables to prevent hot-path `.to_uppercase()` and `String` allocations.
