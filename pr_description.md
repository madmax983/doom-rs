🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."

👤 **User Story:** As a mapper or player loading complex WADs, I want the map analyzer to safely process deeply nested topological structures without crashing, so that the game engine can handle large and unconventional maps reliably.

✅ **Acceptance Criteria:**
- The engine must successfully process maps with graph depths of at least 10,000 nodes without triggering a stack overflow.
- The map analysis must complete without panic on malicious or highly segmented input maps.

🚫 **Out of Scope:**
- Performance optimization beyond avoiding the stack overflow.
- Refactoring the entire map loading pipeline; changes should be limited to the map analysis algorithms.
