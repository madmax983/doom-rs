1. Add `achievements` feature to `crates/doom-game/Cargo.toml`.
2. Create `crates/doom-game/src/achievements.rs` with the `AchievementEngine` and `Badge` types, and tests.
3. Export `pub mod achievements;` in `crates/doom-game/src/lib.rs` conditionally behind the `achievements` feature.
4. Verify tests pass (`cargo test --all-targets --all-features`)
5. Verify clippy passes (`cargo clippy --all-targets --all-features -- -D warnings`)
6. Format code (`cargo fmt --all`)
7. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
8. Submit PR with Nova branding.
