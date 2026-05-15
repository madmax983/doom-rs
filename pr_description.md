🎯 **Target**: `crates/doom-map/src/lumps.rs` (`Blockmap` parsing, `Sidedef`, `Sector`)
💣 **Risk**: WAD parsing of `Blockmap` lacked size boundary checks and could attempt OOM allocations based on unverified header lengths. `try_into().unwrap()` in `Sidedef` and `Sector` conversions masked their implicit safety, hiding crash possibilities during array parsing.
🧪 **Strategy**:
- Added unit tests specifically for `Blockmap` parsing targeting short boundaries, OOM limits, and out-of-bounds offset bounds checking.
- Replaced `try_into().unwrap()` with `.expect("slice bounds guarantee exact 8 byte length")` to properly enforce and document Rust safety assumptions within parsing implementations.
- Used Python scripting to safely inject the tests under a dedicated `#[cfg(test)] mod blockmap_tests { ... }` block to resolve AST collisions.
🔬 **Verification**: `cargo test -p doom-map` and `cargo clippy` passed cleanly.
