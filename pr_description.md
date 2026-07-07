🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."

👺 Havoc: Fix out-of-bounds blockmap query yielding garbage data
🧨 **The Trigger:** Out-of-bounds blockmap queries caused `block_linedefs` to fallback to offset 0, reading the header fields as linedef indices.

📉 **The Stack Trace:**
```
thread 'main' panicked at 'out of bounds linedef access'
```

🔬 **Reproduction:** "Run `cargo test -p doom-map --lib lumps` to see `havoc_blockmap_oob`."

😈 **Comment:** "You assumed `unwrap_or(0)` was harmless. You were wrong. The parser happily swallowed the header x-origin as a linedef index."
