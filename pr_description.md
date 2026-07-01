⚒️ Forge: Refactor manual sector loops for floor movers

🚽 Smell: In `crates/doom-game/src/specials.rs`, `activate_floors` contains verbose, repeated `for` loops iterating over all sectors to calculate targets and dispatch `activate_floor_raise_single_typed` and `activate_floor_lower_single_typed` for numerous linedef types (56, 65, 94, 36, 69, 70, 71, 98).

✨ Solution: Extracted the repeated logic into two clear helper functions: `ev_floor_raise_to_lowest_ceiling_minus_8` and `ev_floor_lower_to_highest_plus_8`. Replaced the manual loops in the `match` arms with calls to these new helpers.

🧹 Benefit: Eliminates boilerplate and cognitive load in the massive `activate_floors` dispatch function, conforming to DRY principles, and bringing these cases in line with existing helpers like `ev_floor_raise_to_lowest_ceiling`.

🛡️ Verification: Tests passed. No logic changed.
