# 🔭 Vantage: Spec for PWAD (Patch WAD) Support

## 👤 User Story
As a Player, I want to load custom levels and assets (PWADs) alongside the main game data (IWAD), so that I can play community-created content and expand the lifespan of the game.

## 💼 The "So What?" (Business Problem)
A game without modding support has a finite lifespan. By enabling PWAD loading, we unlock decades of community-created content (levels, graphics, sounds), transforming the engine from a static retro curiosity into a dynamic platform. This dramatically increases user engagement and retention.

## 📊 Metric Definition
- **Success** = 100% of standard vanilla Doom PWADs load without crashing or data corruption.
- **Success** = Asset resolution correctly prioritizes PWAD lumps over IWAD lumps.
- **Success** = Load time overhead for a single PWAD is less than 50ms compared to IWAD alone.

## 🕳️ Gap Analysis
- **Current State:** The engine currently only loads a single IWAD (Internal WAD) containing the core game assets.
- **Market Standard (Vanilla Doom/Chocolate Doom):** Players can specify `-file <wad1> <wad2> ...` on the command line, and the engine seamlessly overlays these patch WADs over the core IWAD, replacing existing assets (like sounds or graphics) or adding new maps.
- **The Gap:** We lack the ability to handle multiple WAD files simultaneously and merge their asset namespaces.

## ✅ Acceptance Criteria
1. **Command Line Support:** Users can specify one or more PWADs using a `-file` command-line argument.
2. **Asset Override:** If a lump exists in both the PWAD and the IWAD, the engine must use the PWAD version.
3. **Multiple PWADs:** If multiple PWADs are loaded, the last one specified on the command line takes precedence for asset overrides.
4. **Graceful Failure:** If a specified PWAD file is missing or corrupted, the engine must exit cleanly with a human-readable error message.
5. **Map Support:** Custom maps included in PWADs must be loadable via the command line or in-game menu.

## 🚫 Out of Scope
- Support for modern, advanced modding formats like PK3, UDMF (if requiring advanced ZDoom features), or DECORATE.
- DeHackEd patch support (BEX/DEH).
- Merging flats or textures from multiple PWADs that require complex texture lump rebuilding (PNAMES/TEXTURE1 merging can be Phase 2).
