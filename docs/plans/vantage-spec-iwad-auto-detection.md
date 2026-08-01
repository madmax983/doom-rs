# 🔭 Vantage: Spec for Game-Mode / IWAD Auto-Detection

## Context

Currently, the engine does not automatically detect the game mode (e.g., shareware, registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom) from the loaded IWAD files. Instead, it relies on manual configuration or assumes a default state. This causes friction because players expect the engine to "just work" and present the correct episodes, skies, and level structures based on the IWAD they provide, exactly like Chocolate Doom.

This spec defines the "What" and the "Why" for introducing Game-Mode and IWAD Auto-Detection.

## 👤 User Story

"As a Player, I want the engine to automatically configure the correct game rules and content based on the IWAD file I provide, so that I don't have to manually specify which game I am playing."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Complexity is a cost. If the user has to tell the engine what game they are playing, we are forcing them to do the engine's job. Automatic IWAD detection removes friction, lowers the barrier to entry, and ensures that the user experience is seamlessly correct from the moment they launch the game. This directly drives parity with Chocolate Doom and ensures proper content gating, which is a prerequisite for accurate demo playback and correct command-line behavior.

## ✅ Acceptance Criteria

### 1. Auto-Detection Logic
- **Success Metric:** The engine correctly identifies the game mode 100% of the time based on standard IWAD signatures.
- **Criteria:**
  - The engine inspects the loaded IWAD to identify the specific release (shareware vs registered vs Ultimate vs Doom II vs Plutonia vs TNT vs Chex vs HACX vs FreeDoom).
  - The core game state is automatically initialized with the correct context upon loading the IWAD.

### 2. Content Gating & Presentation
- **Success Metric:** The correct content is presented to the player based on the detected game mode.
- **Criteria:**
  - The episode and map selection interface correctly reflects the game mode.
  - The correct sky textures and finale text are loaded according to the game mode and current level progress.

## 🚫 Out of Scope

- **PWAD Auto-Detection:** Automatically detecting and applying complex mod (PWAD) load orders is out of scope. We only focus on the base game file.
- **Custom Game Modes:** Supporting entirely new, non-vanilla game modes via this auto-detection is out of scope.

## ⚖️ Gap Analysis

Currently, the engine lacks the capability to differentiate between shareware, registered, or expansion content automatically. We need to introduce a detection layer during the initial asset loading phase that identifies the game type and passes this context to the core simulation and presentation layers, mirroring the behavior described in the parity roadmap.
