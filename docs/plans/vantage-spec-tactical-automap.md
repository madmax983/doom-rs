# 🔭 Vantage: Spec for Tactical Automap Overlay

## 👤 User Story
As a Player or Map Designer, I want to see a tactical overlay on the Automap that highlights chokepoints and isolated areas, so that I can better understand map flow, plan movement routes, and appreciate the underlying graph topology of complex levels.

## ❓ So What?
The map graph analyzer module (`doom-map::analyzer::MapAnalyzer`) was recently fixed to prevent stack overflows and is successfully calculating valuable topological data (chokepoints/articulation points and isolated areas) for every loaded map. However, this data is currently invisible to the user. Surfacing this "hidden" backend data creates a new, high-utility feature for players who want a tactical advantage and for modders debugging level connectivity, effectively monetizing (in utility terms) the engineering cost already spent on the analyzer.

## 📏 Metric Definition
- **Success Criteria:**
  - A new toggleable mode exists in the Automap (e.g., "Tactical View").
  - When enabled, sectors/lines identified as chokepoints by `MapAnalyzer::chokepoints()` are rendered in a distinct highlight color (e.g., Red or Bright Orange).
  - Isolated areas identified by `MapAnalyzer::isolated_areas()` are shaded or outlined distinctly to show disconnects.
  - Toggling the view does not impact gameplay framerate (the analysis should be cached upon level load, not recalculated per frame).

## 🔍 Gap Analysis
- **Current State:** The Automap simply renders standard lines (walls, ceilings, floors). The `MapAnalyzer` computes chokepoints, but the UI ignores the output.
- **Standard Libs / Market:** Modern strategy games often provide tactical map overlays (e.g., XCOM, Civilization). In the context of Doom, adding strategic overlays pushes the engine beyond a simple port and into a tool that offers deeper insight into level design.

## ✅ Acceptance Criteria
- Must introduce a new `MapOverlayMode` enum (e.g., `Standard`, `Tactical`) controlled by a keybind when the Automap is active.
- Must cache the `MapAnalyzer` results in the `Level` or `GameState` structure during initial map load.
- Must update the renderer (`doom-renderer` or `doom-tui` automap drawing logic) to conditionally apply alternate colors/styles to lines belonging to chokepoint sectors.
- Must provide a legend or on-screen text indicating when Tactical Mode is active.

## 🚫 Out of Scope
- Real-time recalculation of topology (e.g., if a door closes, we don't recalculate the entire graph).
- Enemy pathfinding overlays (focus only on static map topology for now).
