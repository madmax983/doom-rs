# 🔭 Vantage: Spec for Chokepoint Analyzer

## 👤 User Story
As a Level Designer, I want to identify chokepoints in my maps, so that I can better understand map flow and optimize monster placement.

## ❓ So What?
The current recursive DFS implementation for finding chokepoints (`analyzer.chokepoints()`) causes a stack overflow on highly nested topologies. This completely crashes the application when loading large or malicious maps. Fixing this allows the engine to analyze complex maps reliably without crashing, improving overall stability.

## 📏 Metric Definition
- **Success Criteria:**
  - `MapAnalyzer::chokepoints()` correctly identifies all articulation points in the map graph.
  - The function successfully processes extremely deep/nested graphs (e.g., linear segments with over 10,000 nodes) without triggering a stack overflow.

## 🔍 Gap Analysis
- **Current State:** The `chokepoints()` method uses a recursive Depth-First Search algorithm that is susceptible to stack overflows.
- **Market:** Graph algorithms that handle large datasets must either use bounded recursion or iterative approaches using an explicit heap-allocated stack.

## ✅ Acceptance Criteria
- Must rewrite `MapAnalyzer::chokepoints()` to use an iterative DFS instead of recursive DFS.
- Must preserve the exact same articulation point detection logic and return values.
- Must pass the existing `test_chokepoints_large_linear` test which explicitly checks for stack overflow handling.

## 🚫 Out of Scope
- Rewriting the entire graph traversal system.
- Adding visual UI representations of the chokepoints.
