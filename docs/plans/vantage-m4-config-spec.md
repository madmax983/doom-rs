# 🔭 Vantage: Spec for M4 Config File Handling

## 👤 **User Story:**
As a player, I want the engine to read and write my DOS `default.cfg` and a `chocolate-doom.cfg`-equivalent configuration file, so that my preferred key bindings, sound volumes, and video settings are saved across sessions and respect my existing vanilla setups.

## ✅ **Acceptance Criteria:**
- The engine reads a vanilla `default.cfg` and round-trips it without dropping unknown keys.
- Key bindings, sound volumes, and video settings defined in the config file are correctly applied to the game state.
- The engine supports a secondary `chocolate-doom.cfg`-equivalent file for extended settings.
- Command-line interface (CLI) arguments correctly override config file settings when specified.

## 🚫 **Out of Scope:**
- Building a new graphical or terminal-based in-game menu for modifying these settings (Phase 1).
- Implementing per-PWAD specific configuration profiles.
- Wiring multiplayer chat macro playback.
