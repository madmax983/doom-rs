# 🔭 Vantage: Spec for Game-Mode / IWAD Auto-Detection

## 👤 User Story
As a Player, I want the engine to automatically detect which Doom game I am playing based on the loaded IWAD, so that I don't have to manually configure command-line flags for the correct episodes, skies, and level structures.

## 🎯 The "So What?" (Business Problem)
**Utility over Complexity:** Currently, the player experience is brittle. If a user loads a Doom II IWAD but the engine doesn't automatically switch to Doom II's continuous level structure and cast call, the simulation breaks. By auto-detecting the IWAD type (Shareware, Registered, Ultimate, Doom II, Final Doom, etc.), we reduce friction, prevent invalid configurations, and ensure simulation parity with Chocolate Doom.

## 📊 Metric Definition
- **Success:** 100% of supported official IWADs boot into the correct internal game mode without requiring manual user overrides.
- **Success:** The engine identifies the game/mission from IWAD lumps and successfully sets the correct internal mode (`Shareware`, `Registered`, `Retail`, `Commercial` or `Indetermined`) and mission (`Doom`, `Doom2`, `PackTnt`, `PackPlutonia`, `PackChex`, `PackHacx`, or `None`) as defined in `crates/doom-app/src/iwad.rs`.

## 🔍 Gap Analysis
- **Current State:** The roadmap (`docs/PARITY_ROADMAP.md`) indicates that IWAD-based game-mode/mission auto-detection is "PARTIAL (weak)". The roadmap says: "`GameMode` enum is only SinglePlayer/Deathmatch; **no IWAD-based gamemode/gamemission auto-detection**; episode/map only from `--warp`. Doom II vs Doom 1 naming handled; Final Doom / shareware gating not auto-detected." Note that while `crates/doom-app/src/iwad.rs` now contains definitions for `GameMode` and `GameMission`, the full auto-detection logic based on lumps (such as correctly gating features or inferring settings) is incomplete as per the roadmap.
- **Market Standard:** Chocolate Doom uses a well-defined set of heuristics (checking for specific lumps) to set the exact game mission and version, gating content appropriately.
- **Roadmap Alignment:** Directly addresses **M3 — Game-mode / IWAD auto-detection** in `docs/PARITY_ROADMAP.md`.

## ✅ Acceptance Criteria
1. Must identify the game/mission from IWAD lumps (distinguishing shareware, registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom).
2. Must set the correct level structure (Episodes vs. continuous maps) based on the detected IWAD.
3. Must gate content appropriately based on the detected mode.
4. Must load the correct sky textures and finale text based on the detected game mode.

## 🚫 Out of Scope
- Support for totally custom, non-standard IWAD structures outside of the officially recognized Doom engine lineage.
- Automatic downloading of missing IWADs.
