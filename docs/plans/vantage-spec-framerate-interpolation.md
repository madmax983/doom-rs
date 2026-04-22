# 🔭 Vantage: Spec for Framerate Interpolation

## 👤 User Story
As a Player, I want the game to render smoothly on high refresh rate monitors, so that movement feels fluid rather than stuttery at 35 FPS.

## ❓ So What?
The original Doom engine runs at a fixed 35 tics per second. On modern 144Hz or 240Hz monitors, rendering exactly at 35 FPS looks visually stuttery, degrading the player experience. By interpolating object positions between the last tic and the current tic based on the render time remainder, we can present a buttery smooth visual experience without altering the deterministic 35Hz physics simulation. Complexity is a cost, but fluid rendering is a massive utility for modern audiences.

## 📏 Metric Definition
- **Success Criteria:**
  - The renderer outputs unique, interpolated frames at the monitor's native refresh rate without breaking the 35Hz game state determinism.

## 🔍 Gap Analysis
- **Current State:** The game loop ticks at 35Hz and rendering is locked to those discrete tics.
- **Standard Libs / Market:** Modern source ports like GZDoom or Crispy Doom implement tic interpolation to decouple the rendering framerate from the fixed physics rate.

## ✅ Acceptance Criteria
- Must interpolate positions and rotations of the player view and all visible entities.
- Must not modify or break the core 35Hz game simulation state.
- Must be toggleable via a configuration option or CLI flag.

## 🚫 Out of Scope
- Changing the actual physics tick rate (e.g., to 60Hz or variable). The physics must remain strictly 35Hz for demo compatibility and determinism.
