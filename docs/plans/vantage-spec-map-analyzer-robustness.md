# Map Analyzer Robustness

## 👤 User Story
As a Map Designer and Engine Developer, I want the map analyzer to safely handle extremely complex, deeply nested maps without crashing, so that the game engine remains stable when loading user-generated content.

## 💼 The "So What?" Ask
Prevents total application crashes (stack overflows) on malicious or highly segmented input maps, ensuring the engine can reliably analyze any valid WAD.

## 📏 Metric Definition
Success = `cargo test --package doom-map` passes on a linear segment map containing over 10,000 deep nodes without stack overflow.

## 🔍 Gap Analysis
The current graph traversal algorithm in `analyzer.chokepoints()` is vulnerable to stack overflows on deep graphs. Standard graph traversal in robust systems uses iterative approaches to bound memory usage to the heap.

## ✅ Acceptance Criteria
- Must handle deeply nested maps (10,000+ nodes) without panicking.
- Must correctly identify chokepoints in standard maps.

## 🚫 Out of Scope
- Optimizing the performance of the analyzer beyond crash prevention.
