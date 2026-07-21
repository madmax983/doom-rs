# 🔭 Vantage: Spec for Game-Mode / IWAD Auto-Detection

## 👤 User Story
As a player, I want the engine to automatically identify the game I am playing based on the IWAD provided, so that I get the correct levels, skies, finale text, and game behavior without needing to manually specify command-line flags.

## 💼 The "So What?" (Business Problem)
Currently, users must manually configure game settings if they play something other than standard Doom. This is a poor user experience. By automatically detecting the game mode from IWAD lumps, we lower the barrier to entry, reduce configuration errors, and bring the engine closer to Chocolate Doom parity (Milestone M3 in the Parity Roadmap). A seamless boot process is essential for user retention and frictionless play.

## 📊 Metric Definition
- **Success** = 100% accurate identification of Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex Quest, HACX, and FreeDoom based solely on their IWAD contents.
- **Success** = Zero user intervention required to set the correct episode/level structure or sky behavior upon launch.

## 🕳️ Gap Analysis
- **Current State:** The engine lacks automatic game/mission identification. The correct episode structure, skies, and lump gating are not automatically set.
- **Market Standard (Vanilla Doom/Chocolate Doom):** Chocolate Doom identifies the game mode based on specific lump signatures (e.g., presence of `MAP01` vs `E1M1`, or unique title lumps) and configures the engine state accordingly.
- **The Gap:** We map directly to Milestone M3: "Identify game/mission from IWAD lumps and set correct episode/level structure, sky, finale text, and lump gating... Unblocks correct content selection for M1's demo corpus and M5's CLI."

## ✅ Acceptance Criteria
1. The engine must examine the loaded IWAD lumps at startup to identify the game version (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, FreeDoom).
2. The episode and level sequence must automatically match the detected game (e.g., 3 episodes for Registered, 4 for Ultimate, 32 linear maps for Doom II).
3. Sky textures and finale screens must automatically map to the correct levels for the detected game.
4. Content gating (e.g., preventing access to BFG in Shareware) must be enforced based on the detected mode.

## 🚫 Out of Scope
- PWAD level replacements (covered separately).
- Identifying arbitrary, non-standard IWADs not listed in the acceptance criteria.
- Automatically downloading missing IWAD files.
