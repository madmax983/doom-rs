# 🔭 Vantage: Spec for Resilient Map Processing

## 👤 User Story
As a Server Host or Player, I want the game to safely load and analyze complex or maliciously crafted maps without crashing, so that the server and game client remain stable and resilient against denial-of-service vectors.

## ❓ So What?
Currently, processing map geometries with highly nested or extremely deep sector topologies causes a stack overflow during the `chokepoints()` analysis, effectively crashing the program. Since custom WADs are user-generated content, this represents a significant stability and security risk. A robust engine must gracefully handle untrusted or extreme inputs.

## 📏 Metric Definition
- **Success Criteria:**
  - Processing a linear segment map with 10,000+ deep nodes completes successfully without stack overflow.
  - Map loading time remains reasonable (e.g., under 500ms) even for deeply nested topologies.

## 🔍 Gap Analysis
- **Current State:** The map analyzer relies on recursive algorithms (DFS) that scale with the map's topological depth, exceeding the call stack limit on extreme inputs.
- **Standard Libs / Market:** Modern parsers and engines use iterative traversal algorithms (using heap-allocated stacks) or enforce strict depth limits to process untrusted tree/graph structures safely.

## ✅ Acceptance Criteria
- Must refactor map analysis (such as the `chokepoints` DFS) to use iterative algorithms with heap-allocated data structures, avoiding call stack recursion.
- Must successfully process maps with arbitrarily deep topologies (up to system memory limits) without stack overflows.
- Must fail gracefully (return a `Result::Err`) instead of panicking if absolute memory or node limits are exceeded.

## 🚫 Out of Scope
- Fixing rendering or rendering-specific BSP tree depth issues.
- Handling malformed WAD binary structures (this is specific to the topological sector graph analysis).
