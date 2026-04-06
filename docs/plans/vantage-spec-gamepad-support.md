# 🔭 Vantage: Spec for Gamepad Support

## Description

👤 **User Story:**
As a Player, I want to play the game using a standard gamepad or controller, so that I can enjoy a comfortable, console-like experience on my computer.

**So What? (Business Problem):**
Keyboard-only controls alienate players who prefer modern controller setups or are playing on handheld PC devices (like the Steam Deck). Without this, our reach is limited to traditional desktop users. Adding gamepad support expands our total addressable audience and modernizes the gameplay experience.

**Success Metric Definition:**
- Success = 99% of standard XInput/DirectInput controllers are automatically detected without manual configuration.
- Success = Gamepad input latency is indistinguishable from keyboard input latency (<10ms).

✅ **Acceptance Criteria:**
- **Plug-and-Play:** Standard gamepads (Xbox, PlayStation, generic USB) are automatically detected and mapped to sensible default controls (e.g., Left Stick to move, Right Trigger to fire).
- **Menu Navigation:** The gamepad can be used to navigate all in-game menus, including the title screen and pause menus.
- **Hot-swapping:** The game seamlessly transitions between keyboard and gamepad input if the user switches mid-game.
- **Graceful Disconnect:** If a gamepad is disconnected during gameplay, the game should automatically pause.

🚫 **Out of Scope:**
- Fully customizable button remapping UI (Phase 2).
- Haptic feedback / Rumble support (Phase 2).
- Gyro aiming.
