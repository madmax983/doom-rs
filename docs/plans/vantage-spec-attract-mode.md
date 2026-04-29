# 🔭 Vantage: Spec for Attract-Mode Demo Playback

## 👤 User Story
As a Player launching the game, I want to see an "attract mode" that automatically loops through recorded gameplay demos after waiting on the title screen, so that I get an exciting preview of the game's action without pressing any buttons.

## ❓ So What?
Currently, demo playback is a functional developer tool but lacks the classic "arcade" presentation loop of the original Doom. In vanilla Doom, sitting idly on the title screen automatically loads pre-recorded demo files (DEMO1, DEMO2, DEMO3) from the WAD and plays them sequentially. This "attract mode" is a core part of the classic aesthetic and sets the mood before the player even starts playing. Re-implementing this feature closes an identified gap in parity (as noted in `2026-03-13-parity-closeout-and-remaining-work.md`) and enhances the authentic look and feel of the engine.

## 📏 Metric Definition
- **Success Criteria:**
  - After a specified idle timeout on the title/menu screen (e.g., ~10 seconds), the engine automatically starts playing `DEMO1`.
  - When `DEMO1` finishes, the engine transitions to the next state (e.g., title screen, then `DEMO2`, and so on).
  - Any input (keyboard or gamepad) instantly interrupts the demo playback and returns the player to the title/menu screen.

## 🔍 Gap Analysis
- **Current State:** Demo recording and playback functionality exists (e.g., input round-tripping works), but it must be manually invoked. There is no top-level state machine orchestration to handle idle timeouts and automatic demo sequencing.
- **Vanilla Parity:** The original Doom engine manages an explicit high-level loop that transitions between the title screen, menus, and demo playback based on timers and user input.

## ✅ Acceptance Criteria
- Must implement an idle timer in the title screen or main menu state.
- Must automatically load and play DEMO lumps from the main WAD when the idle timer expires.
- Must sequence through multiple demos (e.g., DEMO1, DEMO2, DEMO3) if available in the WAD.
- Must instantly abort playback and return to the menu upon receiving any player input (key press, mouse click, gamepad button).
- Must gracefully handle missing DEMO lumps (e.g., by resetting the idle timer or ignoring the demo state).

## 🚫 Out of Scope
- Creating new demos or re-recording the original vanilla demos.
- Advanced demo features like pausing, rewinding, or fast-forwarding during attract mode.
- Implementing the "credits" screen loop (which is technically part of the vanilla attract mode sequence, but we focus solely on the gameplay demos for this pass).
