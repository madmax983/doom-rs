# 🔭 Vantage: Spec for Demo Parity

## Context

Following the parity audit and recent bug fixes, the game has excellent rendering, audio, and gameplay logic. However, the true Vanilla Doom experience requires the ability to faithfully record, playback, and interact with demo files. This pass addresses the "Demo as a Recording/Playback System" and "RNG Parity" to ensure perfectly deterministic runs.

## 👤 User Story

"As a Speedrunner or Content Creator, I want to record my gameplay perfectly and play back standard Vanilla Doom demos, so that I can analyze runs, prove completions, and share my experience with the community."

## 🎯 The "So What?" ask

**What business problem does this solve?**
Without accurate demo recording and playback, the engine is completely useless for the competitive and speedrunning communities. If a demo recorded in Chocolate Doom desyncs in our engine, or vice versa, we fail our core mandate of parity. Achieving demo parity unlocks a massive existing library of historical speedruns and provides an automated way to integration-test gameplay logic over long durations.

## ✅ Acceptance Criteria

### 1. File Format Compatibility
- **Success Metric:** The engine must read and write LMP files that are perfectly byte-for-byte compatible with Vanilla Doom and Chocolate Doom.
- **Criteria:**
  - Standard vanilla LMP headers must be parsed and generated correctly.
  - Player input (tics, turning, forward/side movement, buttons) must be correctly serialized and deserialized.

### 2. Perfect Determinism (RNG & Logic Sync)
- **Success Metric:** A 30-minute Vanilla demo file must play back from start to finish with 0% desync.
- **Criteria:**
  - The Pseudo-Random Number Generator (PRNG) must perfectly match the original Doom lookup table and index advancement logic.
  - Monster AI decisions, damage rolls, and spread calculations must consume RNG values in the exact same sequence as vanilla.
  - Timing and tic rates must not drift over time regardless of host machine performance.

### 3. Demo Recording Workflow
- **Success Metric:** Players can start the engine with a command-line flag (e.g., `-record`) and output a valid LMP file upon exit.
- **Criteria:**
  - A recording session captures all necessary initialization state.
  - The recording session successfully dumps the buffer to disk when the game is cleanly exited or the level ends.

## 🚫 Out of Scope

- **Advanced Playback Controls:** Fast-forwarding, rewinding, pause/step, or seeking to specific tics are out of scope. Parity only requires standard linear playback.
- **Multiplayer Demos:** Recording or playing back demos with more than one player is deferred to a future networking parity pass.
- **Modern Formats:** Support for MBF, Boom, or other extended demo formats; only strictly Vanilla Doom format is required.

## ⚖️ Gap Analysis

Currently, the engine has a stubbed `doom-demo` crate. While core gameplay mechanics are largely stable, they have not been stress-tested against the rigid sequence of inputs found in a demo file. The current PRNG implementation is likely not bound to the strict index-based table approach required for determinism. The command-line parsing needs to be wired up to initiate recording, and the main game loop needs the ability to substitute the TUI input poller with a file-based tic reader when playing back a demo.
