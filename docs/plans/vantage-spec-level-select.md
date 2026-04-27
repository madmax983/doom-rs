# 🔭 Vantage: Spec for In-Game Level Selector

## 👤 User Story
As a Player, I want to be able to select and warp to any specific level directly from the in-game menu, so that I can easily practice specific sections, resume my progress without save files, or skip levels I have already beaten without needing to restart the game from the terminal with CLI arguments.

## ❓ So What?
Currently, players must either play sequentially from E1M1, rely on save states, or manually restart the application using the `--warp` CLI argument to start at a specific map. This is highly disruptive to the user experience, especially in a terminal context where dropping back to the shell breaks immersion. An in-game level selector vastly improves accessibility, utility, and user retention by keeping the player inside the application.

## 📏 Metric Definition
- **Success Criteria:**
  - A new "Level Select" or "Warp" option is added to the Main Menu or Options Menu.
  - The feature allows the user to input or select an Episode (1-4) and Map (1-9) for Doom 1, or Map (1-32) for Doom 2/PWADs.
  - Upon selection, the game immediately transitions to the chosen level, properly resetting player state (health, ammo, inventory) to pistol-start defaults, exactly as if `--warp` was used.

## 🔍 Gap Analysis
- **Current State:** Level warping is strictly tied to application startup arguments (`--warp`). There is no UI for it.
- **Standard Libs / Market:** Almost all modern Doom source ports and retro remasters include a level select feature in the UI, recognizing that players frequently want to replay specific iconic maps (like E1M1 or MAP01) without hassle.

## ✅ Acceptance Criteria
- Must add a navigable "Level Select" UI state within the terminal menus.
- Must present a valid list of selectable levels based on the loaded WAD (e.g., dynamically detect if Doom 1 or Doom 2 format).
- Must successfully trigger a map change event and load the new level.
- Must ensure the player starts the new level with default starting gear (Pistol, 50 bullets, 100% health).

## 🚫 Out of Scope
- A visual thumbnail gallery of the maps.
- Tracking which maps the player has "unlocked" (all maps are available by default).
- Modifying the underlying BSP/map loading logic; this feature strictly leverages the existing warp capabilities via a new UI frontend.
