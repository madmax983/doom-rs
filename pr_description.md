🎯 Target: `Blockmap::block_linedefs` in `crates/doom-map/src/lumps.rs`.
💣 Risk: Out-of-bounds blockmap queries silently resolved to offset 0, parsing the blockmap header as linedef list data which led to incorrect results or unexpected behavior.
🧪 Strategy: Explicitly check for out-of-bounds indexing by handling `None` in `offsets.get()`, and return an empty iterator. Added `blockmap_out_of_bounds_returns_empty_iterator` test.
🔬 Verification: `cargo test -p doom-map`
