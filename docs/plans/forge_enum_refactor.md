# ⚒️ Forge: Enum Refactor for CeilingType and FloorType

1. **Refactor `CeilingType` and `FloorType` to use `strum_macros::FromRepr`.**
   - Apply `#[derive(strum_macros::FromRepr)]` and `#[repr(u8)]` to both enums in `crates/doom-game/src/state.rs`.
2. **Update savegame serialization/deserialization logic.**
   - In `crates/doom-game/src/savegame.rs`, simplify `write_ceiling_type` and `read_ceiling_type` using the `u8` representation.
   - Do the same for `write_floor_type` and `read_floor_type`.
3. **Verify.**
   - Ensure `cargo test -p doom-game --all-features` passes cleanly without changing functionality.
   - Run `cargo clippy -p doom-game --all-features -- -D warnings` to verify idiomatic Rust.
4. **Pre-commit and Submit.**
   - Run pre-commit checks.
   - Create a PR.
