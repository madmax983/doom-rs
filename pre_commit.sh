cargo check --all-targets --all-features
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
