# 🔭 Vantage: Spec for Headless Timedemo Benchmarking

## Context

Following the implementation of `DemoPlaybackApp` and the core `doom-demo` functionality, the engine has the ability to decouple the internal game tick from the live poll rate of the terminal. However, we currently lack the ability to run these demos as performance benchmarks.

This spec focuses on the "What" and the "Why" for introducing a `-timedemo` equivalent, similar to the original vanilla Doom functionality, but extended for our terminal-based architecture.

## 👤 User Story

"As an Engine Developer or Speedrunner, I want to play back a demo file as fast as my CPU allows without being constrained by the terminal's 35Hz frame pacing or vsync, so that I can benchmark the engine's performance or rapidly verify that a demo doesn't desync."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Performance regressions are difficult to catch without a reproducible workload. Standard `cargo bench` provides micro-benchmarks, but we need a macro-benchmark that exercises the entire engine (BSP traversal, AI logic, software rendering). A timedemo feature allows us to run a complex, known workload (like `DEMO1`) in a few seconds, outputting a measurable metric (frames per second, total time). This is also crucial for automated integration testing to ensure logic remains 100% deterministic over long runs without waiting 30 minutes for a real-time playback.

## ✅ Acceptance Criteria

### 1. The Command Line Interface
- **Success Metric:** The user can invoke `doom-app` with a specific command-line flag (e.g. `--timedemo <FILE>`) to initiate the benchmark.
- **Criteria:**
  - The feature must be mutually exclusive with live play (`--server`, `--connect`, or normal startup).
  - The feature should bypass the standard 35Hz terminal event loop.

### 2. Maximum Throughput Execution
- **Success Metric:** The engine processes every tic in the demo file as quickly as possible.
- **Criteria:**
  - The game logic must not wait for wall-clock time between tics.
  - The software renderer must still generate the frame buffer (to accurately benchmark rasterization performance), but it must **not** attempt to render to the actual terminal output, bypassing `ratatui` or `crossterm` overhead.

### 3. Benchmark Reporting
- **Success Metric:** Upon demo completion, the application cleanly exits and outputs performance statistics to standard out.
- **Criteria:**
  - The output must include the total number of tics simulated.
  - The output must include the total real-world elapsed time.
  - The output must include the calculated framerate (Tics per second or FPS).

## 🚫 Out of Scope

- **Real-time visual rendering:** Drawing the frames to the terminal during a timedemo is out of scope. The point is headless benchmarking; terminal I/O becomes the bottleneck.
- **Partial execution:** Seeking or running only a slice of the demo. The timedemo must run the entire file.
- **Demo format extensions:** We will strictly use the existing vanilla LMP parser.

## ⚖️ Gap Analysis

The engine currently has the ability to decouple the internal game tick from live player input (for demo playback), but the main execution loop is still strictly tied to the terminal's refresh rate and rendering pipeline. To implement this spec, we need a new execution path that loads the demo data and runs the simulation loop continuously at maximum speed, completely bypassing the terminal polling and drawing phases.
