👺 Havoc: Prevent MapAnalyzer Memory Exhaustion / DFS Spin Loop

🧨 The Trigger: Constructing an adversarial map graph with hundreds of thousands of interconnected sectors causes `analyzer.chokepoints()` and `analyzer.isolated_areas()` to consume excessive CPU and memory during graph traversal.
📉 The Stack Trace: (Process killed by OOM killer or times out during unbounded DFS/BFS).
🧪 Reproduction: Run tests with a densely packed graph of > 100,000 nodes. (See `test_chokepoints_large_linear` and `havoc_test_analyzer_depth_limit`).
😈 Comment: You assumed graphs would always be small and sane, but untrusted WAD files can contain arbitrarily large graphs that trap the analyzer in an endless traversal. A hard limit per tree prevents malicious exhaustion.
