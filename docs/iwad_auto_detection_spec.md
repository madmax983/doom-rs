# 🔭 Vantage: Spec for IWAD Auto-Detection

## 👤 User Story
As a player, I want the engine to automatically identify which Doom game version I am playing from the provided IWAD file, so that the correct episodes, levels, skies, and intermission text are loaded without manual configuration.

## 🎯 So What? (Business Problem)
Currently, players have to know and manually specify their game type, or the engine might assume incorrect defaults. Automating this eliminates a major point of friction during startup, ensuring a seamless "plug-and-play" experience for any supported classic Doom title (Shareware, Registered, Ultimate, Doom II, Final Doom, etc.). It guarantees that demos will sync properly by having the correct underlying game rules and content loaded automatically.

## 📈 Success Metrics
- 100% accurate detection of standard commercially released IWADs (Doom Shareware, Doom Registered, Ultimate Doom, Doom II, Plutonia, TNT, Chex Quest, HACX, FreeDoom).
- Startup latency impact of detection logic is < 5ms.

## ✅ Acceptance Criteria
- Must inspect the provided IWAD file's directory/lumps to uniquely identify the game version.
- Must configure the internal game mode state (episodes, map progression, sky textures, finale text) based on the detected version.
- Must display a clear log message indicating which game mode was detected.
- Must gracefully fallback or halt with a descriptive error if the IWAD is malformed or completely unrecognizable.

## 🚫 Out of Scope
- Support for arbitrary fan-made PWAD detection.
- Dynamic hot-swapping of IWADs during gameplay.
