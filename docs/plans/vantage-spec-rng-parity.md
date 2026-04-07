# Vantage: RNG Parity Status

## User Story
As a player and speedrunner, I want the game's random number generation to match vanilla Doom's table and consumption order, so demos stay in sync and combat outcomes feel authentic.

## Current Status

- The engine already uses Doom's canonical 256-byte lookup table.
- Table stepping behavior is aligned with vanilla's modulo-256 walk.
- This is necessary, but not sufficient, for demo determinism.

## Acceptance Criteria Closed

- Exact RNG table contents: implemented.
- Core table-index advancement model: implemented.

## Still Open

- Consumption-order parity across all gameplay systems is not yet proven.
- Long-run deterministic validation against reference demos is still required.
- Separation between gameplay RNG and non-deterministic cosmetic/UI randomness still needs a holistic audit.

## Non-Goals For This Pass

- Replacing the lookup table with a modern RNG algorithm.
- "Improving" vanilla's exploitable RNG behavior.
- Claiming full demo-sync parity from table parity alone.

## Gap Analysis

The remaining RNG risk is not the table itself. It is call-site behavior:

- extra or missing `P_Random` uses in gameplay code
- wrong ordering between monster AI, combat, and weapon systems
- accidental use of gameplay RNG by non-deterministic systems

The next real proof step is end-to-end demo validation, not another table audit.
