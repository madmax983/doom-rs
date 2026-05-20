🎯 Target: Added test `save_error_from_doom_game` in `crates/doom-app/src/savegame.rs`.
💣 Risk: The app mapped `doom_game::savegame::SaveError` to `doom_app`'s internal `SaveError` implicitly without dedicated verification logic, meaning regressions in error interpretation (like truncations or bad magic mapping) would pass silently.
🧪 Strategy: Added a strict match check in `save_error_from_doom_game` unit test covering all the relevant `SaveError` types (`TooShort`, `BadMagic`, `BadVersion`, `Truncated`, `UnsupportedVanillaDsg`) against their mapped targets.
🔬 Verification: Run `cargo test --package doom-app --lib savegame::tests::save_error_from_doom_game`.
