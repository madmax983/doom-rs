import re

with open('crates/doom-game/src/actions.rs', 'r') as f:
    content = f.read()

# Refactored Mobj coordinate extraction for sound emission in doom-game/src/actions.rs
# Action: Use an idiomatic if let Some(mo) = gs.mobjslab.get(handle) block

# The instructions mentioned:
# **Refactored `Mobj` coordinate extraction for sound emission in `doom-game/src/actions.rs`**
# **Learning:** Extracting `mo.x` and `mo.y` via `.map(|mo| (mo.x, mo.y)).unwrap_or_default()` when checking if an entity exists is unnecessarily verbose and causes Boolean Blindness by creating intermediate default values (0, 0) that are immediately consumed.
# **Action:** Use an idiomatic `if let Some(mo) = gs.mobjslab.get(handle)` block to directly access the entity's coordinates and embed the dependent logic (like pushing to a `sound_queue`) inside the block.

# Another rule:
# **Refactor Mobj field extraction using if-let guard clauses**
# **Learning:** `match gs.mobjslab.get(handle) { Some(mo) => (mo.x, mo.y), None => return, };` with tuple destructuring repeats boilerplate, is noisy, and has an indentation hit.
# **Action:** Replace `match` blocks used solely for extracting values into tuples with idiomatic `if let Some` guard clauses (`let Some(mo) = gs.mobjslab.get(handle) else { return; }; let x = mo.x;`) to improve linearity and readability.
