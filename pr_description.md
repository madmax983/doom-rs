💡 **The Spark:** "I noticed we can record and playback demos, but we don't have any way to extract meaningful statistics from them to compare player performance or speedrun efficiencies without watching the whole demo."
🚀 **The Feature:** "Implemented `analyzer.rs` in `doom-demo` which parses a `DemoPlayer` to extract high-level metrics like `total_tics`, `idle_tics`, `attack_tics`, `use_tics`, and total movement distance."
🔮 **The Potential:** "Could be used by the CLI to print a summary of a demo file without playing it, or to aggregate statistics across multiple demos for telemetry."
⚠️ **Risk:** "Low. Isolated additive module in `doom-demo/src/analyzer.rs`. No changes to core playback logic."
