# 🔭 Vantage: Spec for Safe Map Analysis

## 👤 User Story
As a Server Administrator hosting custom WADs, I want the map analyzer to safely handle maliciously crafted or extremely deep map topologies, so that the engine does not crash during map loading or analysis.

## ❓ So What?
If the engine crashes on complex maps, users lose trust in the software and multiplayer servers become vulnerable to denial-of-service via malicious maps. Ensuring stability turns the engine from a toy into a robust platform.

## 📊 Metrics
- **Success =** 0 crashes on maps with 10,000+ sequential sectors. Analysis completes within reasonable time limits.

## 🕳️ Gap Analysis
Currently, analyzing deep topologies exhausts the runtime call stack due to recursion, causing fatal runtime errors. Competing engines handle deep graphs iteratively to bypass stack limits.

## ✅ Acceptance Criteria
- The map analysis phase must not panic or cause a stack overflow on highly nested topologies.
- Must accurately identify map chokepoints without relying on unbounded call stack growth.
- Must include automated tests that process 10,000+ deep nodes successfully.

## 🚫 Out of Scope
- General memory optimizations for rendering.
- Rewriting the WAD file parser.
- Fixing memory leaks unrelated to map topology analysis.