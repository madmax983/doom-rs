# 🔭 Vantage: Spec for Game-mode / IWAD Auto-Detection

## 👤 User Story
As a Player, I want the engine to automatically detect which Doom game I am playing based on the IWAD file I provide, so that I don't have to manually specify command-line arguments for the correct episodes, skies, and level progression.

## ❓ So What?
Currently, users may have to rely on manual configuration or specific launch flags to load the correct assets and level sequences for variations like Ultimate Doom, Doom II, or Final Doom. This friction degrades the out-of-the-box experience. Auto-detection ensures a seamless, "it just works" launch that immediately drops the player into the authentic experience for their provided game data, significantly improving usability and first impressions.

## 📏 Metric Definition
- **Success Criteria:**
  - 100% of officially supported vanilla IWADs (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT) are correctly identified upon launch.
  - The correct game mode parameters (episode count, map sequence, finale text, sky textures) are automatically applied without user intervention.

## 🔍 Gap Analysis
- **Current State:** The engine lacks automatic identification of game versions from IWAD lumps, potentially loading incorrect defaults for certain map sequences or skies.
- **Market / Standard:** Chocolate Doom and other standard source ports implement robust IWAD auto-detection to ensure parity with the original DOS executable behaviors for each specific release.

## ✅ Acceptance Criteria
- Must identify the game version by scanning the provided IWAD file's lumps (e.g., Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom).
- Must automatically configure the internal engine state (episodes, levels, skies, finale text) to match the detected game version.
- Must boot seamlessly into the correct game mode without requiring manual command-line overrides.

## 🚫 Out of Scope
- Support for auto-detecting deeply customized, non-standard PWAD total conversions that don't mimic official IWAD structures.
- A GUI-based IWAD selector/launcher on startup (handled in a separate feature).
