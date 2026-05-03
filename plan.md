1. **Create fuzz target for custom save format:**
   - Create `fuzz/fuzz_targets/fuzz_target_savegame_doomrs.rs` to execute fuzz tests against the custom DRS1 format (`load_game`).
2. **Fix `doom-app` JSON error formatting for CLI arguments:**
   - Modify `crates/doom-app/src/main.rs` to format `clap` errors using JSON if `--json` is present.
3. **Run tests and verify changes:**
   - Use `cargo test` to run tests and make sure the game compiles and passes properly.
4. **Complete pre-commit steps:**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
5. **Submit the PR:**
   - Submit the changes using the Sentry persona.
