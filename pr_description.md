🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."

🔭 Vantage: Spec for Resilient Map Topology Analysis

👤 **User Story:** As a Server Admin or Player, I want the engine to safely process extremely complex or malicious map topologies, so that the game does not crash with a stack overflow during map loading or gameplay.

**The "So What?" ask:** Crashing on map load is a critical failure that ruins the user experience and introduces a Denial of Service (DoS) vulnerability in multiplayer environments. Robustness against extreme inputs is a core requirement for a stable engine.

**Metric Definition:** Success = Engine successfully analyzes a map with >10,000 deep nodes without stack overflow, and map analysis completes in <50ms.

**Gap Analysis:** The current recursive DFS implementation uses the call stack, which limits depth and creates a critical crash vulnerability. Production graph traversal systems utilize heap-allocated iterative approaches to handle unbounded depth safely.

✅ **Acceptance Criteria:**
- Must handle map topologies of arbitrary depth (e.g., >10,000 nodes) without a stack overflow or panic.
- Must maintain analysis performance, completing in under 50ms for dense maps.

🚫 **Out of Scope:** Multi-threaded map analysis or optimizing map load times beyond fixing the crash.
