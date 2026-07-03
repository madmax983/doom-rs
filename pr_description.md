🎯 Target: `savegame_vanilla::parse_header`, `savegame_vanilla::load_game`, `savegame_vanilla::save_game`
💣 Risk: Invalid or corrupt vanilla savegame headers could cause panics or silent failures if not properly parsed and rejected. Unsupported loads/saves could also fail silently without returning the expected `UnsupportedVanillaDsg` error.
🧪 Strategy: Added unit tests that explicitly construct a valid vanilla header and then test boundary and failure conditions, including short data (`TooShort`), bad magic bytes (`BadMagic`), unsupported version strings (`BadVersion`), and unsupported load/save executions.
🔬 Verification: Run `cargo test --manifest-path crates/doom-game/Cargo.toml --all-targets --all-features` to verify.
