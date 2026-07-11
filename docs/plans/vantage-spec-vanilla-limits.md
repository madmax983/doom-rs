# 🔭 Vantage: Spec for Vanilla Limits and Overflow Emulation

## Context
The engine currently has limit-removing behavior by default, allowing it to render and simulate maps that exceed the original DOS Doom's static buffer limits. However, for true vanilla compatibility, players need an opt-in mode that faithfully reproduces the exact limitations and overflow consequences of the original executable.

## 👤 User Story
"As a classic Doom player and mapmaker, I want to play maps with strict vanilla limits enforced, so that I can experience the game exactly as it behaved on DOS, including accurate visual glitches and crashes when limits are exceeded."

## 🎯 The "So What?" Ask
**What business problem does this solve?**
Doom's original limitations are a core part of its historical identity. Many classic mods and community challenges rely on or work around these exact limits. By not supporting them, we alienate purists, speedrunners, and historians who require 100% accurate DOS reproduction. Providing an opt-in vanilla compatibility mode expands our target audience to include the hardcore retro-gaming community without sacrificing the default limit-removing robustness.

## 📈 Metric Definition
- **Success Metric:** 100% of known limit-breaking maps produce the exact same overflow symptoms or crashes in `doom-rs` (when compat mode is enabled) as they do in Chocolate Doom.

## ✅ Acceptance Criteria
- Must introduce a `vanilla-compat` toggle (e.g., via CLI argument `--vanilla-compat`).
- When enabled, the engine must simulate fixed-size buffers with vanilla caps.
- Must accurately reproduce overflow consequences.
- The default behavior must remain limit-removing to ensure modern QoL for regular players.

## 🚫 Out of Scope
- Emulating arbitrary memory corruption or host OS crashes beyond the known, documented vanilla overflow effects.

## ⚖️ Gap Analysis
`doom-rs` currently uses dynamic buffers, effectively making the renderer limit-removing. `limits.rs` caps are verification bounds, often smaller than vanilla (e.g. `MAX_VISIBLE_THINGS=64` vs vanilla `128`) and non-fatal. The engine needs a configuration-driven toggle that swaps the dynamic buffers for fixed arrays and implements the exact overflow failure modes.
