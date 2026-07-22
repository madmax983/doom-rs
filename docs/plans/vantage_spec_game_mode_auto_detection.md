# 🔭 Vantage: Spec for Game-Mode Auto-Detection

## 👤 User Story
As a Player, I want the engine to automatically configure itself based on the WAD file I provide, so that I can play Doom II, Plutonia, or the shareware version without memorizing command-line flags.

## 💼 "So What?" (Business Problem)
Currently, users must manually configure the engine for different IWADs (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom). This adds unnecessary friction and degrades the user experience. By implementing auto-detection based on IWAD lumps, we solve a critical usability gap (Parity Roadmap M3) and unblock correct content selection for demos and the CLI.

## ✅ Acceptance Criteria
- Must identify the game/mission from IWAD lumps (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom).
- Must configure the correct episode and level structure based on the detected game mode.
- Must set appropriate visual and narrative elements (skies, finale text) automatically.
- Must gate lumps correctly depending on the detected version.

## 🚫 Out of Scope
- Auto-detecting PWAD dependencies.
- Modifying the WAD selector UI.
