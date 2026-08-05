# 🔭 Vantage: Spec for Vanilla Limits & Overflow Emulation (M2)

## 👤 User Story
"As a demo author or retro speedrunner, I want an opt-in mode that accurately emulates original DOS engine limits and memory overflows, so that I can reproduce historical bugs like Visplane Overflows, HOM effects, and the All-Ghosts bug exactly as they appeared in Chocolate Doom and DOS."

## ✅ Acceptance Criteria
- Must introduce a global or context-level `vanilla-compat` toggle to enable this mode.
- The default behavior must remain *limit-removing* (using dynamic buffers like `Vec`).
- When `vanilla-compat` is enabled, the engine must use fixed vanilla-sized arrays or cap values and reproduce the consequences:
  - **Visplanes (`MAXVISPLANES` = 128):** Must trigger a fatal "No more visplanes" crash.
  - **Drawsegs (`MAXDRAWSEGS` = 256):** Must overflow and cause Hall of Mirrors (HOM) / visual clipping.
  - **Vissprites (`MAXVISSPRITES` = 128):** Excess sprites must flicker out based on sort order.
  - **Intercepts (`MAXINTERCEPTS` = 128):** Must overflow, resulting in Undefined Behavior (UB) emulation or the "All-ghosts" bug.
  - **Openings (`MAXOPENINGS` = 16384):** Must cause column-clip overflow rendering artifacts.
  - **Active plats (`MAXPLATS` = 30):** Must trigger a fatal "P_AddActivePlat: no more plats!" crash.
  - **Savegame buffer (`SAVEGAMESIZE` = ~180 KB):** Must trigger a fatal "Savegame buffer overrun" crash.
- Must include verification tools (like Chocorenderlimits-style counters) or test oracles to verify bounds during test execution.
- Known limit-breaking maps (e.g. from the test suite) must trigger the correct overflows at the exact same points as Chocolate Doom.

## 🚫 Out of Scope
- Altering the default limit-removing behavior of the engine.
- Fixing the underlying bugs or overflows in `vanilla-compat` mode; the goal is accurate reproduction of the original flaws.
- Demo synchronization logic (M1) or IWAD auto-detection (M3).
