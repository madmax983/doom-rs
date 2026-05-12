# 🔭 Vantage: Spec for Custom Keybinding UI

## 👤 User Story
As a Player, I want to be able to remap my keyboard and gamepad controls in a dedicated UI menu, so that I can play the game comfortably with my preferred layout (e.g., WASD instead of arrow keys) or accommodate accessibility needs.

## 🎯 The "So What?" Ask
Currently, control schemes are hardcoded or rely on out-of-game configuration editing. This is a massive friction point for modern gamers who expect WASD + Mouse look as standard, whereas classic Doom used arrow keys. A hardcoded control scheme alienates players with different keyboard layouts (like AZERTY or Dvorak) and limits accessibility. Providing a robust in-game rebinding UI is critical for player retention and basic modern UX standards.

## 📏 Metric Definition
- **Success Criteria:**
  - Players can open a "Controls" or "Customize Controls" menu from the Options screen.
  - Players can select an action (e.g., "Move Forward") and press a new key/button to bind it.
  - The UI visually reflects the new binding immediately.
  - Bindings are saved to the persistent configuration file and applied instantly to the active game session.
  - Duplicate bindings (e.g., binding "Fire" to the same key as "Use") either gracefully swap or display a clear warning without breaking the game state.

## ⚖️ Gap Analysis
- **Current State:** The Options menu has a "Controls" placeholder item (`MenuAction::Noop`). Key mappings are likely statically defined in `doom-tui/src/input.rs` or `event_loop.rs`. Gamepad support (`gilrs`) is being implemented but lacks dynamic rebinding.
- **Standard Libs / Market:** Almost all PC games provide key rebinding. In Rust, this involves maintaining a dynamic `HashMap<Action, InputEvent>` mapping instead of matching hardcoded events.

## ✅ Acceptance Criteria
- Must implement a new `MenuPage` (e.g., `MenuPage::CustomizeControls`) accessible from `MenuPage::Options`.
- Must list all primary actions (Move Forward, Move Backward, Turn Left, Turn Right, Strafe Left, Strafe Right, Fire, Use, Run, Map).
- Must implement an "input capture" state in the menu that waits for the next keystroke or gamepad button press to assign the binding.
- Must serialize the updated keymap to `doom_config.toml` (or equivalent persistent config).
- Must include a "Reset to Defaults" option.

## 🚫 Out of Scope
- Binding macros or multiple actions to a single key.
- Advanced mouse sensitivity curves or acceleration settings (handle simple sensitivity first).
- Rebinding UI for obscure debug commands or cheats.
