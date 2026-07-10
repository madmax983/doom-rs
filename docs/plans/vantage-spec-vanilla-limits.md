# Vantage Spec: Vanilla Limits (Opt-In Compat Mode)

## User Story
As a purist player and demo speedrunner, I want an opt-in vanilla compatibility mode that enforces original static limits, so that I can reproduce historical overflow behaviors (such as the all-ghosts bug or visplane crashes) exactly as they occurred in DOS Doom.

## "So What?" Ask
What business problem does this solve? It fulfills the core parity mission for the hardcore retro community. By keeping it opt-in, we retain modern robustness for casual users while providing 100% historical accuracy for purists and exact demo playback capability.

## Metric Definition
Success = 100% exact reproduction of limit overflow points (crashes and glitches) on known reference WADs when compared to Chocolate Doom.

## Gap Analysis
Currently, the engine uses dynamic buffers and is effectively "limit-removing". The market reference (Chocolate Doom) reproduces these strict static limits (e.g., 128 Visplanes) and their resulting glitches (HOM, tutti-frutti, medusa). We lack an opt-in mechanism to simulate these historical constraints.

## Acceptance Criteria
- Must be strictly opt-in; the default engine behavior must remain limit-removing.
- Must accurately enforce vanilla limit numbers (e.g., Visplanes=128, Drawsegs=256, Intercepts=128).
- Must accurately reproduce the vanilla failure modes (e.g., "No more visplanes" crash, "all-ghosts" bug).
- Must provide Chocorenderlimits-style counters for debugging and testing.

## Out of Scope
- Fixing the original overflow bugs when in compat mode.
- Adjusting presentation layer resolutions (this is purely simulation parity).
- Rewriting the default dynamic buffer system.
