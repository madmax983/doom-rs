# 🔭 Vantage: Spec for UI Crosshair Toggle

## 👤 User Story
As a Player, I want to be able to toggle a crosshair overlay in the center of the screen, so that I can easily aim weapons in the low-resolution terminal environment.

## ❓ So What?
Currently, aiming relies purely on the weapon sprite alignment, which can be ambiguous due to terminal character aspect ratios and bilinear scaling modes. A crosshair significantly reduces friction in combat, lowering the barrier to entry for players unaccustomed to terminal-based renderers, directly improving the "Human Interface" and accessibility of the game.

## 📏 Metric Definition
- **Success Criteria:**
  - A new "Crosshair" option is added to the Options menu.
  - The crosshair setting is persisted in the game configuration.
  - When enabled, a visible, non-obtrusive crosshair character (e.g., `+`) is drawn at the exact center of the 3D viewport.
  - The feature must not negatively impact rendering loop performance (framerate drop < 1%).

## 🔍 Gap Analysis
- **Current State:** The renderer draws the 3D view and weapon sprites, but there is no central aiming reticle overlay.
- **Standard Libs / Market:** Almost all modern Doom source ports (GZDoom, PrBoom+, Crispy Doom) offer a crosshair toggle. It has become an expected baseline feature for any first-person shooter, even retro ones.

## ✅ Acceptance Criteria
- Must add a `crosshair: bool` field to the persistent configuration state.
- Must add an interactive toggle in the `GameMenu` under Options.
- Must modify the terminal UI renderer to overlay a crosshair in the center of the `viewport` buffer just before final blitting to the terminal.
- Must ensure the crosshair is centered correctly regardless of terminal resizing.

## 🚫 Out of Scope
- Dynamic crosshairs that expand based on weapon spread or player movement.
- Customizing the crosshair shape or color in the UI (hardcoded `+` and a high-contrast color is sufficient for Phase 1).
