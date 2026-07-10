# 🔭 Vantage: Spec for Game-Mode / IWAD Auto-Detection

## 👤 User Story
"As a Player, I want the engine to automatically configure itself based on my chosen WAD file (e.g. Doom 1 Shareware vs Doom II vs Plutonia), so that I don't have to pass complex command-line arguments to play the correct version."

## 🎯 The "So What?"
What business problem does this solve?
Currently, our engine lacks automatic identification of the loaded game variant. Users might have to manually specify the game mode via CLI, or the engine might assume incorrect level names or map sequence rules. By auto-detecting the IWAD, we ensure out-of-the-box compatibility with the ecosystem of vanilla Doom wads, lowering the barrier to entry and reducing user error.

## ✅ Acceptance Criteria
- **IWAD Signature Recognition:** The engine must inspect the loaded IWAD to classify it into one of the known Vanilla GameModes (Shareware, Registered, Ultimate, Doom 2, TNT, Plutonia).
- **Map Sequence Selection:** The identified GameMode must dictate the expected map format (e.g., ExMx vs MAPxx) and episode structure.
- **Title Screen / End Screen Adaptation:** Appropriate title screens and intermissions specific to the detected GameMode must be used.
- **Fail Gracefully:** If an unrecognized WAD is provided, the engine should default to the safest fallback (e.g., Doom 2 mode) and log a warning, rather than panicking.

## 🚫 Out of Scope
- Gameplay modifications or behavior tweaks based on game mode.
- Full DeHackEd or BEX patching.
- PWAD auto-loading.
