# 🔭 Vantage: Spec for Turn-Based Mode

## Context

Recently, an alternative rendering mode (Codename: "Cogmind") was introduced, visualizing the game state as an ASCII grid instead of a 3D perspective. While this changes the presentation, the underlying game remains a real-time, 35Hz simulation. To fully realize the potential of this roguelike presentation, we need to introduce a true Turn-Based execution mode.

This spec defines the "What" and the "Why" for decoupling the game from real-time pacing and implementing turn-based mechanics.

## 👤 User Story

"As a Player, I want to play the game at my own pace where time only advances when I take an action, so that I can enjoy a tactical, roguelike experience using the new ASCII rendering mode."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
The new ASCII rendering mode is visually interesting but mechanically awkward when played in real-time. Players cannot easily parse an entire ASCII grid at 35 frames per second while simultaneously reacting to fast-moving monsters and projectiles. By introducing a turn-based mode, we transform what is currently a novelty renderer into an entirely new, highly playable game genre (a tactical roguelike). This radically expands the engine's utility, appealing to a different demographic of players while leveraging the exact same core simulation logic and WAD assets.

## ✅ Acceptance Criteria

### 1. Energy-Based Action System
- **Success Metric:** Time strictly advances based on player action cost, allowing monsters and projectiles to move proportionally.
- **Criteria:**
  - The real-time 35Hz tic loop is suspended.
  - Every action (move, attack, use, wait) costs a specific amount of "energy" or "time units".
  - When the player acts, the engine must simulate the exact number of vanilla tics required for that action to complete, allowing the AI and world to advance.

### 2. Paced Simulation Execution
- **Success Metric:** The game world correctly updates between player turns without skipping critical simulation steps.
- **Criteria:**
  - During the simulation phase (while the player is "recovering" energy), all active entities (monsters, projectiles, doors, platforms) must update their state.
  - The simulation must pause and wait for input the moment the player has enough energy to act again.

### 3. Mutually Exclusive Execution Modes
- **Success Metric:** Players can clearly choose between real-time or turn-based play at startup.
- **Criteria:**
  - A command-line flag (e.g., `--turn-based` or `--roguelike`) initiates the game in the new mode.
  - Turn-based mode should default to using the ASCII renderer, though it may technically support the 3D renderer.

## 🚫 Out of Scope

- **New AI Behaviors:** Monsters will use their existing state machines and pathfinding. We are not rewriting AI to be "smarter" for turn-based play in Phase 1.
- **Grid-Snapping Movement:** The underlying game state remains continuous (Fixed16_16 coordinates). We are not rewriting the physics engine to strictly lock entities to rigid grid squares; the renderer will simply continue to approximate their positions.
- **Turn-Based Multiplayer:** This feature is strictly for single-player.

## ⚖️ Gap Analysis

The engine currently enforces a strict 35Hz execution loop driven by real-time wall-clock pacing (or demo playback pacing). The `TicInput` system expects continuous polling. To implement this spec, the main application loop must be refactored to support a blocking, event-driven state machine. The game state must be advanced by a calculated number of tics *after* an input is received, rather than advancing one tic every 28.5 milliseconds. We also need to map vanilla Doom durations (e.g., weapon firing frames, player walking speed) into discrete energy costs for the player.