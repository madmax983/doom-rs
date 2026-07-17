# 🔭 Vantage: Spec for Game-Mode / IWAD Auto-Detection

## Gap Analysis
This feature addresses the M3 milestone ("Game-mode / IWAD auto-detection") identified in `docs/PARITY_ROADMAP.md`. Currently, doom-rs lacks automatic identification of IWAD versions (Shareware, Registered, Ultimate, Doom II, Final Doom, Chex, HACX). It does not automatically detect the game/mission from IWAD lumps, meaning episode/level structure, skies, finale text, and lump gating are not automatically configured.

## 👤 User Story
As a player, I want the engine to automatically detect the game version from the loaded IWAD file, so that the correct episodes, levels, skies, and finale texts are presented without requiring manual CLI arguments.

## So What? (Business Problem)
Without auto-detection, the user experience requires manual configuration. It also blocks M1 ("Playsim correctness + demo playback") and M5 ("CLI arg parity") because correct content selection is required for deterministic demo playback and correct command-line behavior.

## Metric Definition
- **Success =** 100% accurate game mode detection across all supported IWADs (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom) automatically.

## ✅ Acceptance Criteria
- Must identify the game/mission automatically from specific identifying lumps within the loaded IWAD.
- Must automatically configure the episode and level structure to match the detected game.
- Must set the correct skies and finale texts based on the detected game.
- Must correctly gate lumps and content (shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom).

## 🚫 Out of Scope
- Implementation of the `vanilla-compat` limit emulation (M2).
- Actually implementing the demo sync engine (M1).
