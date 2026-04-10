# 🔭 Vantage: Spec for Custom Keybindings

## 👤 User Story
As a Player, I want to be able to rebind the default keyboard controls in the Options menu, so that I can use a control scheme that is comfortable and familiar to me (e.g., WASD vs Arrow Keys).

## ❓ So What?
Currently, the "Controls" menu item is a placeholder, and the input mappings are hardcoded. This creates friction for players accustomed to modern control layouts or those who require accessibility adjustments. Without custom keybindings, we artificially limit our audience. This feature solves a core usability problem by increasing accessibility and reducing physical player friction.

## 📏 Metric Definition
- **Success Criteria:**
  - The "Controls" menu is fully functional, allowing players to view and assign new keys to actions (Move Forward, Turn Left, Fire, Use, etc.).
  - Rebound keys are saved persistently and loaded on launch.
  - The game responds correctly to the new inputs instead of the hardcoded defaults.

## 🔍 Gap Analysis
- **Current State:** The "Controls" menu does nothing. Keybindings are hardcoded as static matches in the input handler.
- **Standard Libs / Market:** Virtually all modern PC games, as well as every major Doom source port, offer customizable keybindings. It is considered a mandatory baseline feature for PC gaming.

## ✅ Acceptance Criteria
- Must introduce a way to map semantic actions to input keys.
- Must implement a functional "Controls" UI page accessible from the Options menu.
- Must support listening for the next keystroke to assign a binding.
- Must integrate with the existing persistent configuration system.
- Must safely handle conflicts (e.g., unbinding a key if it is assigned to another action).

## 🚫 Out of Scope
- Controller/Gamepad support (this is tracked in a separate spec).
- Mouse rebinding/mapping (focusing purely on keyboard inputs for this phase).
