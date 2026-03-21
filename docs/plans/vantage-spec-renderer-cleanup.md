# 🔭 Vantage: Spec for Renderer Parity Cleanup

## Context

Following the initial Chocolate Doom Parity Audit and subsequent renderer parity batches (C1 and C2), we have successfully implemented masked midtextures, correct texture height pegging, map-specific sky selection, and renderer-side sector ownership fixes. However, to achieve full renderer parity with vanilla Doom, we need to address the remaining technical debt. Complexity is a cost, and utility is revenue: we need to finish the final remaining renderer features to provide a true, visually authentic vanilla experience for players.

This spec focuses on the "What" and the "Why" for the remaining Renderer Parity pass.

## 👤 User Story

"As a Player and Custom Map Creator, I want the rendering engine to exactly replicate vanilla Doom's visual output—including rendering order and clipping—so that complex maps and visual tricks render correctly without glitches."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Without accurate raw subsector-order parity, deep visplane/sky parity, and exact sprite clipping, our renderer will fail on complex "torture" maps or visual tricks built specifically for the original Doom engine. Currently, the engine relies on a non-vanilla sorting workaround that masks underlying weaknesses. Removing this workaround and achieving true parity ensures that our engine is a robust, drop-in replacement for Chocolate Doom, capable of rendering any custom map exactly as the original authors intended.

## ✅ Acceptance Criteria

### 1. Raw Subsector-Order Parity
- **Success Metric:** The renderer must process subsectors in the exact same front-to-back order as vanilla Doom.
- **Criteria:**
  - The renderer must no longer require explicit nearest-first resorting of segments within a subsector to avoid visual glitches.
  - Complex same-subsector geometry, including far-wall and far-portal cases, must render correctly without leaking or over-clipping when the sorting workaround is removed.

### 2. Deep Visplane and Sky Parity
- **Success Metric:** The visual output of floors, ceilings, and skies must perfectly match vanilla Doom, including overlapping elements and edge cases.
- **Criteria:**
  - Visplane allocation, reuse, and overlap must mirror vanilla behavior exactly.
  - Sky rendering, including full-screen horizon-relative vertical mapping and row-sampling, must not stretch or distort incorrectly compared to vanilla.

### 3. Exact Sprite Clipping
- **Success Metric:** Sprites must be clipped correctly according to the sector they physically occupy, mirroring vanilla behavior.
- **Criteria:**
  - Sprites viewed through portals or near sector boundaries must use the correct sector context for clipping.
  - "Lies" where clip state is borrowed from the wrong sector context must be eliminated to prevent visual artifacts like sprites clipping into floors or ceilings incorrectly.

## 🚫 Out of Scope

- **Gameplay/Combat Cleanup:** Exact `spechit` ordering, refire nuance, and spawn parity are deferred to the gameplay parity pass.
- **Audio and Meta Systems:** Long-lived sound spatial refresh and true attract-mode demo playback are deferred to the audio/meta parity pass.
- **Hardware Acceleration:** Moving the renderer to OpenGL, Vulkan, or other hardware-accelerated backends is strictly out of scope. We are maintaining a pure software renderer for parity.

## ⚖️ Gap Analysis

The current engine has an excellent baseline renderer, but it relies on a hardening sort to cover a real renderer weakness in nasty same-subsector cases. Subsector segments are explicitly resorted nearest-first, which diverges from Doom's subsector processing order. Deeper visplane and sky parity still remain beyond the currently landed fixes, and residual sprite clip edge cases can still occur when clip state is borrowed from the wrong sector context. These discrepancies must be resolved to achieve full parity.
