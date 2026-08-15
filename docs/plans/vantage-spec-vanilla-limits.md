# 🔭 Vantage: Spec for Vanilla Limits & Overflow Emulation

## 👤 User Story
As a Vanilla Doom Player, I want to play with strict original engine limits (like visplane overflows and intercepts bounds) enabled via a toggle, so that I can ensure my experience and demo recordings are compatible with the original DOS release behavior.

## 💼 The "So What?" (Business Problem)
Currently, the engine uses dynamic buffers making it effectively limit-removing, which makes it robust but historically inaccurate. The core Doom community relies on precise reproduction of vanilla bugs and limits (e.g. visplanes, drawsegs, intercepts) to validate map compatibility and ensure demo sync. Providing an opt-in vanilla limits mode allows us to compete directly with Chocolate Doom's bug-for-bug replica standard, capturing the purist market without sacrificing modern stability.

## 📊 Metric Definition
- **Success** = 100% reproduction of static limits (visplanes, drawsegs, vissprites, intercepts, openings, active plats, savegame buffer) and their overflow consequences as defined in the Vanilla-fidelity reference data.
- **Success** = Limit-removing behavior remains the default when the vanilla-compat toggle is off.

## 🕳️ Gap Analysis
- **Current State:** The engine uses dynamic buffers and is effectively limit-removing by default. There is no overflow emulation.
- **Market Standard (Chocolate Doom):** Deliberately preserves exact static limits (`MAXVISPLANES=128`, `MAXDRAWSEGS=256`, `MAXVISSPRITES=128`, `MAXINTERCEPTS=128`, etc.) and reproduces their overflow crashes and rendering artifacts (e.g., visplane crash, all-ghosts bug).
- **The Gap:** We lack the ability to swap the dynamic buffers for fixed vanilla-sized arrays and correctly emulate their original overflow consequences.

## ✅ Acceptance Criteria
- Must implement an opt-in `vanilla-compat` toggle.
- Must preserve current limit-removing behavior as the default state.
- Must reproduce the 128 visplane overflow crash ("No more visplanes").
- Must reproduce the 256 drawseg overflow / HOM failure mode.
- Must reproduce the 128 vissprite flicker failure mode.
- Must reproduce the 128 intercepts overflow and resulting "all-ghosts" bug.
- Must reproduce the 16384 openings column-clip overflow.
- Must reproduce the 30 active plats fatal error ("P_AddActivePlat: no more plats!").
- Must reproduce the ~180 KB savegame buffer overrun crash.

## 🚫 Out of Scope
- Breaking demo compatibility by forcing the vanilla-compat mode as the default behavior.
