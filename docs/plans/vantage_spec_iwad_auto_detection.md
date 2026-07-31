# 🔭 Vantage: Spec for Game-Mode / IWAD Auto-Detection

## 👤 User Story
As a Player, I want the engine to automatically detect the game and mission from the provided IWAD file, so that I don't have to manually specify command-line arguments to get the correct episodes, skies, and level structures.

## ❓ The "So What?" Ask
What business problem does this solve?
Currently, users have to manually configure the engine to know what IWAD they are playing, which creates a high barrier to entry and frustrates users who just want to drag-and-drop their WAD files. Auto-detection provides a seamless, standard source-port experience, unblocking correct demo playback and saving time for the user. It explicitly addresses Milestone M3 in the Parity Roadmap.

## 📊 Metric Definition
Success = 100% of officially supported IWADs (shareware, registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom) are correctly identified and boot into the appropriate game mode without requiring manual command-line overrides.

## 🔍 Gap Analysis
Currently, doom-rs lacks IWAD-based gamemode/gamemission auto-detection. The standard in the source-port market (e.g., Chocolate Doom) is to automatically configure the engine's state from IWAD lumps.

## ✅ Acceptance Criteria
- Must identify the game version (shareware, registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom) based on IWAD lump contents.
- Must set the correct episode and level structure based on the detected game.
- Must set the correct sky textures for the detected game and episode.
- Must set the correct finale text and intermission screens.
- Must properly gate lumps (e.g., preventing Doom II assets from loading in Shareware mode).

## 🚫 Out of Scope
- Support for unrecognized or heavily modified custom IWADs not matching official releases.
- Implementing the missing finale art or bunny screens (tracked separately in M9 Content polish).
