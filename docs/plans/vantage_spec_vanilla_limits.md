# Specification: Vanilla Limits Compatibility Mode

## 👤 User Story
As a Doom purist and map maker, I want to play and test maps under strict vanilla DOS Doom engine limits, so that I can ensure my levels are compatible with the original executable and experience historical quirks exactly as they were in 1993.

## ❓ The "So What?" Ask
What business problem does this solve?
Currently, our engine is limit-removing by default. While this is great for modern WADs, it alienates our core demographic of purists who rely on accurate vanilla constraints for testing. Adding an opt-in compat mode captures this segment without compromising the modern default experience, directly addressing Milestone 2 (M2) in the Parity Roadmap.

## 📊 Metric Definition
Success = 100% of known limit-breaking maps crash or glitch at the exact same geographic points as they do in Chocolate Doom.

## 🔍 Gap Analysis
Currently, dynamic buffers make the renderer effectively limit-removing, and our `limits.rs` caps are verification bounds, often smaller than vanilla (e.g. `MAX_VISIBLE_THINGS=64` vs vanilla 128) and non-fatal. We lack an execution path that artificially caps these buffers to vanilla values and reproduces overflow consequences.

## ✅ Acceptance Criteria
- Must introduce a `vanilla-compat` toggle.
- Must preserve limit-removing behavior as the default when the toggle is off.
- When toggled on, must artificially cap dynamic buffers to vanilla values and reproduce overflow consequences.
- Must accurately emulate vanilla limits, replacing current smaller non-fatal bounds (e.g. `MAX_VISIBLE_THINGS=64`) with accurate vanilla limits (e.g. 128).

## 🚫 Out of Scope
- Implementation details of how the fixed-size buffers are allocated in Rust.
- Defining specific Structs or Enums for configuration management.
