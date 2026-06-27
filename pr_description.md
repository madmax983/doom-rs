⚡ Bolt: Eliminate heap allocations in `sectors_by_tag`

💡 What: Changed the return type of `sectors_by_tag` from `Vec<usize>` to `impl Iterator<Item = usize> + '_`.
🎯 Why: `sectors_by_tag` is used extensively in `linedef_dispatch.rs` (e.g. `dispatch_door`, `dispatch_stairs`, `dispatch_specials`, `close_door_by_tag_or_back`, `open_door_by_tag_or_back`) to trigger actions on multiple sectors. Creating a `Vec` for each linedef interaction caused unnecessary heap allocations in the hot path.
📊 Impact: Completely eliminates intermediate `.collect::<Vec<_>>()` chains in `sectors_by_tag`, removing multiple heap allocations per tag-based trigger dispatch.
🔬 Measurement: Run `cargo test` in `crates/doom-game/src/`. Verification passed via `cargo fmt`, `clippy`, and `cargo test`.
