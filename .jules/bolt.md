**[Optimized Cache HashMaps to use LumpName]
**Learning:** `LumpName` is an 8-byte array (`[u8; 8]`) which allows to use a fixed-size byte array key instead of `String` for Cache Dictionary Lookups.
**Action:** Replaced `HashMap<String, ...>` with `HashMap<LumpName, ...>` in `doom-renderer` (FlatCache, SpriteCache, PatchCache). Avoided unnecessary `String` heap allocations on hot paths.
