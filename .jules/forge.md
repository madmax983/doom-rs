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

**[Boolean Blindness in activate_crusher]
**Learning:** `activate_crusher` taking multiple booleans like `silent` and `remove_when_done` creates "Boolean Blindness", obscuring intent at the call site.
**Action:** Group these configuration flags into a named struct like `CrusherParams` to self-document call sites.

**[Avoid clone when using Copy types like LumpName]**
**Learning:** Using `clone()` on types that implement `Copy` (like `LumpName` which wraps an array of bytes) is redundant, causes Clippy warnings (`clone_on_copy`), and reduces readability without providing any safety benefit.
**Action:** Remove `.clone()` calls on instances of `Copy` types when passing them around or inserting them into collections.

**Refactor trace_ray actor checking to resolve Boolean Blindness**
**Learning:** Functions that accept a boolean flag to enable a feature (like `check_actors: bool`) alongside optional data required only when that flag is true (like `shooter_index` and `actor_positions`) suffer from Boolean Blindness and disconnected parameters.
**Action:** Group the boolean flag and its dependent data into a strongly typed enum (e.g., `ActorCheck::Ignore` and `ActorCheck::Check { shooter_index, actor_positions }`) to enforce correct usage at compile time and clarify intent at call sites.

**Refactored Boolean Blindness in door and floor specials**
**Learning:** Functions like `open_door(gs, level, idx, true)` and `ev_build_stairs(gs, level, idx, type, false)` suffer from Boolean Blindness, hiding the true intent (`OpenWaitClose` and `CrushBehavior::NoCrush`).
**Action:** Replace `bool` with descriptive enums like `DoorBehavior` (`OpenStay` vs `OpenWaitClose`) and `CrushBehavior` (`Crush` vs `NoCrush`) to strongly type API boundaries and self-document the code at the call site.
**Refactored duplicated `match` statements over `SoundRequest` in `handle_sound_events`**
**Learning:** Redundant `match` statements that extract values from an enum based on its variant create code duplication and visual noise, especially when the same matching logic is repeated within the same function or module.
**Action:** Extract the matching logic into helper methods (e.g., `emitter(&self)` and `origin_handle(&self)`) on the enum itself using an `impl` block to encapsulate the behavior and simplify the call sites.

**[Boolean Blindness in spawn_level_things and sync_weapon_anim]**
**Learning:** Functions like `spawn_level_things` taking `is_deathmatch: bool` alongside `carry_player_state: bool` in `load_map_after_intermission` creates "Boolean Blindness", making calls like `spawn_level_things(&mut gs, &level, Skill::Medium, false)` hard to understand. Similarly, `sync_weapon_anim_from_player_psprites(false)` hides intent about what the boolean does (preserve motion vs reset).
**Action:** Replaced `bool` with enums like `GameMode` (`SinglePlayer` vs `Deathmatch`), `PlayerStateCarry` (`Carry` vs `Reset`) and `WeaponMotion` (`Preserve` vs `Reset`) to self-document the code at call sites and enforce correct usage at compile time.

**Refactor sidedef sector extraction guard clauses**
**Learning:** `activate_doors` repeatedly used two lines to extract a `sector_idx` from `left_sidedef`: `let Some(sd) = level.sidedefs.get(left_sidedef as usize) else { return; }; let sector_idx = sd.sector as usize;`. This pattern was repeated 16 times inside a `match` statement, creating verbose boilerplate.
**Action:** Use `.map(|sd| sd.sector as usize)` directly in the guard clause: `let Some(sector_idx) = level.sidedefs.get(left_sidedef as usize).map(|sd| sd.sector as usize) else { return; };` to eliminate the unused intermediate `sd` variable and compress the boilerplate into a single statement.
**[Refactored Boolean Blindness in weapon_anim]**\n**Learning:**  using mutually exclusive booleans  and  allowed impossible states (both true) and complicated the update logic across multiple modules.\n**Action:** Replaced boolean state flags with a strictly typed  enum (, , ) to enforce valid states and clean up update/check conditionals.
**[Refactored Boolean Blindness in weapon_anim]**
**Learning:** `WeaponSprite` using mutually exclusive booleans `raising` and `lowering` allowed impossible states (both true) and complicated the update logic across multiple modules.
**Action:** Replaced boolean state flags with a strictly typed `WeaponTransition` enum (`None`, `Raising`, `Lowering`) to enforce valid states and clean up update/check conditionals.

**Refactor Mobj field extraction using if-let guard clauses**
**Learning:** `match gs.mobjslab.get(handle) { Some(mo) => (mo.x, mo.y), None => return, };` with tuple destructuring repeats boilerplate, is noisy, and has an indentation hit.
**Action:** Replace `match` blocks used solely for extracting values into tuples with idiomatic `if let Some` guard clauses (`let Some(mo) = gs.mobjslab.get(handle) else { return; }; let x = mo.x;`) to improve linearity and readability.

**Refactoring large match statements using from_repr**
**Learning:** `tick_sector_specials` and `tick_sector_damage` used redundant, hardcoded magic numbers inside match blocks across `SectorDamageType`.
**Action:** Extract magic numbers into explicitly named constants (e.g., `LEGACY_DAMAGE_HELLSLIME`) to self-document the values, and replace duplicated health-reduction inline logic with calls to a common `apply_sector_damage` helper function to enforce DRY principles without altering runtime behavior.
## 2024-05-16 - Refactored parse_* in DeHackEd parser
**Learning:** Returning a dummy error using `"".parse::<f64>().unwrap_err()` to fail through an `and_then` block is a strange hack and reduces code readability.
**Action:** Replace dummy error hacks by mapping the errors properly, or replacing `unwrap_err` with standard idiomatic Rust error handling constructs.

## 2024-05-17 - Refactored `parse_*` float fallback hacks in `dehacked.rs`
**Learning:** When refactoring error-handling closures in Rust (such as `or_else(|_| ...)` or `map_err(|_| ...)`), you may encounter `error[E0282]: type annotations needed` if the new code removes the type constraints the compiler relied on.
**Action:** Resolve this by explicitly annotating the closure parameter type (e.g., `|_: std::num::ParseIntError|` or `|_: ()|`).

## 2024-05-18 - Refactored Option destructuring boolean blindness in MapAnalyzer
**Learning:** `if let (Some(a), Some(b)) = (map.get(x), map.get(y))` combined with `unwrap()` fallback on the values causes boolean blindness and leads to verbose error-prone code, especially when updating parent-child tree states inside a graph loop.
**Action:** Always prefer cleanly copying small `Copy` primitive values from maps inside a guard block `let (a, b) = (map.get(x).copied(), map.get(y).copied())` and testing `if let (Some(x), Some(y))` to flatten Option extraction without requiring runtime unwraps.

**Refactored Boolean Blindness in FaceState::tick**
**Learning:** Functions taking multiple booleans like `is_firing` and `is_invulnerable` in `face.tick` create Boolean Blindness, obscuring intent at the call site (e.g. `face.tick(100, false, true, None)`).
**Action:** Replaced `bool` with descriptive enums like `WeaponFiring` (`Firing` vs `NotFiring`) and `Invulnerability` (`Active` vs `Inactive`) to self-document the code at call sites and enforce correct usage at compile time.
