# Spec: Game-mode / IWAD auto-detection

## 👤 User Story
As a Player, I want the engine to identify the game and mission from IWAD lumps, so that the correct episode and level structure, sky, finale text, and lump gating are set automatically.

## ❓ The "So What?" Ask
**What business problem does this solve?**
Users expect the software to run their game files correctly out of the box. By automatically parsing IWAD lumps to determine the game mode, we ensure the correct content is loaded without manual overrides. This specifically unblocks correct content selection for M1's demo corpus and M5's CLI flags, reducing configuration friction for parity-critical workflows.

## 📊 Metric Definition
- **Success =** Each supported IWAD boots into the right game mode with correct maps/skies/text.

## 🔍 Gap Analysis
Currently, our engine lacks game-mode and mission auto-detection. As explicitly defined in the Parity Roadmap (`docs/PARITY_ROADMAP.md`):
- **Gap:** "Game-mode/mission auto-detection from IWAD lumps (shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom), with correct lump gating."
- **ROI:** Implementing this fulfills milestone "M3 — Game-mode / IWAD auto-detection (M)", which explicitly "Unblocks correct content selection for M1's demo corpus and M5's CLI."

## ✅ Acceptance Criteria
- Must identify game/mission from IWAD lumps.
- Must set correct episode/level structure, sky, finale text, and lump gating.
- Must support: shareware, registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, and FreeDoom.
- Each supported IWAD boots into the right game mode with correct maps/skies/text.

## 🚫 Out of Scope
- Command line arguments for episode selection (this is M5).
- Demo playback functionality (this is M1).
