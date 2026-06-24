1. **Analyze performance in `sectors_by_tag`**
   - In `crates/doom-game/src/linedef_dispatch.rs`, the `sectors_by_tag` function collects matching sector indices into a heap-allocated `Vec<usize>` (`.collect::<Vec<_>>()`).
   - Since this function is called repeatedly during gameplay whenever linedef triggers with tags are activated, it causes repeated heap allocations.

2. **Refactor `sectors_by_tag` to return an iterator**
   - Change `sectors_by_tag` to return `impl Iterator<Item = usize> + '_`.
   - Update the call sites to either consume the iterator directly in a `for` loop, eliminating the intermediate `.collect::<Vec<_>>()`.
   - We need to modify `sectors_by_tag` to return an iterator and adapt the call sites.

3. **Complete pre-commit steps**
   - Run formatting (`cargo fmt`).
   - Run clippy (`cargo clippy`).
   - Run tests (`cargo test`).

4. **Submit Pull Request**
   - Create a PR titled "⚡ Bolt: Remove heap allocations in `sectors_by_tag`" with a description detailing the performance improvement.
