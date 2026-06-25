sed -i -e 's/println!("Path found: {}", path_str);/println!("Path found (test): {}", path_str);/g' crates/doom-app/src/main.rs
cargo check -p doom-app
