# 🔭 Vantage: Spec for Vanilla Limits & Overflow Emulation

## 👤 User Story
As a purist player or map creator, I want the engine to optionally enforce original static limits (e.g., visplanes, drawsegs) and reproduce their exact overflow behaviors (e.g., visplane crash, all-ghosts bug), so that I can experience WADs exactly as they were on DOS and validate vanilla compatibility.

## 💼 The "So What?" (Business Problem)
Currently, `doom-rs` is effectively a limit-removing port. While this is great for modern complex maps, it alienates the large segment of the Doom community that relies on Chocolate Doom for exact historical preservation and testing. By implementing vanilla limits, we unlock the "purist" user segment and make `doom-rs` a viable tool for vanilla map authors, directly competing with Chocolate Doom's core value proposition.

## 📊 Success Metrics
- **Fidelity Rate:** 100% replication of known overflow crashes on a curated suite of vanilla limit-breaking test maps (e.g., visplane overflow maps).
- **Zero Regression:** 0% impact on performance or behavior when the compatibility toggle is disabled.

## 🔍 Gap Analysis
- **Current State:** `doom-rs` uses dynamic buffers and safe bounds, making it limit-removing. It does not crash or exhibit vanilla visual artifacts on limit breaches.
- **Market Comparison (Chocolate Doom):** Chocolate Doom rigidly enforces static limits and explicitly reproduces historical bugs.
- **The Gap:** We need a toggleable `vanilla-compat` mode that swaps the safe dynamic structures for vanilla-sized arrays and intentionally invokes historical failure states.

## ✅ Acceptance Criteria
- Must introduce an opt-in `--vanilla-compat` mode.
- Default behavior remains limit-removing.
- When enabled, the following limits must be enforced with matching behaviors:
  - `MAXVISPLANES` (128): "No more visplanes" overflow crash.
  - `MAXDRAWSEGS` (256): Overflow / HOM.
  - `MAXVISSPRITES` (128): Sprites flicker in/out by sort order.
  - `MAXINTERCEPTS` (128): Overflow -> UB; "all-ghosts" bug.
  - `MAXOPENINGS` (16384): Column-clip overflow.
  - `MAXPLATS` (30): Fatal "P_AddActivePlat: no more plats!".
  - `SAVEGAMESIZE` (~180 KB): "Savegame buffer overrun" crash.
- Must include Chocorenderlimits-style counters.

## 🚫 Out of Scope
- Fixing the original DOS bugs.
- Changes to the terminal presentation layer.
- Demo compatibility changes (addressed in M1).
