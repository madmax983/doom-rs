📖 Chapter: The `doom-game::director` module.
🔦 Insight: Clarified how the AI Director manages pacing based on the player's health threshold, mapping specific values to `DirectorAction::SpawnAmbush` and `DirectorAction::SpawnRelief`.
🧪 Example: Added an executable doc-test for `AiDirector::tick` that manually manipulates player health to demonstrate the thresholds, and added basic structural doctests for the enums.
🖼️ Preview: (Ran `cargo doc --open` locally to verify).
