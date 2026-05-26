# 🔭 Vantage: Spec for Resilient Topology Analysis

## 👤 User Story
As a Modder or Player, I want the engine to safely handle extremely large or heavily segmented maps (e.g., slaughter maps or procedurally generated topologies) so that I can play complex content without the game crashing unexpectedly.

## So What? (Business Problem)
Currently, untrusted or complex user-generated content (like maps with highly nested topologies) can cause a stack overflow during engine analysis. This translates to an immediate crash. Game engines are expected to be robust platforms for modders. Crashing on map load creates a poor user experience, frustrates content creators, and limits the community's ability to push the engine's boundaries.

## Metric Definition
- **Success:** The engine must parse and analyze a test map with >10,000 sequentially linked nodes without crashing or exceeding available system memory.
- **Latency:** Topology analysis time for standard commercial maps (e.g., original Doom episodes) should not regress by more than 5%.

## Gap Analysis
- **Current State:** The map analyzer (`analyzer.chokepoints()`) utilizes a recursive Depth-First Search (DFS) for topology traversal. This couples the execution stack depth directly to the map's graph depth, scaling linearly until memory bounds are hit.
- **Standard Practice:** Graph traversals in systems facing unbounded input should utilize iterative algorithms (maintaining an explicit stack/queue on the heap) rather than relying on the call stack.

## ✅ Acceptance Criteria
1. Must handle deeply nested or linear node segments (e.g., depth > 10,000) without triggering a stack overflow panic.
2. Must impose reasonable bounds on processing time to prevent soft-locks on intentionally malicious, infinitely cyclic, or overly massive maps.
3. Must maintain existing correct analysis output (e.g., correctly identifying chokepoints and sectors).

## 🚫 Out of Scope
- Changing the underlying WAD file structure or map format.
- Adding new topology analysis features (we are only stabilizing the existing feature).
