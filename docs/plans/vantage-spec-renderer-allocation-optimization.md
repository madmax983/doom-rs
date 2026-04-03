# 🔭 Vantage: Spec for Renderer Allocation Optimization

## Context

Following an engineering analysis of the current game renderer performance, it was discovered that the renderer dynamically allocates memory per frame for sprite clipping histories. For a standard 35 FPS tick rate at a 320x200 resolution, this generates over 22,400 heap allocations per second.

This spec outlines the necessity of optimizing these rendering allocations from the perspective of software utility.

## 👤 User Story

"As a Player, I want the game renderer to maintain a stable 35 FPS without stuttering, so that my gameplay experience is smooth and uninterrupted."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Complexity and memory churn are costs. Excessive heap allocations per frame cause unnecessary allocator pressure and memory fragmentation, leading to measurable frame-timing micro-stutters during intense scenes or extended play sessions. Eliminating these hot-path allocations guarantees smooth frame pacing across hardware profiles, directly enhancing player immersion.

## ✅ Acceptance Criteria

### 1. Eliminate Dynamic Frame Allocations
- **Success Metric:** Sprite clipping histories use fixed-size memory allocated ahead of time or on the stack, instead of dynamically sizing per frame.
- **Criteria:**
  - The frame-by-frame outer collections for sprite clipping histories must be instantiated without dynamic heap allocations.

### 2. Rendering Parity
- **Success Metric:** Optimization does not alter the visual output of the engine.
- **Criteria:**
  - Sprite clipping behavior remains visually identical to vanilla Doom.
  - No new visual artifacts are introduced during frame rendering.

### 3. Engine Safety
- **Success Metric:** The memory usage remains stable without crashing.
- **Criteria:**
  - The memory footprint remains well within safe memory limits to avoid stack overflow crashes.
  - The solution must not introduce any new arbitrary architectural limits to rendering capacity.

## 🚫 Out of Scope

- **Third-Party Dependencies:** Adding new external packages to solve the inner collections problem.
- **Other Render Passes:** Optimizing non-sprite-clip rendering allocations (e.g., flats) is deferred to future optimization batches.
- **Multi-threading:** Parallelizing the renderer pipeline.
