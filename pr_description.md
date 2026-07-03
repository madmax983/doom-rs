🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."

📖 Chapter: The `sprite_clip` module in `doom-renderer`.
💡 Insight: Explained the rationale for `SpriteClipHistory` acting as an allocation-free fixed buffer to ensure zero heap allocations in the renderer's hot path.
🧪 Example: Added executable `## Examples` doc-tests for `new`, `push`, `last`, and `iter`.
🖼️ Preview: Resolved all `-D missing_docs` lints across the workspace.
