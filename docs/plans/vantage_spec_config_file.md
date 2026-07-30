# 🔭 Vantage: Spec for Config File Handling

## Parity Roadmap Mapping
Maps to **Gap 3: Config file handling** (M4 Config files) in `docs/PARITY_ROADMAP.md`.

## 👤 User Story
As a Player, I want the engine to automatically load and save my settings to a configuration file (`default.cfg`), so that I don't have to re-enter CLI arguments for my preferences every time I launch the game.

## ❓ The "So What?" Ask
What business problem does this solve? Currently, user retention and UX suffer because players are forced to pass verbose CLI arguments for basic preferences (like audio volume or keybindings). Providing a persisted state drastically improves usability and brings us to parity with standard DOS/Chocolate Doom expectations.

## 📈 Metric Definition
Success = 100% of standard `default.cfg` keys are parsed correctly on startup, and a vanilla `default.cfg` is read and round-tripped with key bindings and sound/video settings applied.

## 🔍 Gap Analysis
Standard DOS Doom and Chocolate Doom read and write to a plaintext `.cfg` file. Currently, doom-rs relies entirely on GNU-style `--long` CLI arguments and transient in-memory state. We must build parity with the existing configuration file standard without reinventing the wheel with modern formats like JSON or TOML.

## ✅ Acceptance Criteria
- Must read from `default.cfg` (or `chocolate-doom.cfg`) on startup to populate game settings.
- Must save settings back to the configuration file on exit.
- Must handle missing files gracefully by creating a new file with vanilla-accurate default values.
- Must strictly support the legacy key-value text format compatible with vanilla Doom.

## 🚫 Out of Scope
- Advanced UI menus for editing every single config value (Phase 2).
- Support for modern configuration formats like JSON/TOML or hierarchical configs.
