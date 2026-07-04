🧨 **The Trigger:** `analyzer.chokepoints()` recursive DFS causes a stack overflow on highly nested topologies, effectively crashing the program on malicious or highly segmented input maps.

📉 **The Stack Trace:**
```
thread 'main' (42123) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

🧪 **Reproduction:** "Run `cargo test --package doom-map` with a linear segment map containing over 10,000 deep nodes."

😈 **Comment:** "You assumed call stacks scale linearly with your WADs. You were wrong."

📖 Chapter: The `AiDirector` and `sprite_clip` modules.
🔦 Insight: Explained the dynamic difficulty adjustment logic and added high-performance clipping buffer documentation.
🧪 Example: Added 2 executable doctests demonstrating Director pacing and clipping history allocation.
🖼️ Preview: Documentation looks visually structured and renders perfectly in rustdoc.
