🎯 Target: `doom_map::analyzer::MapAnalyzer::chokepoints`
💣 Risk: Traversal algorithms `unwrap()` on an unmapped adjacency node, leading to engine panics.
🧪 Strategy: Added a fallback `std::sync::OnceLock` for an empty `HashSet` to safely handle missing neighbors. Added a unit test covering unmapped graph node panics.
🔭 Verification: `cargo test`
