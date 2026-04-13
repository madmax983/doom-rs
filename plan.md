1. **Review modifications and cleanup**
   - Use `run_in_bash_session` to run `git diff` to review the modifications.
   - Use `run_in_bash_session` to run `rm -f *.py` to ensure temporary scripts are removed.
2. **Verify changes**
   - Use `run_in_bash_session` to run `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --all`.
3. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
4. **Submit PR**
   - Submit the changes using the `submit` tool with title '⚡ Bolt: Optimize cache lookups using LumpName'.
   - The PR description will contain:
     - 💡 What: Switched `HashMap` keys in `FlatCache`, `TextureCache`, and `SpriteCache` from `String` to `LumpName`.
     - 🎯 Why: Frequent string allocations during cache lookups on the hot rendering path caused unnecessary heap pressure.
     - 📊 Impact: Eliminates multiple heap allocations per frame during flat, texture, and sprite lookups.
     - 🔬 Measurement: Run bench `renderer_bench` or profile rendering performance.
