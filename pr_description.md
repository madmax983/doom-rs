🛡️ Sentry: [test coverage improvement]

🎯 Target: `crates/doom-game/src/sound_prop.rs` - `SoundRequest` enum variants (like `MonsterWake` and `MonsterDie`).
💣 Risk: Missing test coverage for core domain logic in sound propagation. Without these tests, a refactoring error could cause issues when working with the enums.
🧪 Strategy: Added comprehensive unit tests within `mod tests` testing the instantiation of `SoundRequest` enum variants (such as `MonsterWake`), ensuring the enum instances hold the correct position and handle data. Also resolved an unrelated deprecation warning in `doom-tui` to pass strict clippy checks.
🔬 Verification: Run `cargo test -p doom-game --lib`
