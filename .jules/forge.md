
**Refactoring pattern: Use from_repr instead of huge match statements for enums**
**Learning:** `FromRepr` from `strum` can replace large, manual integer-to-enum matches, decreasing the number of lines of code and reducing cognitive load while remaining strictly typed.
**Action:** Use `#[derive(strum_macros::FromRepr)]` on primitive enums instead of writing manual matching logic.

## 2024-05-15 - Refactored sector geometry queries in `doom-game/src/specials.rs`
**Learning:** Functions like `lowest_adjacent_floor`, `highest_adjacent_floor`, `next_highest_floor`, `lowest_adjacent_ceiling`, and `highest_adjacent_ceiling` all repeated the exact same 30-line `for` loop to find adjacent sectors by iterating over all linedefs in the level and checking left and right sidedefs.
**Action:** Extracted the core loop into a single `adjacent_sectors` iterator function that yields `(usize, &Sector)`. The query functions now use functional iterator pipelines (e.g., `.map().min()`, `.map().filter().max()`) making the code significantly shorter, less error-prone, and easier to read without changing behavior.
