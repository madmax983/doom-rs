# 🔭 Vantage: Spec for Gamepad Support

## 👤 User Story
As a Player, I want to use my gamepad (e.g., Xbox, PlayStation controller) to play the game, so that I can enjoy a more ergonomic and traditional console-like Doom experience from the comfort of my couch.

## ❓ So What?
Currently, the engine only supports keyboard input via terminal events. While functional for desk setups, many players prefer analog sticks for smoother movement turning and analog triggers for shooting. Adding gamepad support significantly lowers the barrier to entry and caters to players accustomed to modern source ports and official console releases. Without it, the engine feels strictly tied to the terminal hacker demographic rather than general gamers.

## 📏 Metric Definition
- **Success Criteria:**
  - Gamepad connections and disconnections are handled gracefully without crashing the app.
  - Left analog stick accurately translates to forward/backward movement  and strafing.
  - Right analog stick translates to turning.
  - Face buttons map to primary actions (Fire, Use/Open, Run, Map).

## 🔍 Gap Analysis
- **Current State:** Input is hardcoded to keyboard events within the terminal event loop. The input abstraction layer is already in place, making the backend ready.
- **Standard Libs / Market:** Standard industry libraries are available to provide cross-platform gamepad support and handle SDL-style controller mappings natively.

## ✅ Acceptance Criteria
- Must implement a new input polling thread or hook into the main loop using a standard gamepad library.
- Must translate gamepad stick axes to the `-128..127` range expected by the internal movement logic.
- Must support basic deadzone configuration so drifting controllers don't cause unwanted movement.
- Must document the default button mapping in `README.md` or a dedicated help screen.

## 🚫 Out of Scope
- A dedicated UI menu for rebinding controller buttons (stick to sensible hardcoded defaults or simple config file tweaks for now).
- Rumble / Force Feedback support.
- Support for exotic peripherals (steering wheels, flight sticks).
