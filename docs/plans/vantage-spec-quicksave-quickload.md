# 🔭 Vantage: Spec for Quicksave & Quickload Parity

## Context

Following the core Savegame Parity pass, the engine is now capable of correctly serializing and deserializing deterministic game states to and from vanilla-compatible save files. However, modern players—and many classic players—rely heavily on the "Quicksave" (F6) and "Quickload" (F9) functionality for rapid iteration during difficult encounters. Currently, players must navigate through the full menu system to save or load, adding significant friction to the gameplay loop.

This spec defines the "What" and the "Why" for Phase 2 of Savegame Parity: integrating Quicksave and Quickload inputs directly into the core event loop and presentation layer.

## 👤 User Story

"As a Player, I want to save and load my game instantly with a single button press without navigating menus, so that I can quickly retry difficult encounters without breaking my immersion."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Friction in the core game loop causes player churn. Without quicksave/quickload functionality, players are forced to interrupt the action, open the menu, select a slot, and confirm every time they want to secure progress. Implementing this feature modernizes the UX while remaining faithful to the original Doom feature set, dramatically improving player satisfaction during challenging wads or Nightmare difficulty runs.

## ✅ Acceptance Criteria

### 1. Dedicated Input Bindings
- **Success Metric:** Pressing the Quicksave key (F6) and Quickload key (F9) triggers the corresponding action immediately during active gameplay.
- **Criteria:**
  - The bindings must only function during active gameplay (not on the title screen, intermission, or when the player is dead, unless specifically handling the "load after death" vanilla behavior).
  - The inputs must be intercepted by the event loop before reaching the core tic simulation to prevent accidental weapon switching or movement.

### 2. Slot Management & Prompting
- **Success Metric:** Quicksaving uses a designated slot and provides standard HUD feedback.
- **Criteria:**
  - If no save has been made yet in the current session, pressing Quicksave should briefly prompt the user to select a slot via the save menu, or automatically use a designated "Quicksave" slot (matching vanilla behavior).
  - Subsequent Quicksaves must overwrite the established slot without confirmation prompts.
  - The HUD must display a brief "Quicksaving..." message upon successful save.

### 3. Quickload Safety
- **Success Metric:** Quickloading restores the exact state without crashing or corrupting memory.
- **Criteria:**
  - Pressing Quickload (F9) prompts a simple "Quickload Game? (Y/N)" message on the HUD to prevent accidental data loss.
  - Pressing 'Y' instantly loads the last quicksaved slot.
  - If no quicksave slot exists, the game must fail gracefully with a HUD message (e.g., "No quicksave found").

## 🚫 Out of Scope

- **Auto-saving:** Automatically saving at the start of levels or checkpoints is a modern feature and out of scope for this parity pass.
- **Multiple Quicksave Slots:** Implementing rolling quicksave slots (e.g., Quicksave 1, Quicksave 2, Quicksave 3) diverges from vanilla and is excluded.
- **Cloud Sync Integration:** Automatically pushing quicksaves to external storage.

## ⚖️ Gap Analysis

The underlying save/load serialization machinery is already complete and robust thanks to the Phase 1 Savegame Parity work. The remaining gap is entirely in the input handling and frontend coordination layer. The terminal event loop currently maps standard movement and action keys, but does not intercept F6/F9 or route them to the savegame subroutines. Furthermore, the HUD message queue needs to be wired to handle the "Quicksaving..." notification and the interactive "Y/N" prompt required for safe quickloading.
