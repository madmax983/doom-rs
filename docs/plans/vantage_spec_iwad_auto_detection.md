# Vantage Spec: IWAD Auto-Detection

## 👤 User Story
As a Player, I want the game engine to automatically detect which Doom game I am playing based on the IWAD file I provide, so that the correct episodes, levels, skies, and intermission texts are loaded without requiring me to manually specify command-line arguments.

## ❓ So What?
Currently, users have to manually configure parameters or rely on incomplete detection. Automatic IWAD detection lowers the barrier to entry, ensures a smooth out-of-the-box experience, and prevents bugs caused by loading assets incorrectly. It is a fundamental expectation for a modern source port and directly addresses **M3 — Game-mode / IWAD auto-detection** in the Parity Roadmap.

## 🎯 Metric Definition
Success = 100% accurate auto-detection of the game mode for supported IWADs (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom) upon engine startup.

## ✅ Acceptance Criteria
- The engine must read the provided IWAD file on startup and identify the game based on its lump structure.
- The engine must correctly configure the episode and level progression for the detected game.
- The correct sky textures must be loaded for the respective episodes/levels.
- The appropriate intermission texts and finale screens must be displayed for the detected game.
- The engine must gate lumps appropriately (e.g., shareware vs registered content).

## 🚫 Out of Scope
- Support for arbitrary custom PWAD game-mode definitions (this is strictly for official/known IWADs).
- Modifications to the renderer or gameplay mechanics.
- Downloading or acquiring IWAD files for the user.

## 🔍 Gap Analysis
Currently, doom-rs lacks robust game-mode auto-detection from IWAD lumps. It relies partially on `--warp` and simple naming conventions, missing the nuanced lump gating and content selection required for true parity with Chocolate Doom. This gap prevents accurate demo sync (M1) and robust CLI usage (M5), making it a critical parity blocker.
