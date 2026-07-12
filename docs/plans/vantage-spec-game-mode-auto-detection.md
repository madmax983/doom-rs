# 🔭 Vantage: Spec for Game-Mode / IWAD Auto-Detection

## 👤 User Story
As a Player, I want the engine to automatically identify the game and mission from the provided IWAD file, so that I don't have to manually configure the game mode.

## ❓ So What?
Currently, Game-mode / IWAD auto-detection is absent. This prevents automatic identification of shareware, registered, Ultimate, Doom II, Final Doom (Plutonia/TNT), Chex, and HACX. This is a critical parity gap because it dictates episode/level structure, skies, finale text, and lump gating. Proper auto-detection is required to unblock correct content selection for demo testing and CLI functionality.

## 📏 Metric Definition
- **Success Criteria:**
  - The engine correctly identifies the IWAD (shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom) from its lumps.
  - The correct episodes, level structure, skies, and finale texts are set automatically.
  - Lump gating is applied correctly based on the detected game.

## 🔍 Gap Analysis
- **Current State:** Game-mode / IWAD auto-detection is absent.
- **Standard Libs / Market:** Chocolate Doom and other vanilla-accurate ports automatically identify the game mode from the IWAD contents, which is essential for accurate simulation and presentation.

## ✅ Acceptance Criteria
- Must identify the game/mission from IWAD lumps.
- Must set correct episode/level structure, sky, finale text, and lump gating (shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom).
- Each supported IWAD boots into the right game mode with correct maps/skies/text.

## 🚫 Out of Scope
- Config file handling.
