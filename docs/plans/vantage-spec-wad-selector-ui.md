# 🔭 Vantage: Spec for In-Game WAD Selector UI

## 👤 User Story
As a Player, I want to be able to select which WAD file to load from an in-game menu, so that I don't have to restart the application from the command line every time I want to play a different level or campaign.

## ❓ So What?
Currently, the engine requires the user to specify the WAD file via the command line or it relies on a hardcoded default. This is a significant point of friction for modern players who are accustomed to managing their game content directly within the application's interface. By implementing an in-game WAD selector, we lower the barrier to entry, encourage players to try different custom maps, and modernize the user experience. A seamless flow from launching the app to playing the game is critical for player retention.

## 📏 Metric Definition
- **Success Criteria:**
  - The menu successfully lists all `.wad` files found in a designated directory (e.g., `wads/`).
  - Selecting a WAD successfully unloads the current state and loads the new WAD without crashing.
  - Time from selection to entering the game is under 2 seconds.

## 🔍 Gap Analysis
- **Current State:** WAD loading is strictly tied to application startup arguments. There is no concept of hot-reloading game assets or returning to a root launcher state once a WAD is active.
- **Standard Libs / Market:** Modern source ports like GZDoom use external launchers (like ZDL) or have initial startup dialogs. However, having a completely integrated UI within the engine itself provides a superior, console-like experience.

## ✅ Acceptance Criteria
- Must provide a dedicated UI screen accessible from the main menu (or on initial launch if no WAD is specified).
- Must scan a pre-defined or configurable directory for `.wad` files.
- Must display the WAD filename and, if possible, extract and display basic metadata (e.g., title if it's a known IWAD).
- Must allow navigating the list and selecting an entry using standard menu controls.
- Must cleanly reset the game state when transitioning between WADs.

## 🚫 Out of Scope
- Support for complex PK3 files or arbitrary directory structures.
- Automatic downloading of WADs from the internet.
- Modifying the WAD files themselves (read-only access).
