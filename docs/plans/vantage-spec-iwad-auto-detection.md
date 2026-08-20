# 🔭 Vantage: Spec for Game-mode / IWAD Auto-detection

## Context

Currently, the engine has a weak understanding of game modes (only distinguishing SinglePlayer vs Deathmatch) and lacks the ability to automatically identify the specific game or mission (e.g., shareware, registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom) based on the provided IWAD. This limits the engine's ability to properly select the correct content for demos, level structures, skies, and finale texts.

This spec defines the "What" and the "Why" for introducing Game-mode and IWAD auto-detection to ensure a seamless boot process.

## 👤 User Story

"As a Player, I want the engine to automatically detect the specific Doom game or mission from my IWAD file, so that I can launch straight into the correct episode structure, with the proper skies and finale text, without having to manually specify the game type."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Friction is a cost. By not auto-detecting the IWAD type, we force users to manually configure game variants or suffer from incorrect content gating. This auto-detection directly unblocks accurate content selection for demo playback (M1) and CLI parity (M5), improving the overall seamlessness of the engine and user experience. Utility increases when things "just work."

## ✅ Acceptance Criteria

### 1. IWAD Identification
- **Success Metric:** The engine reliably identifies all supported IWADs upon launch.
- **Criteria:**
  - The engine must parse IWAD lumps to automatically determine the game type (shareware, registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom).
  - The correct game mode and mission state must be set internally before the renderer or game loop starts.

### 2. Content Gating & Structure
- **Success Metric:** The correct levels, skies, and text are loaded based on the IWAD.
- **Criteria:**
  - The correct episode and level structure must be enforced.
  - The appropriate sky textures must be assigned to maps automatically.
  - The correct finale text must be presented for the identified game mode.
  - Unsupported or missing lump requests specific to other game modes must be correctly gated.

## 🚫 Out of Scope

- **Mod Compatibility Fixes:** Trying to auto-fix broken PWADs that have missing required lumps for the base IWAD.
- **Presentation Enhancements:** Changing the visual layout of menus or finales beyond displaying the correct vanilla text and sky.
- **Vanilla Limits Emulation:** Reproducing fixed array bounds or visplane crashes (handled in a separate track).

## ⚖️ Gap Analysis

Currently, `GameMode` is limited to SinglePlayer and Deathmatch. The engine does not automatically detect IWAD types like Shareware gating or Final Doom specifics. The engine only derives episode/map from the `--warp` flag. The engine must be updated to inspect the IWAD contents immediately upon loading and populate a comprehensive game mode configuration to drive subsequent rendering and gameplay logic.
