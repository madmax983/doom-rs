💡 **The Spark:** "I noticed we calculate map chokepoints in `doom-map` (using `MapAnalyzer`), but we don't use them to track player progress. Can we use these chokepoints to automatically generate speedrun splits as the player traverses the map?"

🚀 **The Feature:** "Implemented `SpeedrunTracker` in `doom-game` that hooks into `MapAnalyzer`. It automatically tracks when a player crosses a map's topological chokepoint and records a timestamped speedrun split."

🔮 **The Potential:** "This gives speedrunners zero-setup auto-splitting, and could be extended to show an end-of-level timeline or heatmaps of where players get stuck."

⚠️ **Risk:** "Low. The feature is behind the `speedrun_tracker` Cargo feature flag and is completely additive."
