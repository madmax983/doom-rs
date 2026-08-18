# 🔭 Vantage: Spec for M4 Config file handling

## 👤 User Story
As a vanilla Doom player, I want the engine to read and write DOS `default.cfg` and a `chocolate-doom.cfg`-equivalent, so that my existing key bindings and sound/video settings are applied automatically.

## 🎯 The "So What?" Ask
**What business problem does this solve?**
We are aiming for simulation-level parity with Chocolate Doom. Without vanilla config file support, players cannot migrate their existing `default.cfg` settings, breaking the promise of a drop-in replacement. Respecting these files ensures seamless onboarding and preserves historical preferences.

## ✅ Acceptance Criteria
- Must read and write DOS `default.cfg` and a `chocolate-doom.cfg`-equivalent.
- A vanilla `default.cfg` must be read and round-tripped successfully.
- Key bindings and sound/video settings must apply to the engine.
- Must wire settings through to the engine with CLI overrides taking precedence.

## 🚫 Out of Scope
- Support for advanced source port configuration formats (e.g., ZDoom .ini, Boom .cfg).
- Graphical in-game UI to modify these config files.
