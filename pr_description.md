🛡️ Sentry: MapAnalyzer chokepoints unwrap panic fix

🎯 Target: `MapAnalyzer::chokepoints` in `doom-map`
💣 Risk: Malformed or asymmetric graphs could cause a panic via `.unwrap()` when iterating neighbors.
🧪 Strategy: Replace `.unwrap()` with `.unwrap_or(&empty_set)` to handle missing edge lists safely. Added havoc tests for asymmetric connections and missing back edges.
🔬 Verification: Run `cargo test -p doom-map`
