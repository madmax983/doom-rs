# 🔭 Vantage: Spec for Config File Handling

## 👤 User Story
As a Player, I want the engine to read and write my settings from configuration files, so that my key bindings, sound, and video preferences persist across sessions.

## ❓ So What?
Currently, settings are CLI-only. Without configuration files, players must manually provide command-line overrides on every launch, resulting in poor user experience. Closing this parity gap reduces session friction and moves us closer to full Chocolate Doom fidelity.

## 📈 Metric Definition
Success = A vanilla `default.cfg` can be read and round-tripped (written back) completely without missing keys or data corruption.

## 🔍 Gap Analysis
This directly maps to the explicit gap "M4 — Config file handling" in `docs/PARITY_ROADMAP.md`. The roadmap indicates that config file handling is currently "absent". The standard behavior in Chocolate Doom (our fidelity target) is reading/writing DOS `default.cfg` and a `chocolate-doom.cfg`-equivalent.

## ✅ Acceptance Criteria
- Must read and write DOS `default.cfg`.
- Must read and write a `chocolate-doom.cfg`-equivalent.
- Key bindings and sound/video settings must apply to the engine.
- Command-line interfaces (CLI overrides) must map to and override config file settings.
- A vanilla `default.cfg` is successfully read and round-tripped.

## 🚫 Out of Scope
- Vanilla savegame format handling (covered in M7).
- Game-mode / IWAD auto-detection (covered in M3).
