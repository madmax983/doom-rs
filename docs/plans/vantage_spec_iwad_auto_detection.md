# 🔭 Vantage: Spec for Game-mode / IWAD Auto-detection

## 👤 User Story
As a Player, I want the game engine to automatically detect which version of Doom I am playing based on the provided WAD file, so that I don't have to manually configure game modes, episodes, and correct assets.

## 🎯 The "So What?" (Business Problem)
Currently, players must manually specify or the game incorrectly loads configurations if the WAD isn't perfectly expected. This creates friction and confusion when users try to play Shareware, Ultimate Doom, Doom II, or custom IWADs like FreeDoom. Automating this removes setup friction, directly improving player onboarding and reducing support issues related to incorrect game states. This specifically addresses the M3 milestone in the Parity Roadmap.

## 📊 Metric Definition
- **Success:** 100% of supported IWADs (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom) correctly initialize their specific game mode (skies, map structure, finale text) without manual CLI overrides.
- **Success:** Zero crashes during engine initialization due to missing expected lumps when a valid alternative game-mode IWAD is provided.

## 🔍 Gap Analysis
- **Current State:** The engine lacks comprehensive auto-detection of the game mode based on IWAD contents, risking incorrect level structure or asset loading.
- **Market Standard (Chocolate Doom):** Automatically identifies the game mission and version by probing specific lumps in the IWAD, setting the correct episode limits, sky textures, and UI text seamlessly.
- **Strategic Fit:** This is required for M3 (Game-mode / IWAD auto-detection) and unblocks correct content selection for demo playback (M1) and CLI compatibility (M5).

## ✅ Acceptance Criteria
- Must identify the game version (Shareware, Registered, Ultimate, Doom II, Final Doom, etc.) by inspecting the presence of specific lumps in the WAD.
- Must automatically configure the engine's episode and level structure based on the detected game version.
- Must automatically select the correct sky textures and finale text based on the detected game version.
- Must gracefully default or error with a clear, user-friendly message if an unsupported or unrecognizable IWAD is provided.

## 🚫 Out of Scope
- Support for PWAD-based game mode overrides (this focuses strictly on the base IWAD).
- Emulation of specific bugs tied to different executable versions (this focuses only on content and structure selection).
