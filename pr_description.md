🎯 Target: `tick_psprite_slot` in `crates/doom-game/src/weapons.rs`
💣 Risk: `tick_psprite_slot` relies on `unwrap_or(StateNum::NULL)` when falling back on an invalid state lookup. This edge-case branch lacked explicit testing, which could hide logic regressions where the fallback state isn't handled correctly by downstream state logic.
🧪 Strategy: Added `tick_psprite_slot_with_invalid_next_state_falls_back_to_null` which forces an invalid state transition and explicitly asserts that the fallback to `StateNum::NULL` works and tics are reset to 0.
🔬 Verification: `cargo test --package doom-game`
