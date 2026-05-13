# 🔭 Vantage: Spec for Resilient Map Processing

## Context

The map analyzer is responsible for understanding the topology of game levels by finding chokepoints and isolated areas. Currently, processing extremely complex, highly-nested, or maliciously crafted custom maps causes the engine to fail entirely.

This spec defines the "What" and the "Why" for introducing a resilient map processing subsystem that guarantees the engine will not crash when analyzing arbitrary user-generated content.

## 👤 User Story

"As a Server Administrator, I want the map analyzer to safely process any custom map, no matter how complex or poorly constructed, so that malicious maps cannot crash the multiplayer server."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
The engine currently suffers from catastrophic failures (crashes) when analyzing maps with extreme geometric complexity (e.g., linear map segments exceeding 10,000 nodes). This represents a critical stability risk, especially for multiplayer servers that allow users to upload custom WADs. A single malicious file can take down a server. Fixing this guarantees engine resilience, ensures uninterrupted gameplay, and removes a major vector for denial-of-service, thereby increasing player trust and server uptime.

## 📏 Metric Definition
- **Success Criteria:**
  - The map analyzer successfully completes processing on maps with 10,000+ deep nodes without crashing.
  - Overall map load time for standard maps (e.g., standard IWAD levels) must not degrade by more than 5%.

## ✅ Acceptance Criteria

### 1. Guaranteed Stability Under Extreme Loads
- **Success Metric:** The engine handles arbitrarily deep map topologies without failure.
- **Criteria:**
  - The engine must gracefully traverse highly nested sector graphs (e.g., 10,000+ nodes) without halting or crashing.
  - Memory consumption during analysis must remain bounded.

### 2. Consistent Analysis Output
- **Success Metric:** The results of the map analysis remain correct and consistent with previous, smaller-scale map tests.
- **Criteria:**
  - Standard maps (e.g., from the original IWADs) yield identical chokepoint and isolated area data.

## 🚫 Out of Scope

- **Refactoring the Entire Map Pipeline:** We are not rewriting the fundamental BSP traversal or WAD loading routines in Phase 1; changes must be localized to the graph analysis and topology traversal algorithms.
- **Advanced Map Healing:** Automatically fixing broken map geometry (e.g., unclosed sectors) is out of scope. The analyzer just needs to not crash.

## ⚖️ Gap Analysis

The engine currently analyzes map topology using a recursive Depth-First Search (DFS) algorithm (`analyzer.chokepoints()`). The call stack depth scales linearly with the depth of the map graph. When faced with maps containing 10,000+ sequential nodes, the recursion depth exceeds the maximum call stack size, leading to a stack overflow and a fatal crash. To fix this, the engine must transition from a recursive traversal strategy to an iterative one, utilizing the heap for state tracking rather than the call stack. This shift is required to handle the scale of arbitrary user-generated content safely.
