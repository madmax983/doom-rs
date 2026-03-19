# 🔭 Vantage: Spec for Renderer Parity Cleanup

## Context

Following the initial Chocolate Doom Parity Audit, the engine successfully landed Batch C2 renderer parity fixes, which addressed sky projection, visplane reuse hardening, and renderer-side sector ownership. However, true subsector-order raw parity and deeper visual edge cases were intentionally deferred to prevent immediate regressions. Complexity is a cost, and utility is revenue: we need to finish the final remaining renderer features to provide a true vanilla visual experience for players and modders.

This spec focuses on the "What" and the "Why" for the remaining Renderer Parity pass.

## 👤 User Story

"As a Player, I want the visual rendering to perfectly match Vanilla Doom's idiosyncrasies, so that advanced custom maps and visual tricks render exactly as the authors intended."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Without exact renderer parity, advanced vanilla maps that rely on specific engine quirks (like deep water, invisible stairs, precise sky rendering, or specific sprite clipping behavior) will look broken or glitchy. If a player or speedrunner encounters rendering artifacts that did not exist in Chocolate Doom, it breaks immersion and trust in the engine. Achieving true parity here ensures mod compatibility, visual authenticity, and completely faithful demo playback across complex geometry.

## ✅ Acceptance Criteria

### 1. Raw Subsector-Order Parity
- **Success Metric:** The order in which subsectors are traversed and drawn must strictly match vanilla Doom.
- **Criteria:**
  - The current hardening sort must be safely removed without introducing rendering glitches in same-subsector cases.
  - Pathological map layouts must draw walls and flats in the exact sequence expected by vanilla map authors.

### 2. Deep Visplane and Sky Parity
- **Success Metric:** Visplanes (floors/ceilings) and sky rendering must perfectly align with vanilla limits and projection rules.
- **Criteria:**
  - Edge cases involving overlapping or split visplanes must not create modern clipping artifacts or incorrect textures.
  - Sky rendering must respect vanilla horizon and tiling rules flawlessly across all vertical viewing angles.

### 3. Residual Sprite Clipping Parity
- **Success Metric:** Sprite clipping (things and weapon overlays) must flawlessly match vanilla, especially when viewing across differing sector contexts.
- **Criteria:**
  - Sprites must never borrow clip state from the wrong sector context.
  - Sprites partially obscured by mid-textures, varying floor heights, or complex window geometry must clip at the exact pixel boundaries as Vanilla Doom.

## 🚫 Out of Scope

- **Hardware Acceleration:** OpenGL, Vulkan, or other modern GPU-accelerated rendering methods. This engine is strictly software-rendered.
- **High Resolution / Widescreen:** Modern quality-of-life enhancements like high-res rendering or extended widescreen FOV. The focus is strictly on original aspect ratio and resolution parity.
- **Fixing Original Vanilla Engine Bugs:** "Fixing" visual bugs that existed in Vanilla Doom (e.g., slime trails, Hall of Mirrors effect caused by missing textures) is not the goal. If Vanilla Doom rendered a slime trail there, we must render a slime trail there.

## ⚖️ Gap Analysis

The current engine has an excellent and accurate software rasterizer port, but it relies on a hardening sort to cover a real renderer weakness in nasty same-subsector cases. This deviation means the raw subsector-order parity is not yet safe to land. Deeper visplane and sky details still remain beyond the currently landed fixes. Furthermore, residual sprite clip edge cases can still occur when clip state is accidentally borrowed from the wrong sector context. The gap is the remaining complex visual logic preventing 100% strict Vanilla Doom rendering compatibility.
