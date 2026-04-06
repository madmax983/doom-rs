# 🔭 Vantage: Spec for Persistent Settings & Functional Options Menu

## 👤 User Story
As a Player, I want to be able to change my game settings (like Sound Volume, Music Volume, and Controls) in the Options menu and have them persist across sessions, so that I don't have to reconfigure the game every time I launch it.

## ❓ So What?
Currently, the Options menu contains placeholder items (`Sound Volume`, `Music Volume`, `Controls`) that do nothing (`MenuAction::Noop`), and there is no configuration file to save user preferences. This leads to a frustrating user experience where players cannot adjust their audio levels or controls, and any future settings would be lost upon closing the app. A persistent settings system is foundational for a good user experience.

## 📏 Metric Definition
- **Success Criteria:**
  - The Options menu correctly reads from and writes to a configuration file (e.g., `doom_config.toml` or `doom_config.json`).
  - Adjusting Sound/Music volume in the menu updates the live audio mixer and persists to disk.
  - The game loads the settings on startup and applies them automatically.
  - Error handling gracefully falls back to default settings if the configuration file is missing or corrupted.

## 🔍 Gap Analysis
- **Current State:** The game hardcodes default behaviors (e.g., audio volumes are fixed or not adjustable via UI). The settings menu is visually present but functionally a stub (`MenuAction::Noop`).
- **Standard Libs / Market:** Most terminal-based Rust games or applications use lightweight serialization crates like `serde` with `toml` or `json` to manage configurations locally in a standard user directory (e.g., via the `directories` crate). Implementing this is a standard and expected pattern for any modern source port or engine clone.

## ✅ Acceptance Criteria
- Must introduce a Configuration state module that serializes/deserializes to disk.
- Must implement functional `MenuAction`s for the existing Options items (`Sound Volume`, `Music Volume`, `Controls`).
- Must wire the UI changes to the corresponding backend systems (e.g., `doom-audio` mixer volumes).
- Must gracefully handle file I/O errors and corrupted config files without panicking.

## 🚫 Out of Scope
- Adding new complex options (e.g., custom keybinding rebinding UI) in this phase. The focus is on the foundational system and wiring up the existing placeholders (volumes).
- Cloud saving or syncing of configurations.
