1. **Refactor `hitscan_shot_angle` in `crates/doom-game/src/weapon_fire.rs` to use an enum instead of boolean for `accurate_first_shot`.**
   - Execute `replace_with_git_merge_diff` on `crates/doom-game/src/weapon_fire.rs` to introduce a `ShotType` enum (`AccurateFirst`, `AlwaysSpread`).
   - Execute `replace_with_git_merge_diff` on `crates/doom-game/src/weapon_fire.rs` to update `hitscan_shot_angle` signature and logic.
   - Execute `replace_with_git_merge_diff` on `crates/doom-game/src/weapon_fire.rs` to update all call sites of `hitscan_shot_angle` to use the new enum.

2. **Refactor `next_map` in `crates/doom-game/src/phase.rs` to use `ExitRequest` instead of `secret_exit: bool`.**
   - Execute `replace_with_git_merge_diff` on `crates/doom-game/src/phase.rs` to change the parameter of `next_map`, `next_map_doom1`, and `next_map_doom2` to take `ExitRequest` instead of `secret_exit: bool`.
   - Execute `replace_with_git_merge_diff` on `crates/doom-game/src/phase.rs` to update `tick_playing` logic which calls `next_map`.
   - Execute `replace_with_git_merge_diff` on `crates/doom-game/src/phase.rs` to update all `next_map` call sites in the tests.

3. **Verify refactor correctness and ensure code standards.**
   - Execute `cargo fmt --all` via `run_in_bash_session` to format the code.
   - Execute `cargo clippy --all-targets --all-features -- -D warnings` via `run_in_bash_session` to check for lints.
   - Execute `cargo test` via `run_in_bash_session` to verify that no logic changed and all tests pass.

4. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
