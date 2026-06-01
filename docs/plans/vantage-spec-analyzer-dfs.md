# 🔭 Vantage: Spec for Map Analyzer Engine Resilience

## 👤 User Story
As a Player or Map Creator, I want the map analyzer to gracefully handle highly nested and complex map topologies, so that the game does not crash when I load large or maliciously crafted custom WADs.

## ❓ So What?
The current map analyzer uses a recursive search for finding chokepoints. This recursive approach causes a stack overflow on linear or highly nested maps, crashing the entire game. This makes the engine brittle against user-generated content and prevents players from enjoying large-scale community maps. Resolving this transforms a fatal flaw into robust engine resilience.

## 📏 Metric Definition
- Success = The map analyzer successfully processes a linear segment map with 100,000+ deep nodes without triggering a stack overflow.
- Success = Performance overhead of the new approach is negligible compared to the old approach.

## 🔍 Gap Analysis
- The current implementation relies on the system call stack for recursion, which has a fixed, relatively small size limit.
- We need to shift to an approach that does not rely on the call stack for deep topology traversal.

## ✅ Acceptance Criteria
- Must replace the recursive algorithm in the map analyzer with a non-recursive approach.
- Must maintain the exact same functionality and outputs (identifying all correct chokepoints).
- Must add a test case that specifically verifies resilience against maps with 100,000+ deep nodes.

## 🚫 Out of Scope
- Optimizing or changing the pathfinding AI for monsters.
- Modifying how the BSP map is loaded; this is strictly isolated to the analytical processing layer.
