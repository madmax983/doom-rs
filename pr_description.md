🎯 Target: crates/doom-map/src/lumps.rs
💣 Risk: try_into().unwrap() on byte slices can panic.
🧪 Strategy: Replaced with safe index-by-index array construction and added unit tests.
🔭 Verification: cargo test -p doom-map
