1. **Refactor `sectors_by_tag` in `crates/doom-game/src/linedef_dispatch.rs`**
   - Use `replace_with_git_merge_diff` to update `sectors_by_tag` in `crates/doom-game/src/linedef_dispatch.rs` to return `impl Iterator<Item = usize> + '_` instead of `Vec<usize>`:
```
<<<<<<< SEARCH
/// Collect all sector indices matching `tag`.
fn sectors_by_tag(level: &Level, tag: u16) -> Vec<usize> {
    level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
        .collect()
}
=======
/// Collect all sector indices matching `tag`.
/// ⚡ Bolt Optimization:
/// Avoids an intermediate `.collect::<Vec<_>>()` allocation by returning an iterator.
fn sectors_by_tag(level: &Level, tag: u16) -> impl Iterator<Item = usize> + '_ {
    level
        .sectors
        .iter()
        .enumerate()
        .filter(move |(_, s)| s.tag == tag)
        .map(|(i, _)| i)
}
>>>>>>> REPLACE
```

2. **Verify changes**
   - Execute `run_in_bash_session` to run:
     `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test`

3. **Complete pre commit steps**
   - Complete pre commit steps to make sure proper testing, verifications, reviews and reflections are done.

4. **Submit PR**
   - Run `run_in_bash_session` with a heredoc to write the following text to `pr_description.md`:
```
⚡ Bolt: Eliminate heap allocations in `sectors_by_tag`

💡 What: Changed the return type of `sectors_by_tag` from `Vec<usize>` to `impl Iterator<Item = usize> + '_`.
🎯 Why: `sectors_by_tag` is used extensively in `linedef_dispatch.rs` (e.g. `dispatch_door`, `dispatch_stairs`, `dispatch_specials`, `close_door_by_tag_or_back`, `open_door_by_tag_or_back`) to trigger actions on multiple sectors. Creating a `Vec` for each linedef interaction caused unnecessary heap allocations in the hot path.
📊 Impact: Completely eliminates intermediate `.collect::<Vec<_>>()` chains in `sectors_by_tag`, removing multiple heap allocations per tag-based trigger dispatch.
🔬 Measurement: Run `cargo test` in `crates/doom-game/src/`. Verification passed via `cargo fmt`, `clippy`, and `cargo test`.
```
   - Then run `submit` to submit the branch.
