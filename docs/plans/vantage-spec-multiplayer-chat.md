# 🔭 Vantage: Spec for In-Game Multiplayer Chat

## 👤 User Story
As a Multiplayer Gamer, I want to communicate with other players in-game via text, so that we can coordinate strategies, socialize, and discuss match settings without relying on external voice or text applications.

## ❓ So What?
Multiplayer games without native communication tools isolate players. Adding a native text chat feature reduces the friction of coordination, improves team cohesion in co-op modes, and provides the fundamental social layer expected in any multiplayer experience. Without this, players are forced to use third-party tools, which breaks immersion and creates a barrier for pick-up groups.

## 📏 Metric Definition
- **Success Criteria:**
  - Players can initiate chat input with a dedicated hotkey (e.g., `t`).
  - While typing, normal game movement inputs are suppressed.
  - Pressing `Enter` broadcasts the message to all connected clients.
  - Received messages appear in the HUD message queue and fade out after a set duration.
  - The chat payload must be integrated efficiently into the existing network tick without causing stutter.

## 🔍 Gap Analysis
- **Current State:** The game currently supports a functional UDP-based multiplayer loop (`doom-net`) and can display pre-defined HUD messages (`doom-app`). However, there is no system to capture free-form user text input during gameplay, nor a network packet type defined for arbitrary string broadcasts.
- **Market Standard:** Almost every multiplayer game since the 1990s provides a global text chat baseline. Our current engine lacks this basic feature.

## ✅ Acceptance Criteria
- Must introduce a "chat input mode" in the application layer that captures keystrokes until `Enter` or `Esc` is pressed.
- Must extend the networking protocol to transmit chat messages between clients/server.
- Must render received chat messages (e.g., `Player 1: watch out behind you`) in the existing HUD message system.

## 🚫 Out of Scope
- Voice Chat.
- Team-only or private whispering channels.
- Profanity filters or moderation tools.
