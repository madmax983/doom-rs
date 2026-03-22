# 🔭 Vantage: Spec for Renderer Cleanup Parity

## Context

Following the initial renderer parity batches (C1 and C2), significant improvements were made to sky projection, masked midtexture ordering, visplane reuse, and sector ownership. However, a few deep, source-faithful rendering nuances were intentionally deferred to prioritize stability. Complexity is a cost, and utility is revenue: we need to complete these final renderer cleanup items to ensure perfect visual parity with vanilla Doom.

This spec focuses on the "What" and the "Why" for the final Renderer Parity Cleanup pass.

## 👤 User Story

"As a Player, I want the renderer to perfectly mimic vanilla Doom's visual output and clipping rules, so that custom maps, advanced tricks, and sky visuals look exactly as the original authors intended."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Without complete renderer parity, certain complex map geometries, portal overlapping tricks, and sky projections will render incorrectly or exhibit visual glitches (like sprite clipping lies or incorrect draw order) that aren't present in Chocolate Doom. If our renderer cannot reliably render popular custom WADs or tricky vanilla maps exactly as they appear in Chocolate Doom, we fail our core mandate of parity. Achieving full renderer parity ensures our engine is a trusted, pixel-perfect replacement for the original.

## ✅ Acceptance Criteria

### 1. Subsector Raw-Order Parity
- **Success Metric:** Segments within a subsector must be processed in the exact same order as vanilla Doom.
- **Criteria:**
  - Remove the temporary hardening nearest-first sort in `crates/doom-renderer/src/seg.rs`.
  - Fix the underlying renderer weaknesses that made the hardening sort necessary, ensuring synthetic same-subsector portal and window cases render correctly under the vanilla subsector processing order.

### 2. Deep Visplane and Sky Parity
- **Success Metric:** Visplane allocation and sky projection must perfectly match vanilla behavior.
- **Criteria:**
  - Implement full vanilla parity for visplane overlap reuse and fresh-plane allocation after conflicts.
  - Implement full vanilla parity for sky vertical mapping (`skytexturemid`), going beyond the current simplified horizon-relative projection.

### 3. Residual Sprite Clip Parity
- **Success Metric:** Sprites must clip perfectly against level geometry, even in complex portal or overlapping sector scenarios.
- **Criteria:**
  - Eliminate residual edge cases where sprite clip state is borrowed from the wrong sector context.
  - Sprites viewed through two-sided linedefs (`mfloorclip`/`mceilingclip`) must correctly inherit their clipping boundaries from the appropriate visible sector, matching vanilla behavior.

## 🚫 Out of Scope

- **High-Res Rendering:** Adding support for high-resolution rendering, hardware acceleration, or floating-point precision rendering. We strictly require software-rendering fixed-point parity.
- **Audio and Meta Systems:** Long-lived sound updates, attract-mode demo playback, and input/demo sync are handled in a separate pass.
- **Savegame Parity:** True binary-compatible save/load parity is a separate, dedicated final pass.

## ⚖️ Gap Analysis

The current engine implements many correct rendering behaviors from Batches C1 and C2. However, it still relies on an explicit seg sorting hack (`seg.rs`) to prevent portal clipping leaks, which diverges from vanilla Doom's subsector processing order. Additionally, sky vertical mapping is simplified, and specific edge cases in sprite sector context borrowing can still cause minor visual clipping errors. Addressing these gaps will align the renderer perfectly with Chocolate Doom's `r_bsp.c`, `r_segs.c`, `r_plane.c`, and `r_things.c` behaviors.
