👤 **User Story:** As a Player, I want to use my gamepad (e.g., Xbox, PlayStation controller) to play the game, so that I can enjoy a more ergonomic and traditional console-like Doom experience from the comfort of my couch.

✅ **Acceptance Criteria:**
- Must translate gamepad stick axes to the appropriate movement range expected by the engine.
- Must support basic deadzone configuration so drifting controllers don't cause unwanted movement.
- Must document the default button mapping in the documentation or a dedicated help screen.

🚫 **Out of Scope:**
- A dedicated UI menu for rebinding controller buttons (stick to sensible hardcoded defaults or simple config file tweaks for now).
- Rumble / Force Feedback support.
- Support for exotic peripherals (steering wheels, flight sticks).
