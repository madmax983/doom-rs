# 🔭 Vantage: Spec for IWAD Auto-detection

## 👤 User Story
As a Player, I want the engine to automatically detect which version of Doom I am playing (Shareware, Ultimate, Doom II, Final Doom) based on my provided IWAD file, so that the correct episodes, maps, title screens, and game rules are applied without me having to manually specify command-line flags.

## ❓ So What?
Currently, the engine has weak game-mode detection (only differentiating between SinglePlayer and Deathmatch) and handles Doom II vs Doom 1 naming via the `--warp` flag. This lack of auto-detection means players might experience incorrect skies, finale text, or missing content gating (like Shareware locking). Automatic game-mode identification from IWAD lumps (e.g., shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom) is essential for vanilla parity and ensuring a seamless, historically accurate experience out of the box.

## 📏 Metric Definition
- **Success Criteria:**
  - Launching the engine with a specific IWAD (e.g., `doom1.wad`, `doom2.wad`, `tnt.wad`) automatically configures the correct game mode and level structure.
  - The correct sky textures and intermission text are displayed for the loaded game mode.

## 🔍 Gap Analysis
- **Current State:** The `GameMode` enum is only SinglePlayer/Deathmatch. There is no IWAD-based gamemode/gamemission auto-detection. Episode/map is only determined from `--warp`. Final Doom / shareware gating is not auto-detected.
- **Standard Libs / Market:** Chocolate Doom and other vanilla-accurate source ports identify the game version by checking for the presence of specific lumps in the IWAD, ensuring accurate game rules and content mapping.

## ✅ Acceptance Criteria
- Must identify the specific game mode (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX) based on the presence of unique lumps within the IWAD.
- Must configure the correct episode and level structure (e.g., Episode 1-only for Shareware, Episodes 1-3 for Registered, Episodes 1-4 for Ultimate, Maps 1-32 for Doom II/Final Doom).
- Must configure the correct skies, intermission text, and finale screens for the detected game mode.
- Must enforce correct lump gating based on the game mode (e.g., preventing access to registered content in shareware mode).

## 🚫 Out of Scope
- Support for arbitrary custom PWAD game modes outside of the standard list.
- Modifying the renderer to handle custom game mode specific behaviors beyond texture and text selection.
