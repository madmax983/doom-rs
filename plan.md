1. **Modify `crates/doom-game/src/movement.rs`**: Change `move_spechit` to return `smallvec::SmallVec<[usize; 8]>` and use `smallvec::SmallVec` for `spechit` and `seen`. Also modify tests in `crates/doom-game/src/movement.rs`.
   - I will use `replace_with_git_merge_diff` to make the changes.
2. **Verify `crates/doom-game/src/movement.rs`**: Use `cat crates/doom-game/src/movement.rs` to verify the changes.
3. **Modify `crates/doom-game/src/actions.rs`**: Change `Vec::new()` to `smallvec::SmallVec::new()` for the `None` case in `spechit` assignment.
   - I will use `replace_with_git_merge_diff` to make the changes.
4. **Verify `crates/doom-game/src/actions.rs`**: Use `cat crates/doom-game/src/actions.rs` to verify the changes.
5. **Update `.jules/bolt.md`**: Add a journal entry documenting the performance fix.
   - I will use `cat << 'EOF' >> .jules/bolt.md` to append the entry.
6. **Verify `.jules/bolt.md`**: Use `cat .jules/bolt.md` to verify the journal entry.
7. **Testing**: Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo fmt --all`.
8. **Pre-commit**: Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
9. **Submit**: Create a PR with title "⚡ Bolt: Use SmallVec in move_spechit" and description with `💡 What:`, `🎯 Why:`, `📊 Impact:`, and `🔬 Measurement:`.
