# 🔭 Vantage: Spec for Menu & HUD Parity

## Context

Following the Chocolate Doom Parity Audit, we have successfully restored significant source-faithfulness to gameplay, rendering, and core audio logic. However, the game's presentation layer still relies on placeholder text and rectangle rendering for menus, the status bar, the HUD face, and the font. To achieve a true vanilla visual experience and fully integrate WAD asset loading into the presentation layer, we must implement WAD patch-based rendering for these elements. Complexity is a cost, and utility is revenue: we need a presentation layer that seamlessly matches the original game's aesthetic and correctly conveys information to the player.

This spec defines the "What" and the "Why" for replacing placeholder UI elements with authentic WAD patch graphics.

## 👤 User Story

"As a Player, I want the menus, status bar, and HUD to display the original Doom graphics, fonts, and face animations, so that the game feels authentic, nostalgic, and visually cohesive."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Without accurate WAD patch rendering for the UI, the engine feels unfinished and unpolished, breaking immersion. If the UI relies on modern, system-rendered fonts or geometric shapes instead of the original textures, it immediately signals to players and modders that this is a non-standard port. Implementing this parity removes the last major visual placeholder, ensuring the engine looks exactly like Vanilla Doom and correctly supports WADs that replace UI assets (like custom fonts, status bars, or Doomguy faces).

## ✅ Acceptance Criteria

### 1. Status Bar Parity
- **Success Metric:** The entire status bar is rendered using WAD patches at their exact original pixel coordinates.
- **Criteria:**
  - The background (`STBAR`), arms box, health/armor digits (`STTNUM`, `STTPRCNT`), ammo counts (`STYSNUM`), and key icons (`STKEYS`) must be drawn using their respective WAD patches.
  - Numbers must be correctly right-aligned using individual digit patches.

### 2. Face Animation State Machine
- **Success Metric:** The Doomguy mugshot behaves exactly like the original game, prioritizing pain, god mode, rampage, and idle states correctly.
- **Criteria:**
  - The face must update dynamically based on health tiers, damage taken, invulnerability, and weapon firing status.
  - A priority cascade must ensure that high-priority states (e.g., Ouch, God Mode) override lower-priority states (e.g., idle look, Evil Grin) for the correct duration (typically 35 tics).
  - Damage direction must influence the pain face variant (left, right, center).

### 3. Menu & Font Rendering
- **Success Metric:** All menus (Main, Episode, Skill, Load/Save, Options) and HUD messages are rendered using WAD menu patches and the WAD font.
- **Criteria:**
  - Standard menu headers and items must use their specific WAD patches (e.g., `M_DOOM`, `M_NGAME`, `M_OPTTTL`).
  - Text rendered dynamically (like save slot names or HUD messages) must use the WAD font (`STCFN033`–`STCFN095`), supporting variable-width spacing and correct centering.
  - The skull cursor (`M_SKULL1`/`M_SKULL2`) must animate and position itself correctly relative to the selected menu item.

## 🚫 Out of Scope

- **Functional Options Menu Sliders:** Visual stubs for sliders are acceptable; fully interactive sliders are out of scope for this visual parity pass.
- **Finale Text Crawl:** The end-of-episode text crawl sequence is a separate presentation state and is deferred.
- **Pause Screen:** The visual "PAUSE" indicator is trivial and can be implemented, but deep pause state mechanics are not the primary focus here.
- **High-Resolution UI:** Any form of upscaling or smooth font rendering; the UI must strictly adhere to the original 320x200 pixel-art resolution and rendering methods.

## ⚖️ Gap Analysis

The current engine uses generic text rendering and solid color rectangles to represent UI elements. It lacks a generalized `draw_patch` function that handles transparent posts and offsets within the 320x200 screen bounds. Furthermore, there is no `PatchCache` or `WadFont` system to efficiently load and render these specific UI assets from the `WadStack`. Finally, the `FaceState` logic is currently missing or simplified, failing to capture the complex, priority-based state machine of the original Doomguy mugshot.
