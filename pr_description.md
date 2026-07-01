🛡️ Sentry: MapAnalyzer missing node gracefully handling

🎯 Target: MapAnalyzer::chokepoints in doom-map
💣 Risk: Prevents unwrap panic on chokepoint generation if a graph vertex isn't present in adjacency list
🧪 Strategy: Used OnceLock for empty HashSet fallback to yield empty iterator in fallback paths, and added test to havoc_analyzer
🔬 Verification: cargo test -p doom-map --all-features
