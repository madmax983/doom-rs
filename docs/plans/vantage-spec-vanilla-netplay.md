# 🔭 Vantage: Spec for Vanilla Netplay

## Context
The engine currently has a rollback relay architecture. To achieve full Chocolate Doom parity (M8), we need a vanilla-compatible peer-lockstep mode and a dedicated server model that is distinct from the rollback relay.

## 👤 User Story
"As a Player, I want to play vanilla-compatible lockstep multiplayer, so that I can experience the original Doom network model and potentially interoperate with other retro source ports."

## 🎯 The "So What?" Ask
**What business problem does this solve?**
While rollback is great for modern play, historical preservation and parity with Chocolate Doom requires reproducing the original lockstep network model. This satisfies the strict-parity requirements and proves the engine's determinism is bit-exact with the original.

## ✅ Acceptance Criteria
- **Vanilla Peer-Lockstep Mode:**
  - **Success Metric:** Network simulation strictly pauses execution until all peer inputs for a given tic are received.
  - **Criteria:** The engine supports a strict lockstep synchronization model.
- **Dedicated Server Support:**
  - **Success Metric:** The game can be orchestrated by a headless server without requiring a participating player.
  - **Criteria:** A headless server mode can broker the lockstep connections.
- **Protocol Parity:**
  - **Success Metric:** Network traffic is formatted correctly such that the engine can interoperate in a multiplayer session with Chocolate Doom.
  - **Criteria:** Uses network structures matching the vanilla IPX/UDP formats.
- **Graceful Desync Handling:**
  - **Success Metric:** Game state inconsistencies are detected and the session halts rather than continuing in an invalid state.
  - **Criteria:** If a desync occurs, the engine cleanly halts rather than silently corrupting state.

## 🚫 Out of Scope
- Modern rollback netcode (handled separately).
- Mid-game join/drop-in (vanilla did not support this).
- Master server/server browser.

## ⚖️ Gap Analysis
Currently, the networking subsystem only implements a rollback-oriented relay transport. We need to introduce an alternative connection model that utilizes a strict lockstep polling loop. The main application loop must be adapted to block when lockstep data is unavailable, rather than predicting it.
