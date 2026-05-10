🎯 Target: `Sidedef::from_bytes` and `Sector::from_bytes` in `crates/doom-map/src/lumps.rs`.

💣 Risk: The functions parsed 30-byte and 26-byte byte streams and used `.try_into().unwrap()` directly on slice windows to extract `[u8; 8]` texture string representations. Although protected by the outer `parse_fixed_records` bounds checks, this violated the zero-panic `unwrap` policy and posed a structural risk of panicking on malformed input data if the length guarantees were ever altered in the future.

🧪 Strategy: Replaced the implicit `.try_into().unwrap()` operations with explicit slice index extraction blocks (e.g. `[b[4], b[5], ..., b[11]]`). Additionally added two new unit tests to the module (`should_parse_sidedef_from_bytes_without_panic` and `should_parse_sector_from_bytes_without_panic`) using a blank `0u8` initialized buffer to guarantee zero panics during deserialization roundtrips.

🔭 Verification: `cargo test -p doom-map --lib lumps::tests::should_parse`
