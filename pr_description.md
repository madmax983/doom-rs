🎯 Target: `crates/doom-game/src/weapons.rs` / `tick_psprite_slot`
💣 Risk: The fallback to `StateNum::NULL` when encountering an invalid next_state was entirely untested, leaving a potential panic or bug unverified if the default behavior is modified in the future.
🧪 Strategy: Added a new unit test `tick_psprite_slot_invalid_state_transitions_to_null` to explicitly verify that `tick_psprite_slot` correctly falls back to `StateNum::NULL` when evaluating a state with an invalid configuration.
🔬 Verification: `cargo test --all-targets --all-features`
