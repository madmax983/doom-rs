# 🔭 Vantage: Spec for Vanilla Limits & Overflow Emulation (M2)

## 👤 User Story
As a Doom Historian and speedrunner, I want to play maps exactly as they behaved on DOS, including their static limits and overflow crashes (like the visplane limit), so that I can practice authentic routing and experience historical maps exactly as authored.

## 💼 Business Problem (The "So What?")
doom-rs currently uses dynamic memory allocation (`Vec`/`ArrayVec`), effectively making it a "limit-removing" port. While this is great for modern enormous maps, it actively breaks the ability to use doom-rs as a historically accurate reference or for vanilla speedrunning. Vanilla behavior—including its bugs and crashes—is a required feature for our target audience.

## 📈 Success Metrics
- **Metric 1 (Fidelity):** 100% of maps that crash Chocolate Doom due to static limit overflows (e.g., visplanes > 128) must also overflow and exit in doom-rs when the compatibility mode is active.
- **Metric 2 (Safety):** Limit-removing mode remains the default. 0% impact to stability for players not opting into vanilla compatibility.

## 🔍 Gap Analysis
- **Market Standard:** Chocolate Doom strictly enforces DOS limits and crashes by design. Crispy Doom provides these limits but allows a limit-removing mode.
- **Current State:** doom-rs is strictly limit-removing. We lack fixed-size buffers for visplanes (128), drawsegs (256), vissprites (128), intercepts (128), openings (16384), and plats (30).

## ✅ Acceptance Criteria
- **Toggle:** A new CLI argument `--vanilla-compat` must be added to opt-in to strict limits.
- **Visplanes:** When active, drawing more than 128 visplanes must result in a fatal overflow crash ("No more visplanes").
- **Drawsegs:** When active, exceeding 256 drawsegs must cause the overflow/HOM visual bug.
- **Vissprites:** When active, exceeding 128 vissprites must cause sprites to flicker in/out based on sort order.
- **Intercepts:** When active, exceeding 128 intercepts must trigger the "all-ghosts" bug.
- **Plats:** When active, exceeding 30 active plats must result in a fatal "P_AddActivePlat: no more plats!" crash.
- **Default Behavior:** Without `--vanilla-compat`, the engine must remain limit-removing, allowing arbitrary sizes.

## 🚫 Out of Scope
- Automatically detecting if a WAD "requires" limit-removing (this is impossible to know definitively ahead of time).
- Emulating the "savegame buffer overrun" crash (deferred to M7 Vanilla savegame format work).
- Rewriting the rendering pipeline; we are only changing buffer sizing and overflow triggers.
