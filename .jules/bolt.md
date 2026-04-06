**Determinism and Slab Iteration**
**Learning:** When attempting to remove `Vec::with_capacity().extend()` in hot paths that iterate over a Generational Arena (Slab) like `MobjSlab`, replacing it with live index-based iteration (`0..slot_count`) can introduce critical bugs. If the iteration loop mutates the slab (e.g., by freeing entities or spawning new ones), live index iteration may process newly spawned entities on the *same tick* they were created, breaking the precise deterministic tick order required by engines like Doom.
**Action:** Always maintain a `Vec` snapshot of live handles (or another non-allocating snapshot mechanism) before ticking entities if the system allows mutations to the slab during iteration. Only use live index iteration for read-only passes (e.g., saving games or logging) where determinism is not at risk.
## 2025-03-22 - Remove unnecessary allocations in sector specials
**Learning:** `ev_floor_...` and `ev_teleport` were redundantly allocating intermediate vectors through `.collect()` on iterator pipelines before iterating over them.
**Action:** Replace `collect()` in favor of direct chaining or iterator folding / lazy position matching (`.iter().position()`) for significant performance improvements across common game event checks, bypassing memory allocation altogether and avoiding fighting the borrow checker by separating read-only and mutating iterations correctly when NLL allows or by cloning only minimal identifiers.
**[NLL Enables Zero-Allocation Iteration in Game State Modifiers]**
**Learning:** Rust's Non-Lexical Lifetimes (NLL) allow us to disjointly borrow immutable components of a struct (e.g., `level.sectors`) while mutably passing another component (`gs`) into a function inside a loop. We do not need to `.collect::<Vec<_>>()` iterator results into an intermediate array just to appease the borrow checker in these cases.
**Action:** When a game loop filters and processes entities from an immutable level definition to apply them to a mutable game state, move the iterator chain directly into the `for` loop, eliminating unnecessary heap allocations on the hot path.

## 2025-03-27 - MobjSlab len Optimization
**Learning:** Computing `len()` on a generational arena by iterating over all slots `slots.iter().filter(...).count()` is O(N) and creates unnecessary overhead on hot paths where allocations or tick operations occur frequently. Adding a `live_count` field turns it into an O(1) read.
**Action:** Track active elements with an explicit `live_count` in custom slab structures when `len()` is called frequently, ensuring to correctly maintain the count during `alloc`, `free`, and `clear` operations.
**[AABB check before Euclidean .sqrt() in radial math]**
**Learning:** Performing a `.sqrt()` calculation on every single actor in `p_radius_attack` creates unnecessary floating-point operations for actors far outside the explosion radius.
**Action:** Adding an Axis-Aligned Bounding Box (AABB) early-out check (`dx.abs() >= radius || dy.abs() >= radius`) quickly skips actors out of range before computing the exact Euclidean distance, saving CPU cycles on the hot path.
**Doc comments on local let bindings**
**Learning:** In Rust, applying an outer doc comment (`///`) to a local statement (such as a `let` binding) inside a function body causes a rustdoc compilation error (`error[E0585]: found a documentation comment that doesn't document anything`).
**Action:** Use standard comments (`//`) instead for inline explanations within functions, even if prompted to "add doc comments (`///`) explaining why the optimization matters" (apply this rule only to item definitions like structs, functions, modules, etc., not inline logic).
**[Eliminated intermediate collection in wad lump scanning]**
**Learning:** `Vec::collect()` intermediate collections over iterators just to iterate over them again later via `.into_iter()` is wasteful. We can preserve an `impl Iterator` to process items continuously and eliminate the initial `Vec` buffer entirely, avoiding temporary allocation overhead at startup.
**Action:** When filtering or mapping data from an underlying collection to form a list that will be consumed downstream, prefer returning a lifetime-bound `impl Iterator` instead of a full `Vec` wherever the call chain allows for lazy iteration.
