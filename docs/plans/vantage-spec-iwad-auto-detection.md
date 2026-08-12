# 🔭 Vantage: Spec for Game-mode / IWAD Auto-detection

👤 **User Story:**
As a Player, I want the engine to automatically identify the provided IWAD and configure the correct game mode (episodes, sky textures, finale texts, etc.), so that I don't have to manually specify command-line arguments for every different Doom version I want to play.

**The "So What?" (Business Problem):**
Currently, players must manually configure game-mode specific settings when loading different IWADs (e.g., Ultimate Doom vs. Doom II vs. Plutonia). This creates friction, degrades the user experience, and breaks parity with Chocolate Doom and vanilla executables, which handle this automatically. By auto-detecting the IWAD, we reduce configuration errors and seamlessly deliver the correct content (skies, maps, text) out of the box.

**Success Metrics:**
- 100% accurate identification of the 9 primary supported IWADs (Shareware, Registered, Ultimate, Doom II, Plutonia, TNT, Chex Quest, HACX, FreeDoom) based on lump presence/hashes.
- Zero manual CLI flags required to launch into the correct game mode (e.g. Episode vs. MAP format) when a valid IWAD is provided.

✅ **Acceptance Criteria:**
- The engine must parse the provided IWAD file and identify its specific release version based on unique lumps or checksums.
- Must correctly configure the episode/level structure (e.g., E1M1-E4M9 vs MAP01-MAP32).
- Must set the correct sky textures for the identified game mode.
- Must set the correct finale text and lump gating based on the identified game mode.
- Unblocks correct content selection for the M1 demo corpus and M5 CLI integration as per the parity roadmap.

🚫 **Out of Scope:**
- Automatic location and loading of IWAD files from the filesystem (users still provide the file, we just identify its contents).
- Support for arbitrary/unrecognized PWADs modifying the core game mode structure (handled separately).
- Changes to the rendering or simulation logic itself; this is purely configuration/initialization routing.
