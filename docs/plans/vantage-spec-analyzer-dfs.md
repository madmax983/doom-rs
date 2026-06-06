# 🔭 Vantage: Spec for MapAnalyzer Deep DFS Handing

## 👤 User Story
As a Server Admin or Player, I want the map analyzer to securely process highly-nested map topologies, so that maliciously crafted or overly complex levels do not crash the server or client with a stack overflow.

## 🎯 So What?
The current recursive Depth-First Search (DFS) in the `MapAnalyzer::chokepoints` and `isolated_areas` methods fails on maps with deep linear topologies (e.g., >10,000 nodes) due to thread stack size limitations. This is a denial-of-service vector and prevents the engine from loading complex user-generated maps.

## 📊 Metric Definition
- **Success:** The engine loads a linear map of 10,000 deep nodes without a stack overflow panic.
- **Success:** The analysis correctly identifies chokepoints in O(V+E) time without excessive memory overhead.

## 🔍 Gap Analysis
Many graph algorithms naturally use recursion, but systems programming in Rust on fixed-size stacks requires iterative approaches for unbounded depth. The standard solution is an explicit `Vec`-based stack on the heap to track traversal state.

## ✅ Acceptance Criteria
- `MapAnalyzer::chokepoints()` must handle linear topologies up to at least 100,000 nodes without panic.
- The same requirement applies to any other recursive DFS in the analyzer (e.g., `isolated_areas`).
- Output and logic of the graph analysis must remain identical to the recursive version.

## 🚫 Out of Scope
- Rewriting the entire map loading process.
- Parallelizing the graph analysis.
