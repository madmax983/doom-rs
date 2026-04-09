# 🔭 Vantage: Spec for Keybinding Configuration

## 👤 User Story
As a Player, I want to be able to customize my keyboard and controller inputs, so that I can play the game comfortably using my preferred layout instead of being forced into default controls.

## ❓ So What?
Currently, the engine has hardcoded default keybindings for movement, combat, and UI interactions. Players who rely on non-QWERTY layouts (like AZERTY or Dvorak) or who have specific accessibility needs are forced to use external tools or simply cannot play the game comfortably. Adding customizable keybindings is a standard accessibility and usability feature that immediately broadens our target audience and improves overall player satisfaction.

## 📏 Metric Definition
- **Success Criteria:**
  - Players can successfully remap primary actions (Move Forward, Move Backward, Turn Left, Turn Right, Fire, Use, Strafe) to any valid keyboard key or controller button.
  - Rebound keys are saved to the persistent configuration file and correctly loaded on startup.
  - The game correctly responds to the new inputs without noticeable latency or conflict errors.

## 🔍 Gap Analysis
- **Current State:** Keybindings are statically mapped in code (e.g., in `doom-tui` and `doom-game`), offering zero flexibility. The options menu has a "Controls" placeholder that does nothing.
- **Standard Libs / Market:** Modern games, including retro source ports, universally support input rebinding. We should implement a robust input mapping layer that translates raw device inputs (keys/buttons) into abstract semantic game actions, which the engine already partially supports via `TicInput`.

## ✅ Acceptance Criteria
- Must introduce an Input Mapping layer that translates raw keyboard/controller events into semantic game actions.
- Must allow saving and loading these mappings to/from a configuration file.
- Must gracefully handle missing or corrupted mappings by falling back to sane defaults.

## 🚫 Out of Scope
- Building a complex graphical UI for key rebinding in this initial phase (players will edit the config file manually for now).
- Support for advanced macros or multi-key combinations (e.g., Shift+W for sprint).
