# 🔭 Vantage: Spec for Resilient Map Topology Analysis

## 👤 User Story
As a Player or Map Creator, I want the engine to robustly load and analyze any valid Doom map, even maliciously crafted or highly nested ones, so that the game does not inexplicably crash with stack overflows.

## ❓ So What?
Currently, the map analyzer uses a recursive DFS algorithm to find chokepoints and isolated areas. When processing a highly segmented or malicious linear segment map (e.g., 10,000+ deep nodes), this recursive approach overflows the call stack and crashes the engine. This makes the game susceptible to arbitrary crashes from custom WADs. Fixing this is critical for engine stability and user trust.

## 📏 Metric Definition
- **Success Criteria:**
  - The game can load and analyze a linear map with 10,000+ deep nodes without crashing.
  - Memory consumption during map analysis remains bounded and does not trigger OOM errors.
  - Existing map analysis features (chokepoints, isolated areas) continue to produce identical results.

## 🔍 Gap Analysis
- **Current State:** The map topology analyzer (`MapAnalyzer::chokepoints()`) relies on a recursive DFS. The stack trace clearly shows a `fatal runtime error: stack overflow` triggered by deeply nested graphs.
- **Standard Libs / Market:** Standard graph traversal on unbounded inputs in Rust must use iterative approaches (an explicit heap-allocated stack, e.g., `Vec`) instead of relying on the system call stack.

## ✅ Acceptance Criteria
- Must rewrite the map topology analysis (DFS) to use an iterative approach with a `Vec` as a stack.
- Must pass all existing tests, including tests against maps containing over 10,000 deep nodes.
- Must not introduce significant performance regressions for average-sized maps.

## 🚫 Out of Scope
- Adding new map analysis features (e.g., shortest path, flow analysis).
- Modifying other rendering or logic components unrelated to graph traversal.
