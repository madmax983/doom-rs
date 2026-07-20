# Spec: Vanilla Limits and Overflow Emulation (M2)

## The "So What?" Ask
What business problem does this solve?
Purist players and speedrunners rely on exact vanilla behavior, including limitations and overflow glitches (like visplane crashes, tutti-frutti, and medusa effects). Without these, our engine cannot act as a 1:1 drop-in replacement for Chocolate Doom or original DOS Doom, limiting its adoption among the hardcore demographic.

## User Story
As a purist player or speedrunner, I want an opt-in vanilla limits mode, so that I can experience classic overflow behavior exactly as it occurred in the original DOS release.

## Metric Definition
Success = Known limit-breaking maps (e.g., visplane overflow maps) overflow at the exact same points and in the same ways as in Chocolate Doom.

## Gap Analysis (from PARITY_ROADMAP.md)
Currently, `doom-rs` is effectively limit-removing, using dynamic `Vec`/`ArrayVec` clip buffers and non-fatal verification-oriented caps (e.g., `MAX_VISIBLE_THINGS=64` vs vanilla 128). This is listed as DIVERGENT in the `PARITY_ROADMAP.md` (Milestone M2). We need to reintroduce fixed-size buffers with vanilla caps and reproduce the overflow consequences, gated behind a `vanilla-compat` toggle to preserve our limit-removing default.

## Acceptance Criteria
- A `vanilla-compat` toggle is introduced (via CLI or config).
- The default behavior remains limit-removing.
- When enabled, dynamic clip/plane/sprite buffers are swapped for fixed vanilla-sized arrays.
- Overflow consequences match vanilla (visplane crash, tutti-frutti, medusa, all-ghosts/intercepts, spechit, "no more plats", savegame-buffer overrun).
- Test suite validates overflows match Chocolate Doom behavior.

## Out of Scope
- Pixel-identical display output and DOS input-timing (tracked separately in Abrash integration).
- Complete demo determinism (handled in M1).
