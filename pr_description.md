Title: 🛡️ Sentry: [test coverage improvement]
Description:
🎯 Target: `tick_psprite_slot` in `crates/doom-game/src/weapons.rs`.
💣 Risk: The fallback to `StateNum::NULL` when transitioning a psprite from an invalid state index was completely untested, meaning a regression here could cause game logic bugs or panics without being caught.
🧪 Strategy: Added `tick_psprite_slot_with_invalid_next_state_removes_psprite` test to simulate an invalid state index and verify that it correctly falls back to `StateNum::NULL`.
🔬 Verification: `cargo test`
