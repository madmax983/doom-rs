# 🔭 Vantage: Spec for Doom II Cast Call and Finale Sequences

## Context

The engine currently lacks support for the complex end-game sequences found in Doom II, such as the monster cast call and the finale art/bunny screens. This missing polish prevents players from experiencing the authentic vanilla conclusion to the game.

## 👤 User Story

"As a Player who just beat Doom II, I want to watch the iconic cast call of monsters and the finale art, so that I can experience the authentic, satisfying conclusion to the campaign."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Achieving simulation parity isn't just about in-game mechanics; it's about delivering the complete, intended player journey from boot up to the credits. Missing the Doom II cast call and finale art leaves the game feeling incomplete for users playing retail IWADs. Adding these finale screens fulfills the M9 Parity Roadmap goal ("Content polish") and provides the necessary closure players expect, increasing the port's perceived quality and fidelity to Chocolate Doom.

## ✅ Acceptance Criteria

### 1. Doom II Cast Call
- **Success Metric:** The engine correctly transitions to the cast sequence after completing the final level in Doom II.
- **Criteria:**
  - The sequence must display the correct monster sprites.
  - The sequence must loop through all defined monsters.

### 2. Finale Art and Bunny Screens
- **Success Metric:** The engine displays the correct post-episode or game completion screens.
- **Criteria:**
  - The image must remain on screen with the appropriate background music until dismissed by the player.

## 🚫 Out of Scope

- **Custom Intermission Sequences:** Adding scripting to allow modders to define custom cast calls or intermissions is out of scope. We are strictly aiming for vanilla IWAD hardcoded behavior in this phase.
- **Non-Vanilla High-Res Assets:** Upscaling or rendering high-resolution replacements for the cast sprites or finale screens is not part of this specification.

## ⚖️ Gap Analysis

The engine currently lacks a dedicated state with the necessary update and render loops to handle the cast call. New logic is needed to iterate through the cast, fetching the correct sprite frames and sound IDs. The presentation layers will also need updates to draw these standalone graphical screens outside of the normal 3D view.
