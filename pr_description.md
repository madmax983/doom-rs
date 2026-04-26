**Smell:**
There were two heavily duplicated `match result { ... }` blocks inside `tick` within `crates/doom-app/src/main.rs`, handling the same menu results (`StartGame`, `Quit`, `LoadGame`, `SaveGame`) repeatedly in both the title screen state and the in-game menu state. This violated DRY principles, increased cognitive load, and created a large "Pyramid of Doom" directly inside the game's core `tick` loop.

**Solution:**
Extracted the duplicated code into a cohesive, private helper function `fn handle_menu_result(&mut self, result: doom_game::menu::MenuResult)`. Replaced both matching blocks inside `tick` with simple `self.handle_menu_result(result)` calls.

**Benefit:**
Reduces technical debt, shrinks the size of the 350+ line `tick` function by over 100 lines, removes duplicated logic entirely, and standardizes menu side-effect handling to exactly one location.

**Verification:**
Tests passed. `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` run clean. No runtime logic changed, this is purely a zero-behavior-change structural refactor.
