**[Eliminate GameState Clone on Load]**
**Learning:** `GameState` was being deep cloned on load (`*gs = payload.state.clone()`) because `apply_save` took `&SaveGame` instead of consuming `SaveGame`. Transferring ownership directly eliminates this massive heap allocation per load.
**Action:** Always prefer transferring ownership of large parsed/loaded structures to pointer targets over expensive deep clones. Change the function signature to take by value if the payload is not needed elsewhere.
**[Eliminate GameState Clone on Load]**
**Learning:** `GameState` was being deep cloned on load (`*gs = payload.state.clone()`) because `apply_save` took `&SaveGame` instead of consuming `SaveGame`. Transferring ownership directly eliminates this massive heap allocation per load.
**Action:** Always prefer transferring ownership of large parsed/loaded structures to pointer targets over expensive deep clones. Change the function signature to take by value if the payload is not needed elsewhere.
