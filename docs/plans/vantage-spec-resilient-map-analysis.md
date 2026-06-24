# 🔭 Vantage: Spec for Resilient Map Topology Analysis

## Context

The map analyzer currently processes level topologies (like finding chokepoints) using algorithms that may rely on the call stack. On highly segmented or maliciously crafted WADs with deep nodes, this can cause the engine to crash via stack overflow.

## 👤 User Story

As a Player or Server Administrator, I want the engine to safely load and analyze complex or malicious custom maps without crashing, so that my gameplay experience or multiplayer server remains stable regardless of the provided WAD file.

## 🎯 The "So What?" Ask

**What business problem does this solve?**
An engine that crashes on specific user-generated content is a liability. By ensuring map analysis scales independently of call stack limits, we guarantee uptime for multiplayer servers and prevent frustrating crashes for single-player users exploring custom WADs. Resilience translates directly to user trust.

## 📏 Metric Definition

- **Success Criteria:** The engine successfully analyzes map topologies with depth > 10,000 without crashing or stack overflowing.

## 🔍 Gap Analysis

- **Current State:** The map analyzer has choke points that can trigger a stack overflow if map depth is excessive.
- **Target State:** Graph traversal must use heap-allocated data structures (like an explicit stack vector) rather than the call stack to manage traversal state.

## ✅ Acceptance Criteria

- Map analysis must successfully process maps with 10,000+ deep nodes.
- Graph traversal algorithms must utilize the heap (e.g., `Vec`) instead of the call stack.
- Engine must not crash or panic when analyzing complex topology configurations.

## 🚫 Out of Scope

- Optimizing the algorithmic time complexity of the map analyzer (we are fixing stability, not speed).
- Modifying the core WAD loading or node-building logic.
