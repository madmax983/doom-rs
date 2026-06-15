# 🔭 Vantage: Spec for Map Analyzer Resilience

## Context

The engine currently crashes with a stack overflow when parsing complex user-created maps (PWADs). Specifically, the map topology analyzer, which calculates chokepoints and isolated areas, fails on maps with deep, linear node structures (like long winding corridors or highly segmented sectors).

This spec outlines the requirement for making the map analyzer resilient against malicious or complex map geometries.

## 👤 User Story

"As a Player, I want to load and play highly complex custom maps without the game crashing, so that I can enjoy community-created content."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Crashing on user input is a critical failure. If the engine cannot handle standard community maps because of arbitrary depth limits in our graph traversal algorithms, we lose the ability to support the vast ecosystem of PWADs. This limits the usefulness of the engine.

## ✅ Acceptance Criteria

### 1. No Stack Overflows on Deep Graphs
- **Success Metric:** The map analyzer must not crash when analyzing a deep linear map containing 10,000 sequentially connected nodes.
- **Criteria:**
  - The graph traversal algorithms must be capable of processing arbitrarily deep graphs.

### 2. Bounded Resource Usage
- **Success Metric:** The map analyzer must prevent unbounded memory consumption by enforcing a limit on the number of nodes it processes.
- **Criteria:**
  - The analyzer must return an error (and halt traversal) rather than panicking or freezing if the node limit is exceeded.
  - Do not silently return incorrect partial results; the error must be explicit.

## 🚫 Out of Scope
- Modifying the actual `chokepoints` and `isolated_areas` logic. The focus is purely on the resilience and resource bounding of the traversal mechanism.

## ⚖️ Gap Analysis
Currently, the `chokepoints` method in `crates/doom-map/src/analyzer.rs` uses an iterative DFS to prevent stack overflows, but `isolated_areas` may still be vulnerable if it uses recursion or unconstrained queues. Additionally, neither method currently implements a strict node limit to prevent memory exhaustion, as noted in the memory rules.
