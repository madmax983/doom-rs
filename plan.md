1. **Done.** Added the `style_meter` feature to `doom-game` and `doom-tui`, and enabled them from `doom-app` by default.
2. **Done.** Integrated `StyleMeter` to `GameState` in `crates/doom-game/src/state.rs`.
3. **Done.** Updated `StyleMeter` state during `GameState::tick` in `crates/doom-game/src/tic.rs` and when the player registers a kill in `crates/doom-game/src/combat.rs`.
4. **Done.** Displayed `StyleRank` in `CogmindHudWidget` in `crates/doom-tui/src/cogmind.rs`.
5. **Done.** Passed the `StyleRank` up from `DoomGame` to `CogmindHud` in `crates/doom-app/src/main.rs`.
6. **Done.** Fixed unit tests to populate the `style_rank` properly.
7. **Complete pre-commit steps.**
   - Run `pre_commit_instructions` to test, lint, format and update the journals.
