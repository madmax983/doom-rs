# Vantage: Savegame Parity Status

## User Story
As a player, I want save/load behavior to be honest about which format is in use, so I can preserve progress without the engine silently claiming vanilla compatibility it does not yet have.

## Current Boundary
- `SaveFormat::DoomRs` is the existing engine-native serializer. It remains the only fully implemented payload format in this pass.
- `SaveFormat::VanillaDsg` is now an explicit compatibility target. The engine detects plausible vanilla/chocolate Doom headers and routes them to a separate vanilla module.
- Full vanilla payload read/write is still unsupported. Strict mode now fails with a format-specific error instead of silently writing or loading `DRS1`.

## Acceptance Criteria For This Pass
- Extended mode writes and reads explicit `DoomRs` saves without behavior regression.
- Strict compatibility mode never falls back to the custom `DoomRs` serializer behind the user's back.
- Savegame format detection distinguishes `DRS1` from a plausible vanilla DSG header.
- Attempting to save or load vanilla DSG payloads fails explicitly with a readable unsupported-format error.
- Corrupted or mismatched save files fail gracefully and return control to the app without crashing.

## Deferred Work
- Exact vanilla payload serialization and deserialization.
- Cross-engine interoperability claims such as "Chocolate Doom can load our saves" or "we load 100% of vanilla saves".
- Save slot UI parity beyond the current app behavior.

## Non-Goals
- Cloud saves or cross-device syncing.
- Broader save slot management redesign.

## Honest Status
- Header-level vanilla boundary: implemented.
- Full vanilla savegame parity: not implemented yet.
- DoomRs deterministic round-trip path: still implemented and covered by tests.
