# 🔭 Vantage: Spec for Savegame Parity

## Description

👤 **User Story:**
As a Player, I want to save my progress and restore it exactly as it was, so that I don't lose my investment of time when I step away from the game.

✅ **Acceptance Criteria:**
- **Exact State Restoration:** Loading a save must restore the player, enemies, map triggers, and inventory exactly to the state they were in when saved.
- **Vanilla Compatibility:** Must be able to read and write save files that are compatible with vanilla Doom.
- **Resilience:** Corrupted or incompatible save files must fail gracefully and return the user to the menu with a readable error, rather than crashing the engine.
- **Success Metric:** 100% of vanilla save files load without error, and games saved in the new engine can be successfully loaded in Chocolate Doom.

🚫 **Out of Scope:**
- Quicksave/Quickload input handling (Phase 2).
- Cloud saves or cross-device syncing.
- Savegame slot management UI overhauls (stick to vanilla behavior).
