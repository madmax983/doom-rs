💡 What: Modified `savegame::apply_save` to take ownership of `SaveGame` payload by value instead of by reference, eliminating a massive deep copy `.clone()` operation on the entire `GameState`.

🎯 Why: During game load (or quick load), the deserialized `SaveGame` object is completely consumed and discarded right after restoring the `GameState`. Because the `apply_save` function originally took the `payload` by reference, we were forced to execute `*gs = payload.state.clone()`. For a large, complex `GameState` containing thousands of structs and slabs, this triggers massive intermediate heap allocations and deep copies that were immediately thrown away when the initial load payload dropped.

📊 Impact: Removes one complete deep-copy of the `GameState` per save load or quick load.

🔬 Measurement: Run `cargo bench` or profile game loading; or simply observe the removed `.clone()` in `savegame.rs`.
