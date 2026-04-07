# Vantage: Demo Parity Status

## User Story
As a speedrunner or content creator, I want demo recording and playback to preserve vanilla Doom input bytes faithfully, so recorded runs can be analyzed, replayed, and compared without action bits silently disappearing.

## Current Status

- Strict demo tic payloads now match the vanilla four-byte shape: `forwardmove`, `sidemove`, one-byte turn, and one-byte buttons.
- Action bits such as attack, use, and weapon changes survive `TicCmd -> DemoTicCmd -> TicCmd` round-trips.
- App-level playback coverage now asserts that attack/use commands reach gameplay logic instead of being dropped during demo playback.
- This closes the byte-format drift, not the full determinism story.

## Acceptance Criteria Closed

- File-format compatibility for vanilla demo tic bytes: implemented.
- End-to-end preservation of action buttons in local playback tests: implemented.
- Command-path support for strict record/playback flow: implemented.

## Still Open

- Long-run deterministic playback against Chocolate Doom or vanilla reference demos is not yet proven.
- Multiplayer demo parity remains deferred.
- Demo byte correctness alone does not prove gameplay/RNG consumption order.

## Non-Goals For This Pass

- Advanced playback controls such as rewind, pause-step, or seeking.
- MBF/Boom demo formats.
- Claiming full demo-sync parity before the long-run validation work exists.

## Gap Analysis

The old byte-level incompatibility is gone. The remaining demo risk is downstream of
simulation parity:

- RNG consumption order across gameplay systems
- monster AI and combat sequencing
- long-run playback validation against external reference demos
