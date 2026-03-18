# 🔭 Vantage: Spec for Gameplay and Combat Cleanup

## Context

Following the Chocolate Doom Parity Audit and subsequent batch landings, we have restored significant source-faithfulness to the gameplay, rendering, and core audio logic. However, the `Batch C2` parity closeout intentionally deferred several long-tail items, primarily focusing on "Gameplay and combat cleanup" as the next priority. Complexity is a cost, and utility is revenue: we need to finish the final remaining gameplay and combat features to provide a true vanilla experience for players.

This spec focuses on the "What" and the "Why" for the remaining Gameplay and Combat Parity pass.

## 👤 User Story

"As a Player, I want the gameplay and combat mechanics to precisely match vanilla Doom, so that the game feels authentic and challenging."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Without accurate gameplay and combat mechanics, the engine fails its core mandate of parity. If a player relies on specific vanilla behaviors (like exact `spechit` encounter ordering, precise refire nuances, or exact damage tables), and our engine behaves differently, it ruins the experience and breaks compatibility with existing speedruns or custom maps. Achieving parity here ensures that our engine is a true drop-in replacement for Chocolate Doom in terms of core gameplay loops.

## ✅ Acceptance Criteria

### 1. Exact `spechit` Encounter Ordering
- **Success Metric:** The order in which special linedefs are triggered (`spechit` ordering) must be 100% deterministic and match vanilla Doom exactly.
- **Criteria:**
  - Pathological cases where multiple special linedefs are crossed simultaneously must resolve in the exact same sequence as vanilla.
  - This ordering must be verified against known vanilla demos that rely on specific `spechit` behavior to progress.

### 2. Deep Refire and State Nuance Parity
- **Success Metric:** Weapon refire mechanics and state transitions must perfectly replicate vanilla behavior.
- **Criteria:**
  - Holding down the fire button must result in the exact same refire delay and state transitions as vanilla.
  - Edge cases involving weapon switching or immediate refiring must not deviate from the original game's frame-perfect logic.

### 3. Damage Table and RNG Parity
- **Success Metric:** Damage calculations must perfectly align with vanilla damage tables and RNG sequences.
- **Criteria:**
  - Every attack (projectile, hitscan, melee) must use the correct vanilla damage formula and lookup table.
  - The sequence of RNG values consumed for damage calculation must match vanilla, ensuring 0% desync in demo playback.

### 4. Spawn Edge Case Parity
- **Success Metric:** Map thing spawning logic must match vanilla, including all specific flag-handling edge cases.
- **Criteria:**
  - Broader map-thing spawn parity and remaining flag-specific edge cases must be swept and implemented.
  - This includes precise handling of floor support, ceiling-spawn flags, and blocked Nightmare respawns.

## 🚫 Out of Scope

- **Renderer Cleanup:** Subsector raw-order parity, visplane/sky details, and sprite clip cases are deferred to the next pass.
- **Audio and Meta Systems:** Long-lived sound updates, attract-mode demo playback, and deeper input/demo audit are deferred.
- **Savegame Parity:** True binary-compatible save/load parity is a separate, dedicated final pass.

## ⚖️ Gap Analysis

The current engine has made significant strides in gameplay and combat parity, but several deep, source-faithful nuances remain. The exact ordering of `spechit` encounters, the frame-perfect refire logic, and the precise damage calculations are not fully audited or implemented. The spawn logic, while improved, still lacks coverage for all flag-specific edge cases. These remaining items are critical for achieving full parity with vanilla Doom's gameplay loop.
