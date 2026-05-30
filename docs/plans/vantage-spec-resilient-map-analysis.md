# 🔭 Vantage: Spec for Resilient Map Analysis

## 👤 User Story
As a Player (or Modder), I want the engine to reliably load and analyze complex or maliciously crafted maps without crashing, so that my game sessions remain stable regardless of the WAD's topological complexity.

## ❓ So What?
Currently, the map analyzer uses an iterative DFS with iterator elements on its stack. While this was intended to prevent traditional call stack overflows, dropping the `Vec` of deeply nested iterators triggers a recursive `Drop` implementation in Rust. This leads to a fatal stack overflow on highly segmented maps (e.g., 10,000+ deep nodes). This is a critical stability flaw, allowing malicious or extremely complex WADs to crash the game engine. We need resilience against arbitrary graph topologies.

## 📏 Metric Definition
- **Success Criteria:**
  - The map analyzer's `chokepoints()` function can process a linear segment map with 100,000 deep nodes without a fatal stack overflow panic.
  - Existing chokepoint detection logic remains functionally correct.

## 🔍 Gap Analysis
- **Current State:** The iterative DFS implementation in `MapAnalyzer::chokepoints()` uses `(node, std::collections::hash_set::Iter)` in its manual stack. When this stack is dropped or deeply chained, it causes a recursive drop stack overflow due to Rust's memory management of nested iterators.
- **Standard Libs / Market:** Resolving this typically requires decoupling the iteration state from Rust's recursive iterator trait implementations. The standard approach is to eagerly collect neighbors into a simple vector and track the index, storing `(node, neighbors_vec, index)` instead of holding onto the iterator itself.

## ✅ Acceptance Criteria
- Must refactor the DFS stack in `MapAnalyzer::chokepoints()` to avoid deeply chained iterators.
- Must replace the `(node, iter)` tuple on the manual stack with `(node, neighbors_vec, index)` or an equivalent non-recursive structure.
- Must add a regression test ensuring maps with 100,000+ nodes do not panic during analysis.

## 🚫 Out of Scope
- Optimizing the runtime performance of `chokepoints()` beyond what is required for stability.
- Altering the core mathematical logic of the articulation point detection algorithm itself.
