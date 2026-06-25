🔭 Vantage: Spec for Resilient Map Topology Analysis

👤 **User Story:**
As a Player or Map Creator, I want the engine to robustly load and analyze any valid Doom map, even maliciously crafted or highly nested ones, so that the game does not inexplicably crash with stack overflows.

✅ **Acceptance Criteria:**
- Must rewrite the map topology analysis (DFS) to use an iterative approach with a `Vec` as a stack.
- Must pass all existing tests, including tests against maps containing over 10,000 deep nodes.
- Must not introduce significant performance regressions for average-sized maps.

🚫 **Out of Scope:**
- Adding new map analysis features (e.g., shortest path, flow analysis).
- Modifying other rendering or logic components unrelated to graph traversal.
