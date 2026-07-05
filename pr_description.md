🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."

# 🔭 Vantage: Spec for Resilient Map Analysis

## Description

* 👤 **User Story:** "As a Player, I want to load arbitrarily complex community maps without crashing, so that my game experience is stable regardless of map geometry."
* **The "So What?" ask:** What business problem does this solve? The map analyzer uses a recursive DFS algorithm that crashes the engine via stack overflow on deeply nested maps. Stability is a baseline requirement; crashing on community content is an unacceptable liability for our platform.
* **Metric Definition:**
  - Success = The map analyzer processes a 10,000-depth linear map without a stack overflow.
* **Gap Analysis:**
  - Current implementation uses the thread call stack (recursive DFS), hitting hard memory limits on large graphs.
  - Standard Libs / Market: Deep graph traversal in Rust typically uses explicit, heap-allocated stacks (like `Vec`) to ensure unbounded, safe processing.
* ✅ **Acceptance Criteria:**
  - Must implement an iterative traversal for `chokepoints()` using a heap-allocated `Vec`.
  - Must correctly identify articulation points and maintain accuracy matching the original implementation.
  - Must pass the `test_chokepoints` integration test without panicking.
* 🚫 **Out of Scope:**
  - Multithreading the graph analysis (Phase 2).
  - Calculating new tactical metrics beyond chokepoints and isolated areas.
