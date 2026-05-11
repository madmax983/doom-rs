# 🔭 Vantage: Spec for DeHackEd Patch Support

## 👤 User Story
As a Player, I want to load custom DeHackEd patches (.deh/.bex), so that I can play classic community mods that feature new weapons, customized enemies, and altered gameplay behavior without needing a modern scripting language.

## ❓ So What?
**What business problem does this solve?**
A significant portion of the classic Doom modding ecosystem relies on DeHackEd patches to modify monster behavior, weapon fire rates, and text strings. Currently, our engine can only play pure Vanilla maps or maps that don't rely on state table overrides. By failing to support DeHackEd, we exclude thousands of iconic community WADs (like Aliens TC or Batman Doom) that define the "modded Doom" experience. Supporting DeHackEd directly expands our compatible content library by an order of magnitude.

## 📏 Metric Definition
- **Success Criteria:**
  - The engine can successfully parse standard `.deh` and `.bex` files provided via command line (e.g., `-deh patch.deh`).
  - Patches correctly override the game's internal hardcoded state tables, thing properties, and HUD strings.
  - Mods heavily reliant on state overrides load and run without crashing or undefined behavior.

## 🔍 Gap Analysis
- **Current State:** All game states (monsters, weapons) and text strings are currently statically compiled into the `doom-game` crate. There is no mechanism to mutate these tables at runtime.
- **Standard Libs / Market:** Most source ports load an internal mutable copy of the info tables on startup, then parse the DeHackEd file and overwrite specific indices before launching the main menu.

## ✅ Acceptance Criteria
- Must parse basic DEH patch syntax (Thing properties, State overrides, Weapon parameters).
- Must support BEX string replacement (e.g., changing level names or HUD messages).
- Must fail gracefully and log a warning if an unsupported extended patch format (like MBF21 or DECORATE) is encountered.
- Must decouple the currently hardcoded state tables into a runtime-mutable structure that is initialized before the first level loads.

## 🚫 Out of Scope
- Full MBF21 standard support (new codepointers, new weapon actions). This is reserved for a future phase.
- An in-game editor for creating DeHackEd patches.
