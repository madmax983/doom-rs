# 🔭 Vantage: Spec for Gamepad Support

## 👤 User Story
As a Player, I want to be able to play the game using a standard gamepad (like an Xbox or PlayStation controller), so that I can enjoy a console-like experience from my couch or steam deck.

## ❓ So What?
Doom was originally a PC-centric game, but its modern incarnations are heavily played on consoles and portable PC devices (Steam Deck, ROG Ally). Relying exclusively on keyboard input severely limits the playability of the engine on these modern form factors. Adding robust gamepad support modernizes the engine and dramatically expands where and how players can enjoy the game.

## 📏 Metric Definition
- **Success Criteria:**
  - The game automatically detects connected XInput or standard DirectInput gamepads on launch or hotplug.
  - Core actions (Movement, Turning, Firing, Using, Weapon Switching) are mapped to standard modern controller layouts by default.
  - Analog stick input correctly scales turning/movement speed, rather than acting as strict digital on/off switches.
  - The player can navigate the main menu and options using the gamepad.

## 🔍 Gap Analysis
- **Current State:** The input system (likely `doom-tui/src/input.rs` or `event_loop.rs`) currently only polls terminal keyboard events.
- **Standard Libs / Market:** The Rust ecosystem provides excellent libraries like `gilrs` for cross-platform gamepad support. The current state is missing this integration entirely.

## ✅ Acceptance Criteria
- Must implement gamepad polling alongside existing keyboard input polling.
- Must provide a default button mapping that aligns with modern shooter conventions (e.g., Left Stick = Move, Right Stick = Turn, RT = Fire, A = Use).
- Must support analog input values, mapping stick deflection to movement/turn velocity.
- Must allow menu navigation via D-Pad or Left Stick, with A/B acting as Accept/Cancel.
- Must handle controller disconnects without crashing.

## 🚫 Out of Scope
- Advanced controller features like Rumble/Haptics or Gyro aiming.
- Custom button rebinding UI specifically for the gamepad (relying on defaults for Phase 1).
- Split-screen local multiplayer.
