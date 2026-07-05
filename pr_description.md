💡 What: Replaced `HashMap<usize, HashSet<usize>>` with `Vec<HashSet<usize>>` in `SectorGraph` adjacency list.
🎯 Why: Sector indices are dense, sequential 0-indexed integer keys. Using a `HashMap` incurs unnecessary hashing overhead and heap allocations. Pre-allocating a `Vec` based on `level.sectors.len()` eliminates this overhead, providing a zero-cost abstraction for sector lookups.
📊 Impact: Reduces heap allocations and eliminates hashing overhead during graph building and pathfinding/analysis queries.
🔬 Measurement: Run `cargo test` to verify graph correctness and analysis (like chokepoints and isolated areas). No regressions in behavior.
