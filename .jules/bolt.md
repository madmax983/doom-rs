**[Apply Save GameState Clone]
**Learning:** Found a massive clone of `GameState` occurring on every load inside `apply_save`. The payload `SaveGame` is parsed and single-use, so borrowing it just to clone the state out is wasteful.
**Action:** Always check if a deeply cloned argument inside a function can be taken by value to transfer ownership instead, especially when it's a single-use deserialized payload.
