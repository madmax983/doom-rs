# 🔭 Vantage: Spec for Controller Support

## Context

The engine currently relies entirely on terminal input, meaning it is strictly bound to keyboard interaction (and eventually mouse via the graphical frontend spec). While this stays true to PC roots, the modern gaming landscape is hardware-agnostic. Many users expect to play retro shooters using a gamepad, whether on a couch, a Steam Deck, or a desktop PC.

This spec defines the "What" and the "Why" for introducing standard Controller Support.

## 👤 User Story

"As a Player, I want to play the game using a standard game controller (like an Xbox or PlayStation controller), so that I can enjoy a comfortable, console-like experience without needing a keyboard and mouse."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Modern gamers heavily index on controller compatibility. Without it, the engine caters only to purists and desktop-bound users. Adding standard gamepad support dramatically increases accessibility and the engine's utility in modern living room setups and handheld PCs. Complexity is a cost, but this utility taps directly into how a massive segment of people actually play retro shooters today, significantly expanding the potential user base.

## ✅ Acceptance Criteria

### 1. Seamless Input Mapping
- **Success Metric:** The game automatically detects standard XInput/SDL-compatible controllers and maps inputs to game actions without manual configuration.
- **Criteria:**
  - Analog sticks map to movement and turning with appropriate deadzones.
  - Face buttons and triggers map to firing, interacting, and weapon switching.
  - The controller must be able to navigate the game's menus natively.

### 2. Plug and Play Reliability
- **Success Metric:** Connecting or disconnecting a controller does not crash the game or require a restart.
- **Criteria:**
  - Hot-plugging is supported (connecting a controller mid-game immediately activates it).
  - If a controller is disconnected, the game seamlessly falls back to keyboard/mouse input.

### 3. Input Determinism
- **Success Metric:** Controller input must serialize perfectly into the existing game input event structure for demo recording and netcode compatibility.
- **Criteria:**
  - Analog inputs must be cleanly quantized into the game's movement values without breaking the deterministic engine loop.

## 🚫 Out of Scope

- **Custom Button Remapping UI:** Building an in-game graphical interface to change button layouts is Phase 2. The standard default layout is sufficient for Phase 1.
- **Rumble/Haptic Feedback:** Force feedback adds complexity and is deferred to Phase 2.
- **Gyro Aiming:** Not supported in Phase 1.

## ⚖️ Gap Analysis

Currently, the game loops only poll terminal keystrokes. We lack an input abstraction layer that can listen to OS-level game controller events. Implementing this will require evaluating a minimal dependency that aligns with our low-dependency philosophy. Crucially, these new analog inputs must be piped into the existing input pipeline reliably, ensuring that demo playback and deterministic simulations are unaffected by the input source.
