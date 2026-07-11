1. Modify `crates/doom-game/src/player.rs` using `replace_with_git_merge_diff` to add the `//!` module-level storytelling documentation.
2. Modify `crates/doom-game/src/player.rs` using `replace_with_git_merge_diff` to add `## Examples` and `## Edge Cases` to `apply_damage` and `heal`.
3. Modify `crates/doom-game/src/player.rs` using `replace_with_git_merge_diff` to add `## Examples` and `## Panics` sections to `heal_overheal` and `set_health_capped`.
4. Run `cargo doc -p doom-game --open` (in the background) and `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo fmt --all` to verify documentation fixes.
5. Create `.jules/bard.md` and append a journal entry about the clarification using `cat << 'EOF' >> .jules/bard.md`.
6. Verify the journal entry was written using `cat .jules/bard.md`.
7. Write the pull request description to a file `pr_description.md` using the exact command containing all sections required by the Bard persona.
8. Verify the PR description contents using `cat pr_description.md`.
9. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
10. Submit the PR using the `submit` tool with the title "🎻 Bard: Rich documentation for PlayerState health APIs" and description from the file created in step 7.
