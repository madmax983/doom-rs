# 🔭 Vantage: Spec for Config File Handling (M4)

## 👤 User Story
As a player, I want the engine to read and write my settings to `default.cfg` and a `chocolate-doom.cfg`-equivalent, so that my key bindings, sound, and video preferences persist across sessions and are compatible with vanilla Doom.

## 🎯 The "So What?" (Business Problem)
Currently, all settings in doom-rs are CLI-only, which causes friction across sessions. Implementing vanilla-compatible config file handling drives user retention and fulfills a mandatory compatibility requirement for Chocolate Doom parity.

## 📊 Success Metrics
* 100% successful parsing of a standard vanilla `default.cfg` file.
* Settings changes persist to disk and load correctly on the next launch.

## ✅ Acceptance Criteria
* The engine must read and write DOS `default.cfg` and a `chocolate-doom.cfg`-equivalent.
* A vanilla `default.cfg` must be successfully read and round-tripped.
* Key bindings, sound, and video settings from the config must apply to the engine.
* CLI arguments must override config file settings.

## 🚫 Out of Scope
* Using modern configuration formats (e.g., JSON/YAML) instead of vanilla CFG.
* Expanding the settings UI menu.
