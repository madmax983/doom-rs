# 🔭 Vantage: Spec for RNG Parity

## Context

Following the core gameplay, rendering, and audio parity passes, the engine operates predictably in most scenarios. However, the final foundational pillar for true Vanilla Doom compatibility—specifically for demo playback and speedrunning—is perfect Pseudo-Random Number Generator (PRNG) parity. Doom's behavior is entirely deterministic *if* the sequence of random numbers and the specific game events consuming them match perfectly.

This spec focuses on the "What" and the "Why" for the dedicated RNG Parity pass.

## 👤 User Story

"As a Player and Speedrunner, I want the game's random number generation to perfectly match vanilla Doom's RNG tables and index advancement, so that demos sync perfectly and combat feels authentic."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Without perfect RNG parity, any recorded demo or deterministic simulation (like rollback netcode) will eventually suffer a "butterfly effect" desync. A monster choosing a different patrol direction, an attack doing slightly more damage, or a projectile spreading differently will alter the entire course of the game from that point forward. Achieving 100% RNG parity is the prerequisite for stable demo playback, reliable multiplayer netcode, and exact combat authenticity.

## ✅ Acceptance Criteria

### 1. Exact RNG Table and Advancment
- **Success Metric:** The engine's core PRNG function (`P_Random`) must yield the exact sequence of 256 bytes found in the original Doom executable.
- **Criteria:**
  - The static lookup table must match vanilla byte-for-byte.
  - The global index must start at the correct initial value and advance exactly as it does in vanilla Doom on every call.

### 2. Deterministic Consumption
- **Success Metric:** Every system that relies on randomness must consume RNG values in the exact same order and at the exact same time as vanilla Doom.
- **Criteria:**
  - Damage calculations (e.g., `(p_random() % 8) + 1`).
  - Monster AI decisions (e.g., wandering, attack rolls, pain chance).
  - Weapon spread (e.g., shotgun pellets, chaingun inaccuracy).
  - Cosmetic effects that consume the game RNG (e.g., blood splats, debris).

### 3. Class-Specific RNG Separation
- **Success Metric:** Cosmetic or modern engine features must not pollute the deterministic gameplay RNG sequence.
- **Criteria:**
  - Screen wipes, modern particle systems, or UI elements must use a separate, non-game-state RNG to avoid advancing the main index and causing desyncs.

## 🚫 Out of Scope

- **Modern RNG Algorithms:** We will not replace the 256-byte lookup table with a "better" algorithm (like Mersenne Twister) for gameplay logic. Parity demands the original, flawed distribution.
- **Fixing RNG Manipulation Tricks:** Speedrunner techniques that rely on manipulating the RNG index (e.g., firing a weapon to advance the index to a favorable position) must remain fully functional.

## ⚖️ Gap Analysis

The engine currently has a `p_random()` implementation (`crates/doom-game/src/random.rs`) that uses a table, but a complete audit of *where* and *when* `p_random()` is called across all subsystems has not been finalized. The combat, movement, and spawn routines have seen partial parity updates, but hidden deviations (such as an extra or missing RNG call during a specific monster state transition) will cause immediate desyncs during demo validation. We need to lock down the table implementation and exhaustively verify every consumption site against the Chocolate Doom reference.
