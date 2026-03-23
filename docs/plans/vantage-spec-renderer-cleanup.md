# 🔭 Vantage: Spec for Renderer Cleanup Parity

## Context

Following the Chocolate Doom Parity Audit and subsequent batch landings, we have restored significant source-faithfulness to the gameplay, rendering, and core audio logic. The `Batch C2` parity closeout successfully landed sky projection, visplane reuse hardening, and fixed door floor leaks. However, it intentionally deferred several long-tail rendering items to maintain stability. Complexity is a cost, and utility is revenue: we need to finish the final remaining renderer features to provide a true vanilla visual experience for players and eliminate visual glitches.

This spec focuses on the "What" and the "Why" for the remaining Renderer Cleanup pass.

## 👤 User Story

"As a Player and Custom Map Maker, I want the renderer to precisely match vanilla Doom's visual output, including all known rendering quirks and edge cases, so that custom wads and vanilla maps look exactly as they did in the original game without visual corruption."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Without accurate rendering parity, the engine fails its core mandate of parity. If a player or modder relies on specific vanilla rendering behaviors (like exact subsector draw order, precise visplane limits or behaviors, and sprite clipping quirks), and our engine behaves differently (or relies on hacky hardening sorts), it ruins the experience and breaks compatibility with existing complex custom maps. Achieving parity here ensures that our engine is a true drop-in replacement for Chocolate Doom in terms of visual presentation and avoids the technical debt of maintaining non-standard renderer crutches.

## ✅ Acceptance Criteria

### 1. Raw Subsector-Order Parity
- **Success Metric:** The renderer must process and draw subsectors in the exact same raw order as vanilla Doom.
- **Criteria:**
  - The hardening sort currently masking the renderer's weakness in nasty same-subsector cases must be removed.
  - The renderer must naturally handle same-subsector edge cases through faithful BSP traversal and segregation logic without relying on non-vanilla sorting crutches.

### 2. Deep Visplane and Sky Parity
- **Success Metric:** Visplane generation, merging, and sky rendering must match vanilla Doom's behavior perfectly.
- **Criteria:**
  - All remaining visplane nuances, including overflow conditions and plane merging logic, must act identically to the original game.
  - Sky rendering must be pixel-perfect with vanilla, removing any modern interpolations or projection deviations that remain.

### 3. Sprite Clip Parity
- **Success Metric:** Sprites must be clipped perfectly according to vanilla Doom's rules, without residual edge cases.
- **Criteria:**
  - The clip state must always be borrowed from the correct sector context.
  - Cases where sprites are incorrectly clipped by portal viewing windows (`mfloorclip`/`mceilingclip`) due to wrong context must be eliminated.

## 🚫 Out of Scope

- **Audio and Meta Systems:** Long-lived sound updates, attract-mode demo playback, and deeper input/demo audit are deferred to the Audio/Meta pass.
- **Savegame Parity:** True binary-compatible save/load parity is a separate, dedicated final pass.
- **Hardware Acceleration:** Any form of OpenGL/Vulkan rendering or non-fixed-point math optimizations; the renderer must remain a strictly fixed-point software rasterizer.
- **Gameplay/Combat Cleanup:** Exact `spechit` ordering, refire nuance, and damage-table parity are covered in a separate gameplay pass.

## ⚖️ Gap Analysis

The current renderer relies on a non-vanilla hardening sort to cover up weaknesses in nasty same-subsector cases, indicating that our raw subsector draw order is not truly faithful. Furthermore, while sky and visplane logic has improved, it still contains residual modernizations or inaccuracies compared to deep vanilla behavior. Finally, sprite clipping occasionally borrows state from the wrong sector context, leading to visual artifacts. These remaining items are critical for achieving full visual parity and removing technical debt from the renderer.
