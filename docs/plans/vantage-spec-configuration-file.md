# 🔭 Vantage: Spec for Configuration File Support

## Context

Currently, `doom-app` relies entirely on command-line arguments (`--iwad`, `--pwad`, `--skill`, `--renderer`, `--turn-based`, etc.) to configure the game session. While this provides flexibility, it creates high friction for players who must re-enter long, complex command strings every time they want to play, especially if they have a preferred setup (like a specific renderer, skill level, or mod loadout).

This spec defines the "What" and the "Why" for introducing a persistent Configuration File to streamline the player experience.

## 👤 User Story

"As a Player, I want to save my preferred game settings (renderer, difficulty, mod loadout) in a configuration file, so that I can quickly launch the game with my favorite setup without typing long commands every time."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Complexity is a cost. By forcing players to define their environment via CLI arguments every launch, we increase the barrier to entry and reduce the likelihood of repeat play sessions. A configuration file solves this by saving player intent, making the "time to fun" nearly instantaneous on subsequent launches. This directly improves user retention and the overall "Human Interface" of the product.

## ✅ Acceptance Criteria

### 1. Persistent Storage
- **Success Metric:** A player's settings are remembered across game sessions.
- **Criteria:**
  - The application reads from a standard configuration file (e.g., `doom-app.toml`, `config.ini`, or `.doomrc`) at startup.
  - The file should support defining default values for all major CLI arguments (IWAD, PWADs, renderer mode, default skill, turn-based toggle).
  - If the file does not exist, the game launches with safe defaults and optionally generates a template file for the user.

### 2. Override Hierarchy
- **Success Metric:** Players can still easily test one-off configurations without altering their saved defaults.
- **Criteria:**
  - Command-line arguments must always take precedence over values defined in the configuration file. (e.g., If the config specifies `--renderer halfblocks` but the user runs `doom-app --renderer sixel`, the game must use `sixel` for that session).

### 3. Human-Readable Format
- **Success Metric:** Players can manually edit their settings using a standard text editor.
- **Criteria:**
  - The configuration file must use a widely understood, human-readable format (e.g., TOML, INI, or JSON).
  - It should support comments so that default files can include instructions.

## 🚫 Out of Scope

- **In-Game Settings Menu:** We are not building a graphical UI within the game to edit these settings in Phase 1. Players will edit the text file manually.
- **Per-WAD Configurations:** Advanced profiles (like different settings for different mods) are out of scope. We are focusing on a single, global configuration file.
- **Keybinding Configuration:** Customizing input mappings is a separate, more complex feature and is out of scope for this spec.

## ⚖️ Gap Analysis

Currently, `crates/doom-app/src/main.rs` uses `clap::Parser` to construct the `Args` struct directly from `std::env::args`. To implement this spec, the startup sequence must be refactored to parse the configuration file first, and then merge or override those values with the parsed CLI arguments. We will need to evaluate whether `clap` provides built-in tools for config file fallbacks, or if a dedicated configuration management library (like `figment` or `config`) should be introduced, keeping in mind the project's goal to minimize heavy dependencies.