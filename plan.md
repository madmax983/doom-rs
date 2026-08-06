1. **Add `lowest_adjacent_ceiling_no_neighbors_returns_own` test to `crates/doom-game/src/specials.rs`**:
   - Write a unit test `lowest_adjacent_ceiling_no_neighbors_returns_own` to verify `lowest_adjacent_ceiling` behavior for an isolated sector, asserting it falls back to the sector's own ceiling height.
2. **Add `highest_adjacent_floor_no_neighbors_returns_own` test to `crates/doom-game/src/specials.rs`**:
   - Write a unit test `highest_adjacent_floor_no_neighbors_returns_own` to verify `highest_adjacent_floor` behavior for an isolated sector, asserting it falls back to the sector's own floor height.
3. **Run Code Verifications**:
   - Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test -p doom-game` to verify the codebase after the changes.
4. **Pre-commit Instructions**:
   - Run `pre_commit_instructions` tool to execute pre-commit steps, ensuring proper testing, verification, review, and reflection are done.
5. **Submit PR**:
   - Submit a PR titled "🛡️ Sentry: [test coverage improvement]" with a description matching the required format.
