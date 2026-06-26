🌟 Nova: Achievements System

💡 **The Spark:** We track a bunch of end-of-level stats (kills, items, secrets, time) but don't reward the player for mastery! Can we combine `GameState` and `LevelStats` to award achievements?
🚀 **The Feature:** Added a new `achievements` module behind a feature flag. It evaluates the `GameState` to award `Pacifist`, `Completionist`, and `Speedrunner` achievements.
🔮 **The Potential:** UI components can now display shiny badges at the intermission screen or integrate with platform-specific achievement APIs.
⚠️ **Risk:** Low. Completely isolated in `crates/doom-game/src/achievements.rs` behind a feature flag.
