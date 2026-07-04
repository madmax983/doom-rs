🛡️ Sentry: Fix out-of-bounds blockmap iterator and add test coverage

🎯 Target: `doom-map::lumps::Blockmap::block_linedefs`
💣 Risk: Prevents an out-of-bounds `col` or `row` from returning data from offset 0 (the blockmap header) by mistake due to `unwrap_or(0)` fallback, which could cause infinite loops or bogus linedef processing in the collision detection.
🧪 Strategy: Added explicit bounds checking to ensure out-of-bounds blockmap queries return an empty iterator. Added `blockmap_block_linedefs_out_of_bounds_returns_empty` unit test to verify the fix.
🔭 Verification: `cargo test -p doom-map`
