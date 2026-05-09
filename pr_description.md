💡 What: Replaced a `.collect::<Vec<_>>()` call with direct iterator passing to `List::new()` in `crates/doom-app/src/net_mode.rs`.
🎯 Why: The intermediate `Vec` allocation was unnecessary because `List::new()` accepts any type that implements `IntoIterator`.
📊 Impact: Eliminates one heap allocation per render frame in the multiplayer lobby UI.
🔬 Measurement: Run `cargo bench -p doom-demo` to verify zero regression.
