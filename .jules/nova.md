**Integrate StyleMeter into CogmindHud**
**Learning:** doom-game implements an optional feature `style_meter` which tracks DMC-like style. However, the score isn't visibly exposed to the user. Integrating it with doom-tui's CogmindHud provides an awesome way to utilize this hidden feature.
**Action:** Plumbed `StyleRank` up through `GameState`, `CogmindHud`, and `CogmindHudWidget`.

**Nova Simulator Mode**
**Learning:** `DemoPlaybackApp` processes tics very efficiently, but it was previously tied to the visual rendering loop. Running it in a tight `while !playback_app.is_finished()` loop allows for extremely fast, headless simulation of an entire demo.
**Action:** Implemented the `--simulate-demo` feature to rapidly parse an LMP file, process all logic ticks headlessly, and output the final stats (and optionally export GeoJSON telemetry). This enables CI benchmarking and rapid AI evaluation without the overhead of drawing frames to the terminal.
