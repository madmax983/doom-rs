# 🔭 Vantage: Spec for Map Topology Sandboxing

## Context

Recent testing has highlighted a critical vulnerability in the map topology analyzer (specifically, `analyzer.chokepoints()`). Highly nested or segmented maps can trigger a deep recursive Depth-First Search (DFS), leading to stack overflows and a hard crash of the engine. While the immediate algorithm is an engineering concern, the broader product issue is that a single malformed or malicious map can bring down the entire application.

This spec defines the "What" and the "Why" for introducing Map Topology Sandboxing to ensure the engine fails gracefully rather than crashing.

## 👤 User Story

"As a Player, I want the engine to safely handle extremely complex or poorly designed maps without crashing, so that I don't lose my game session or progress when encountering malicious or edge-case WAD files."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Crashes severely damage player trust and the engine's reputation for stability. By allowing a simple loaded map to cause a stack overflow, we have a vulnerability that can be exploited intentionally (malicious WADs) or encountered accidentally (poorly optimized custom maps). This feature transforms a fatal runtime error into a handled exception, improving overall resilience and ensuring the engine remains a reliable tool for playing both classic and highly experimental Doom content.

## 📏 Metric Definition

- **Success Metric:** 100% of maps that previously triggered a stack overflow during analysis (e.g., linear segment map containing over 10,000 deep nodes) must now either be processed successfully or result in a graceful error dialog/log message without terminating the main application process.

## 🔍 Gap Analysis

Currently, map parsing and analysis are deeply integrated into the loading sequence. If the analysis phase (like finding chokepoints) panics or overflows the stack, it brings down the main thread. Modern robust applications separate potentially unbounded or recursive parsing from the critical execution thread or replace recursive algorithms with iterative ones. The current gap is a lack of bounding on recursive depth and a lack of isolation for map analysis operations.

## ✅ Acceptance Criteria

### 1. Robust Algorithmic Limits
- **Criteria:**
  - Graph traversal algorithms used in map analysis must not rely on unbounded recursion.
  - If a map exceeds a reasonable complexity threshold (e.g., node depth limits), the engine must detect this and gracefully abort the specific analysis rather than attempting to process it indefinitely or overflowing the stack.

### 2. Graceful Failure State
- **Criteria:**
  - When map analysis fails (either due to hitting a complexity limit or an internal error), the game must not crash.
  - The engine must log the specific failure reason.
  - The map should either load without the advanced tactical analysis features, or present a clear error to the user and return to the main menu/launcher state.

## 🚫 Out of Scope

- **Fixing the Specific DFS Bug:** The engineering team will handle rewriting the DFS to be iterative or increasing the stack size. This spec strictly covers the requirement that *no map analysis* should be capable of crashing the engine.
- **Validating all WAD lumps:** This spec focuses specifically on map topology (nodes, sectors, linedefs) analysis, not general resource validation (like textures or sounds).
