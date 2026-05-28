# 🔭 Vantage: Spec for Map Topology Safety

## Context

The engine currently parses and analyzes map topology (e.g., discovering chokepoints and isolated areas) when loading a level. This analysis relies on recursive DFS algorithms (`analyzer.chokepoints()`). However, some maps (like intentionally malicious ones or highly segmented "linear" maps) can have node topologies that are tens of thousands of nodes deep. The current recursive implementation creates a massive call stack, exceeding the default thread stack limits and causing fatal `stack overflow` aborts.

## 👤 User Story

"As a WAD Author, I want the engine to reliably parse my extremely large or deep maps without crashing, so that players can experience massive, complex geometry."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
The engine crashes on certain complex inputs due to a stack overflow. Software that crashes on user-provided data is fundamentally broken and insecure. This prevents players from loading some existing community WADs, causing immediate frustration and limiting our compatibility. By making the map topology analysis robust against deep node graphs, we restore basic stability, prevent accidental (or intentional) denial-of-service via map design, and expand the range of playable community content.

## ✅ Acceptance Criteria

### 1. Robust Graph Analysis
- **Success Metric:** The `analyzer.chokepoints()` function must successfully process a linear graph of at least 100,000 nodes without panicking or overflowing the stack.
- **Criteria:**
  - The analysis must complete within an acceptable timeframe (e.g., < 100ms for 100k nodes) without crashing.
  - The map parsing phase must handle deep tree/graph structures gracefully.

### 2. Accurate Topology Detection
- **Success Metric:** The analysis must still correctly identify chokepoints.
- **Criteria:**
  - Standard maps must continue to process correctly and identify the correct nodes as chokepoints.
  - The refactored solution (e.g., manual stack, iterative DFS, or heap allocation) must output the same exact topological results as the original recursive version.

## 🚫 Out of Scope

- **Structural Architecture Changes:** Rewriting the fundamental way `SectorGraph` or map loading is handled is not required. The goal is to harden the existing path traversal logic, not redefine the map architecture.

## ⚖️ Gap Analysis

The engine currently uses standard recursive calls in Rust, which lack tail-call optimization and quickly exhaust the thread stack on deep inputs. Other Rust crates handle deep trees either by using iterative traversals (with a heap-allocated `Vec` stack) or by explicitly increasing the thread stack size. Given we are processing arbitrary user WADs, increasing the stack size is a band-aid; the correct fix is to convert the algorithm to an iterative approach that uses heap-allocated memory, removing the hard limit on map depth.
