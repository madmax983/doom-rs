# 🔭 Vantage: Spec for IWAD Auto-Detection

## User Story
As a Player, I want the engine to automatically detect the specific Doom game version (e.g., Shareware, Ultimate Doom, Doom II, Plutonia) based on the provided IWAD file, so that I experience the correct episodes, levels, skies, and finale text without needing manual configuration.

## So What?
Currently, users have to manually configure or guess the correct settings for different WAD files. This introduces friction and ruins the plug-and-play experience that modern source ports provide. By auto-detecting the IWAD, we eliminate a major usability hurdle and unblock correct content selection for demo playback and CLI arguments.

## Metric Definition
- Success = 100% accurate detection of shareware, registered, Ultimate, Doom II, Plutonia, TNT, Chex, HACX, and FreeDoom IWADs upon loading, with zero user input required.
- Load time impact from detection must be < 50ms.

## Gap Analysis
According to `docs/PARITY_ROADMAP.md` (Milestone 3 — Game-mode / IWAD auto-detection), we need to "Identify game/mission from IWAD lumps and set correct episode/level structure, sky, finale text, and lump gating". This unblocks correct content selection for M1's demo corpus (Playsim correctness + demo playback) and M5's CLI parity.

## Acceptance Criteria
- Must accurately identify the game version by inspecting the IWAD lumps.
- Must correctly configure the episode and level structure based on the detected game.
- Must set the appropriate sky textures and finale text for the detected game.
- Must enforce correct lump gating (shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom).

## Out of Scope
- Support for unrecognized or heavily modified PWADs altering core game definitions.
- Automatic downloading of missing IWAD files.
