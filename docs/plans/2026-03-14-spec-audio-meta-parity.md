# 🔭 Vantage: Spec for Audio & Meta System Parity (Batch D Phase 2)

Date: 2026-03-14

## Context

Following the Chocolate Doom Parity Audit and subsequent batch landings, we have restored significant source-faithfulness to the gameplay, rendering, and core audio logic. However, the `Batch D` audio and meta systems pass intentionally deferred several long-tail items. Complexity is a cost, and utility is revenue: we need to finish the final remaining audio and demo features to provide a true vanilla experience for players.

This spec focuses on the "What" and the "Why" for the remaining Audio & Meta System Parity pass.

## 👤 User Story

"As a Player, I want continuous spatial audio tracking and true demo playback, so that the audiovisual experience and title sequence perfectly matches vanilla Doom."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Without accurate continuous spatial audio parity, sounds from moving objects (like platforms or monsters) feel static and disconnected from the world. Without true attract-mode demo playback, the title screen feels lifeless and incomplete, lacking the iconic hook of the original game. Achieving true parity here eliminates the final major friction points for purists and speedrunners.

## ✅ Acceptance Criteria

### 1. Continuous Spatial Refresh Parity
- **Success Metric:** Panning and volume of active, long-lived sounds (e.g., doors opening, platforms moving, long monster attacks) must update dynamically each frame.
- **Criteria:**
  - If a sound origin moves relative to the player, its pan and volume must smoothly adjust in the audio output.
  - The game must not blindly play a sound at its initial computed spatial coordinates if the source or listener moves during playback.
  - The implementation must respect the original game's "one live channel per origin" rule during these updates.

### 2. True Attract-Mode Demo Playback
- **Success Metric:** Leaving the game idle at the title screen plays a sequential loop of the game data's built-in demo recordings.
- **Criteria:**
  - The game must automatically cycle between the title screen, the credits screen, and demo playback phases when no user input is received.
  - Pressing any key during demo playback must immediately abort the demo and return to the title screen / menu.

### 3. Input & Demo Sync Parity
- **Success Metric:** Demo playback accurately reproduces original game recordings without desynchronization.
- **Criteria:**
  - Game cadence and input semantics during demo playback must be deterministic and match the original recording format perfectly.
  - The random number generator seed and sequence must align with vanilla behavior to prevent butterfly-effect desyncs during combat or monster movement in recorded demos.

## 🚫 Out of Scope

- **Savegame Parity:** True binary-compatible save/load parity is a massive undertaking and is deferred to a dedicated final pass (System 5).
- **Network Parity:** True peer-to-peer lockstep networking is out of scope for this pass; the current relay server approach remains the standard.
- **Advanced Demo Features:** Features like rewinding, fast-forwarding, or free-cam during demo playback are considered modern enhancements and out of scope for *parity*.

## ⚖️ Gap Analysis

The current engine has a priority-based audio mixer and a robust music player, but it lacks the game-loop integration to continuously update the spatial positioning of active sound channels. The demo subsystem can record and parse recordings, but the title screen phase currently falls back to a fake loop instead of loading and running these demos automatically. Standard system libraries provide the necessary output streams, but the logic bridging the game state to the audio engine (and the title sequence state machine) needs to be completed.
