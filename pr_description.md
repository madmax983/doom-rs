🌟 Nova: Headless Demo Simulator

💡 **The Spark:** We have robust demo playback (`DemoPlaybackApp`) and spatial telemetry tracking, but they are tied to the live terminal event loop. I wanted to see if we could strip away the renderer and play back an entire level's demo in milliseconds to instantly generate a tactical report or heatmap.

🚀 **The Feature:** Implemented a new `--simulate-demo` CLI argument. It wraps the game in a `DemoPlaybackApp` and tight-loops over `.tick()` headlessly until the demo is exhausted. It then outputs the final intermission statistics (and optionally the GeoJSON telemetry map). It is fully guarded behind the `simulator` Cargo feature flag.

🔮 **The Potential:** This lays the foundation for AI training or continuous integration benchmarking. We can now run hundreds of demos in seconds to find performance regressions, validate deterministic behavior, or see how an `AiDirector` would behave over a full playthrough.

⚠️ **Risk:** Low. The feature is completely additive and hidden behind the `simulator` feature flag. It reuses existing demo loading and `TicInput` mechanics, avoiding any modifications to the core `GameState` logic.
