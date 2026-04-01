**Avoid `Vec::with_capacity` snapshot allocations when iterating over `MobjSlab` mutating operations**
**Learning:** `Vec` allocations can be avoided entirely when mutating an arena structure by capturing `slot_count` and `next_generation` bounds before the loop.
**Action:** Use the pattern `let initial_slot_count = slab.slot_count(); let initial_generation = slab.next_generation();` and check `handle.generation >= initial_generation` to safely iterate and mutate without intermediate vectors.

**Avoid `Vec::with_capacity` snapshot allocations when iterating over `MobjSlab` mutating operations**
**Learning:** `Vec` allocations can be avoided entirely when mutating an arena structure by capturing `slot_count` and `next_generation` bounds before the loop.
**Action:** Use the pattern `let initial_slot_count = slab.slot_count(); let initial_generation = slab.next_generation();` and check `handle.generation >= initial_generation` to safely iterate and mutate without intermediate vectors.
