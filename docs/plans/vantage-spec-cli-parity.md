# 🔭 Vantage: Spec for CLI Argument Parity (M5)

## 👤 User Story
As a player, I want to use standard vanilla Doom command-line arguments, so that I can easily launch custom game modes and load mods using my existing launch scripts.

## 💼 Business Problem (The "So What?")
Currently, `doom-rs` does not support standard vanilla command-line flags. This prevents integration with standard community tools (like Doom launchers) and breaks user workflows. By closing this gap, we increase product utility and user retention by allowing it to function as a drop-in replacement.

## 🔍 Gap Analysis
According to the `PARITY_ROADMAP.md`, `doom-rs` currently uses GNU `--long` flags and lacks standard game modifiers (`-nomonsters`, `-respawn`, `-fast`, etc.) and response file (`@file`) parsing. The market standard is to support these vanilla arguments.

## ✅ Acceptance Criteria
- Add parsing for vanilla single-dash arguments alongside existing GNU `--long` flags.
- Implement the following specific launch modifiers:
  - `-nomonsters`
  - `-respawn`
  - `-fast`
  - `-turbo`
  - `-episode`
  - `-loadgame`
  - `-deathmatch` / `-altdeath`
  - `-file`
  - `@file` (response files)
- The standard vanilla command lines must launch equivalent sessions.

## 🚫 Out of Scope
- Graphical launcher UI.

## 📊 Success Metrics
- 100% of the specified vanilla arguments map correctly to engine state on startup.
