💡 What:
Replaced the `Vec<usize>` return type of `sectors_by_tag` with `impl Iterator<Item = usize> + '_` by removing the `.collect()` call at the end of the chain.

🎯 Why:
To avoid intermediate heap allocations (`Vec::new()`) when mapping sector indices based on game tags, which is queried frequently during line trigger resolution.

📊 Impact:
Eliminates intermediate vector allocations on the hot path when triggering actions that affect grouped sectors. This fits within zero-cost abstractions by leveraging Rust’s lazy evaluation.

🔭 Measurement:
Run `cargo bench` and verify through `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` that the logic functions identically.
