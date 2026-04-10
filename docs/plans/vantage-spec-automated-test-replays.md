# 🔭 Vantage: Spec for Automated Test Replays

## 👤 User Story
As a Developer, I want to record precise, frame-perfect gameplay sessions and use them as automated integration tests, so that I can ensure complex game logic and physics changes do not introduce subtle regressions.

## ❓ So What?
Currently, our test suite heavily relies on unit tests and small, synthetic level constructs. While this is great for testing pure functions, it fails to capture the emergent behavior of the engine (e.g., monster infighting, complex line-of-sight interactions, and physics edge cases). If a change subtly breaks monster pathing, our unit tests might pass, but the game is broken. Using demo files as automated replay tests provides a high-fidelity safety net that guarantees the engine behaves exactly as it did when the demo was recorded. Complexity is a cost; regressions are a bigger cost.

## 📏 Metric Definition
- **Success Criteria:**
  - A test runner can execute a demo file in "headless" mode.
  - The test verifies that the final state of the game (e.g., player health, monster kills, final sector positions) perfectly matches a known-good checksum or snapshot.
  - The test fails immediately if the demo desyncs (e.g., the player dies when they shouldn't, or a monster survives).

## 🔍 Gap Analysis
- **Current State:** We have a robust playback system capable of recording and playing back input streams, and a CLI argument to trigger it. However, there is no harness to use these demos as assertions in our test runner.
- **Standard Libs / Market:** Classic source ports use demo sync as the gold standard for engine parity. If a demo from 1993 plays back correctly, the physics engine is accurate. We need to bring this standard into our testing pipeline.

## ✅ Acceptance Criteria
- Must introduce a test harness that runs standard demos.
- Must execute the game loop without invoking audio or visual rendering backends to ensure tests run fast and reliably in CI.
- Must compare the final game state hash or specific metrics (kills, items, secrets, player health) against expected values.
- Must provide a simple way to add new test demos to the repository (e.g., placing a demo and an expectation file in a fixtures directory).

## 🚫 Out of Scope
- Visual frame-buffer regression testing (image diffing). The focus is purely on the logical game state and physics determinism.
- Multiplayer (netplay) demo recording and playback.
