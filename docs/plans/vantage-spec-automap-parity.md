# 🔭 Vantage: Spec for Automap Parity

## Context

Following the core gameplay, rendering, and audio parity passes, the engine's presentation layer is coming into focus. While the primary 3D view accurately represents the Doom world, the overhead Automap is currently a simplified implementation. It provides basic navigation but lacks the exact visual style, nuanced linedef coloring, and full feature set (like panning, zooming, and specific cheat reveals) of the original game.

This spec focuses on the "What" and the "Why" for the Automap Parity pass.

## 👤 User Story

"As a Player, I want the Automap to look and behave exactly like the original Doom automap, so that I can navigate complex levels efficiently and experience the game precisely as it was designed."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
The Automap is a critical tool for players, especially in labyrinthine classic maps. If the Automap is difficult to read, uses the wrong colors for specific linedef types (like doors or secret walls), or lacks the fluid panning/zooming controls of the original, it frustrates the player and breaks the promise of a true vanilla experience. Achieving parity here ensures the game feels polished and provides the exact tactical information players expect.

## ✅ Acceptance Criteria

### 1. Visual Parity & Linedef Coloring
- **Success Metric:** The automap must draw lines using the exact color mappings found in vanilla Doom.
- **Criteria:**
  - Walls (1-sided lines), floor height changes, ceiling height changes, and undiscovered areas must be color-coded correctly.
  - The player's arrow marker must be drawn accurately and reflect their current rotation.
  - Thing markers (when revealed by cheats or powerups) must use the correct shapes and colors (e.g., green for items, red for enemies).

### 2. Input and Interaction Parity
- **Success Metric:** The player can toggle, zoom, pan, and follow smoothly using the standard bindings.
- **Criteria:**
  - The map must toggle on and off with the `Tab` key (or bound equivalent).
  - The map must support zooming in and out.
  - The map must support panning independently of the player's position.
  - The "Follow Mode" (toggleable) must lock the camera to the player, updating the center coordinates dynamically.

### 3. Cheat Parity (IDDT)
- **Success Metric:** The `IDDT` cheat must cycle through the correct reveal states.
- **Criteria:**
  - First use: Reveals the entire map geometry.
  - Second use: Reveals all map things (enemies, items, decorations).
  - Third use: Resets to normal, explored-only visibility.

## 🚫 Out of Scope

- **Textured Automaps:** Modern source ports sometimes draw flat textures on the automap polygons. This is an advanced visual enhancement and strictly out of scope for *vanilla parity*.
- **High-Resolution Vector Rendering:** The automap should conceptually represent the original pixel-art style, even if drawn via lines in the terminal. Anti-aliased high-res vector graphics are not a requirement.
- **Custom Waypoints:** Allowing the player to place custom markers or draw on the map is not a vanilla feature.

## ⚖️ Gap Analysis

The current engine has a basic Automap implementation that handles tracking the active toggle and following the player's position. However, the drawing logic currently implements a simplified version of the visual representation. The cheat logic for `IDDT` needs to be fully integrated with the automap state to correctly cycle through the reveal tiers, and the rendering functions must be audited to ensure strict adherence to vanilla line coloring and marker logic.
