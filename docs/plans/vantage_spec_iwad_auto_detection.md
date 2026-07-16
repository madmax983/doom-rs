# 🔭 Vantage: Spec for IWAD Auto-Detection

## Context

Currently, `doom-app` sets the game mode (e.g., episode and map structure, skies, finale text) partially through command-line arguments like `--warp`, and relies on basic file naming (Doom 1 vs Doom II). It fails to fully auto-detect the intended game mode and mission structure directly from the content of the provided IWAD file.

This spec defines the requirements for implementing full IWAD auto-detection to close the gap identified in the Parity Roadmap (M3).

## 👤 User Story

"As a Player, I want the engine to automatically configure the correct game mode (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX) simply by pointing it at an IWAD, so that I experience the correct episodes, skies, lump gating, and finale text without needing manual configuration."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
A core promise of a Doom engine is that you can drop in any standard game file and it "just works." Forcing the player to manually specify the game variant creates friction and risks them experiencing broken content (like a Doom II engine expecting Doom 1 lumps). Auto-detection guarantees the engine boots into the correct behavioral state, achieving Chocolate Doom simulation parity and ensuring proper content gating (like preventing commercial PWADs from running on the shareware IWAD).

## 📊 Metric Definition

**Success =** 100% of supported commercial and retail IWADs boot into their correct, respective game modes with zero user-supplied CLI flags aside from `--iwad`.

## ✅ Acceptance Criteria

- The engine reads the provided IWAD file's lumps (or hash) to identify its specific release version.
- Supported target variants: Shareware Doom, Registered Doom, Ultimate Doom, Doom II, Final Doom (Plutonia/TNT), Chex Quest, HACX, and FreeDoom.
- The determined game mode configures the correct episode and map structures (e.g., E1M1 vs MAP01).
- The determined game mode configures correct sky textures, finale text, and intermission sequences.
- Proper lump gating is enforced (e.g., shareware IWADs reject PWADs to match vanilla behavior).

## 🚫 Out of Scope

- **Code Execution / Code Changes:** This is purely an analytical spec. The actual implementation in Rust is deferred to the engineering team.
- **PWAD Auto-Detection:** The scope is strictly limited to the base IWAD game mode detection. Guessing the intended compatibility level of random user-made PWADs is out of scope.

## ⚖️ Gap Analysis

Based on `docs/PARITY_ROADMAP.md`, the current system only discriminates between SinglePlayer and Deathmatch. Identifying the specific IWAD release (Shareware vs. Final Doom vs. Ultimate) is missing. The solution will require reading the data early in the boot sequence to establish the specific game mode variant, and communicating that variant across the playsim and configuration boundaries.
