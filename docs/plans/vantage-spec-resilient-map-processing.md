# 🔭 Vantage: Spec for Resilient Map Processing

## Context

The engine must parse and analyze community-created maps, which can vary wildly in complexity and size. Currently, map processing logic uses recursive algorithms that scale directly with map depth. This means highly segmented or malicious "slaughter maps" with massive linear segments or deep node graphs can easily trigger stack overflow crashes. We must build robust parsing systems that do not rely on the runtime call stack to process input data.

## 👤 User Story

"As a Player, I want to load and play massive, highly complex custom maps without the engine crashing, so that I can experience the limits of community creativity without fear of arbitrary technical boundaries."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Doom's longevity is tied entirely to its modding and mapping community. If our engine crashes on modern, complex maps (which often push the boundaries of sector counts and node depths), we lose relevance to the very community we are building for. Crashing on user input is an unacceptable failure mode that damages player trust and limits the engine's utility.

## 📏 Metric Definition

- **Success:** The engine successfully parses and runs a test map containing 100,000 deep interconnected nodes without exceeding standard stack limits or triggering a crash.

## ✅ Acceptance Criteria

### 1. Robust Graph Analysis
- **Success Metric:** Map analysis tools must handle infinite or maximum-scale data gracefully without crashing.
- **Criteria:**
  - Graph traversal functions must not use recursive function calls that scale with map complexity.
  - Algorithms must use heap-allocated data structures (like explicit Stacks or Queues) for processing loops instead of the call stack.

### 2. Graceful Error Handling
- **Success Metric:** When processing limits are legitimately hit, the engine must safely exit to the menu with an error, not crash the process.
- **Criteria:**
  - Map loading functions must gracefully report errors back to the user interface instead of aborting or panicking when maps exceed theoretical engine limits (e.g., node limits).

## 🚫 Out of Scope

- **Runtime performance optimization:** While iterative processing shouldn't be terribly slow, making it run at 1000 FPS is not the goal here. The goal is strictly *not crashing*.
- **Map format extensions:** We are not expanding what a map *is* (e.g., adding UDMF features), only ensuring we safely process what exists.

## ⚖️ Gap Analysis

The engine currently uses recursive DFS (Depth-First Search) in systems like `analyzer.chokepoints()`. The community has already proven that call stacks do not scale linearly with WAD sizes, triggering stack overflows. We need to refactor these critical chokepoints to use iterative logic with heap-allocated state tracking.
