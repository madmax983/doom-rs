# 🔭 Vantage: Spec for Multiplayer Rollback Netcode

## Context

The engine currently supports single-player gameplay, but true Doom has always been a multiplayer experience. While a relay server and UDP client structure exists in `doom-net`, we need to implement a robust multiplayer experience over real-world internet connections. This means dealing with latency, packet loss, and jitter. A traditional lockstep networking model feels sluggish over high latency. To solve this, we will leverage client-side prediction and rollback netcode, allowing players to experience zero-latency movement while the engine resimulates past tics to correct mispredictions when remote data arrives.

## 👤 User Story

"As a Player, I want to play multiplayer Doom over the internet with my friends without my inputs feeling delayed, so that I can enjoy fast-paced deathmatch and coop even on connections with moderate latency."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Without functional, low-latency multiplayer, the engine is restricted to single-player and strictly local network use. Modern players expect online play to feel instantaneous. If inputs are delayed by network latency (as in traditional lockstep), the game feels unresponsive and unplayable for fast-paced combat. Implementing rollback netcode transforms the engine from an interesting technical port into a viable platform for modern, internet-based multiplayer Doom, significantly expanding its utility and user base.

## ✅ Acceptance Criteria

### 1. Client-Side Prediction
- **Success Metric:** The local player's input must be applied immediately to their viewpoint and movement without waiting for server acknowledgment.
- **Criteria:**
  - The local game state advances using local inputs while assuming remote players continue their last known actions.
  - The client must maintain a history of recent local inputs and game states to allow for rewinding.

### 2. Deterministic Rollback and Resimulation
- **Success Metric:** When remote inputs arrive that contradict the local prediction, the engine must transparently rewind and replay the game state to correct the divergence.
- **Criteria:**
  - Upon receiving authoritative data, the engine must rollback to the last verified state.
  - The engine must rapidly resimulate the game loop (re-applying local historical inputs and the newly received remote inputs) to catch back up to the present frame.
  - The game simulation must remain perfectly deterministic; the engine must resimulate identical outcomes given the same tic inputs.

### 3. Desync Detection
- **Success Metric:** The system must detect state divergence between clients and the server to prevent silent, irrecoverable desyncs.
- **Criteria:**
  - Clients must compute a checksum of the deterministic game state at specific, agreed-upon frames.
  - The server must compare checksums from all clients for a given frame and flag inconsistencies.

## 🚫 Out of Scope

- **Mid-game Joining (Drop-in/Drop-out):** For Phase 1, all players must connect in a pre-game lobby and launch the map together. State synchronization for late joiners is deferred.
- **Client-Server Architecture with Server Authority:** The server remains a blind relay (routing UDP packets). Clients run the authoritative simulation. True server-side authority with state reconciliation is out of scope.
- **TCP Fallback:** The transport layer is strictly UDP. Handling severe packet loss via TCP fallback is not planned; the input log redundancy will handle standard UDP packet loss.

## ⚖️ Gap Analysis

The foundational pieces exist: the engine is deterministic, and the networking subsystem contains logic for UDP transport, an input log, a snapshot history, and a rollback coordinator. However, these components are not fully wired into the main application loop. The main loop currently runs in a single-player paradigm or a naive polling mode. To bridge this gap, the event loop must be refactored to query the rollback coordinator for the number of frames to simulate (which may be >1 during a rollback) and feed the correct historical and predicted inputs to the game simulation.
