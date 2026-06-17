# 🔭 Vantage: Spec for Map Analyzer Resilience

## 👤 User Story
"As a Mapper, I want the map analysis tools to handle extremely deep or complex topologies gracefully, so that the engine doesn't crash via stack overflow when analyzing my custom map."

## 🎯 The "So What?" Ask
**What business problem does this solve?**
The current `MapAnalyzer::chokepoints` function uses a recursive Depth-First Search (DFS) or deeply nested iterative approach that can trigger a stack overflow on linear or highly segmented maps (e.g., 10,000 deep nodes). This causes a fatal runtime error. By making the traversal algorithm resilient to arbitrary depth, we improve engine stability and ensure players and mappers don't experience hard crashes on edge-case content, thereby increasing the reliability and robustness of our mapping tools.

## ✅ Acceptance Criteria
- **Success Metric:** The engine can process a linearly connected graph of 10,000 nodes without triggering a stack overflow or panicking.
- **Criteria:**
  - The `MapAnalyzer::chokepoints` algorithm must be refactored to use an iterative approach with an explicit heap-allocated stack (e.g., `Vec`), eliminating deep function call recursion or unbounded state tracking.
  - A test case must be added or maintained (e.g., `test_chokepoints_large_linear`) that explicitly constructs a 10,000-node linear graph and verifies it completes successfully.

## 🚫 Out of Scope
- **Performance Optimization:** We are not rewriting the entire analyzer for speed in Phase 1, only for stability. The focus is on preventing stack overflows, not necessarily making the chokepoint detection asymptotically faster.
- **Memory Limits:** While we avoid stack overflow, we are not implementing hard memory limits or failing gracefully on out-of-memory (OOM) for the heap in Phase 1.

## ⚖️ Gap Analysis
The current implementation relies on an iterative approach that may still be creating deep states or not properly avoiding stack exhaustion. We need to analyze `analyzer.rs` to ensure the DFS traversal is entirely iterative, pushing state to a heap-allocated `Vec` rather than relying on the call stack, and that it passes the large linear graph test without issue.
