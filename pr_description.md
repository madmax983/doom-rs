🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."

👤 **User Story:**
As a Player running custom maps (WADs), I want the engine to reliably process highly complex, deeply nested, or even malicious map topologies without crashing, so that my gameplay experience remains stable and uninterrupted.

✅ **Acceptance Criteria:**
- The engine must successfully parse and analyze maps with arbitrary depth and complexity without triggering a stack overflow.
- Maliciously constructed maps designed to exceed standard nesting limits must either be processed safely or rejected gracefully with a user-friendly error message, rather than crashing the engine.
- Map analysis performance must not significantly degrade compared to the current implementation. Success is defined as map load times remaining within 10% of their current baseline for standard maps.

🚫 **Out of Scope:**
- Performance optimization of the general rendering pipeline.
- Modifying the core gameplay simulation loop.
- Support for fundamentally new map data formats not currently handled.
