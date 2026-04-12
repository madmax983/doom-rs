
**Refactoring pattern: Use from_repr instead of huge match statements for enums**
**Learning:** `FromRepr` from `strum` can replace large, manual integer-to-enum matches, decreasing the number of lines of code and reducing cognitive load while remaining strictly typed.
**Action:** Use `#[derive(strum_macros::FromRepr)]` on primitive enums instead of writing manual matching logic.

## 2024-05-15 - Refactored sector geometry queries in `doom-game/src/specials.rs`
**Learning:** Functions like `lowest_adjacent_floor`, `highest_adjacent_floor`, `next_highest_floor`, `lowest_adjacent_ceiling`, and `highest_adjacent_ceiling` all repeated the exact same 30-line `for` loop to find adjacent sectors by iterating over all linedefs in the level and checking left and right sidedefs.
**Action:** Extracted the core loop into a single `adjacent_sectors` iterator function that yields `(usize, &Sector)`. The query functions now use functional iterator pipelines (e.g., `.map().min()`, `.map().filter().max()`) making the code significantly shorter, less error-prone, and easier to read without changing behavior.

**Refactored `Mobj` coordinate extraction for sound emission in `doom-game/src/actions.rs`**
**Learning:** Extracting `mo.x` and `mo.y` via `.map(|mo| (mo.x, mo.y)).unwrap_or_default()` when checking if an entity exists is unnecessarily verbose and causes Boolean Blindness by creating intermediate default values (0, 0) that are immediately consumed.
**Action:** Use an idiomatic `if let Some(mo) = gs.mobjslab.get(handle)` block to directly access the entity's coordinates and embed the dependent logic (like pushing to a `sound_queue`) inside the block.

**Flatten audio event dispatch**
**Learning:** The background audio command loop contained deeply nested `if let` and `match` blocks (Pyramid of Doom), making the main event dispatch obscured by indentation.
**Action:** Use guard clauses (`let Ok(x) = ... else { continue }`) to flatten deeply nested logic loops, significantly improving read flow without altering early-exit semantics.
**[Extracted and flattened Linedef/Specials Dispatch logic]
**Learning:** `crates/doom-game/src/linedef_dispatch.rs` and `crates/doom-game/src/specials.rs` contained significant amounts of boilerplate and repetitive iterator loops inside `match` statements across several special types (e.g. types 56, 65, 36, etc.). Iterating over `sectors` and doing `collect` on indices is slow and redundant. Using descriptive structs instead of arrays of unlabelled arguments provides much better documentation.
**Action:** Consolidate tag-matching iterator chains using `filter` and `map` to perform the transformation succinctly and cleanly. Extract large repetitive structures into generic closure-based helpers when applicable. Replace deeply nested condition checks with early returns and guards to reduce nesting density. Always maintain zero-overhead logic.
