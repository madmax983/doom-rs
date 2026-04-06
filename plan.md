1. **Optimize `sfx_candidate_names` in `crates/doom-app/src/audio_system.rs`**
   - Currently, it collects the iterator into a `Vec<String>`.
   - By changing its return type to `impl Iterator<Item = String> + '_` (or returning the iterator directly), we can avoid allocating the intermediate `Vec` when building the cache and lookup maps.
   - Modify the signature: `fn sfx_candidate_names<'a>(wad: &'a WadStack) -> impl Iterator<Item = String> + 'a`
   - Modify `populate_sfx_cache` and `build_sfx_lookup` to consume the iterator instead of `.into_iter()`.

2. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
   - Run tests and clippy formatting check.

3. **Submit the PR**
   - Include description about the optimization and metrics.
