# Feature Specification: IWAD Auto-Detection

## 👤 User Story
As a player, I want the engine to automatically identify the game from the IWAD lumps, so that the engine sets the correct episode and level structure, sky, finale text, and lump gating.

## 💼 "So What?" (Business Problem)
Currently, shareware gating is not auto-detected. This feature unblocks correct content selection for M1's demo corpus and M5's CLI.

## 📊 Metric Definition
- **Success:** Each supported IWAD (shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom) boots into the right game mode with correct maps/skies/text.

## 🔍 Gap Analysis
- Shareware gating is not auto-detected.
- Game-mode/mission auto-detection from IWAD lumps is missing.

## ✅ Acceptance Criteria
- Identify game/mission from IWAD lumps.
- Set correct episode/level structure, sky, and finale text.
- Set correct lump gating.
- Supports shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom.

## 🚫 Out of Scope
- Implementation of M1's demo corpus.
- Implementation of M5's CLI.
