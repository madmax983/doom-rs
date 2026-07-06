🎯 Target: Player ammo fetching, vanilla savegame version string parsing, and doomrs savegame description loading.
💣 Risk: Untested `unwrap_or()` fallback logic obscuring potential panics or silent data corruption during out-of-bounds indexing or malformed UTF-8 processing.
🧪 Strategy: Added targeted unit tests to verify `PlayerState::ammo` handles invalid index 999, `version_string` safely parses buffers without null terminators, and `load_game` gracefully defaults to `""` on invalid UTF-8 descriptions.
🔬 Verification: Run `cargo test -p doom-game` to verify all fallback branches execute without panics.
