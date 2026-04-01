import os

with open('.jules/atlas.md', 'a') as f:
    f.write('''
**[Encapsulate Sector Specials]**
**Tangle:** The `doom-game` crate exposed numerous sector special handling functions (`ev_*`, `tick_sector_specials`, `tick_doors`, etc.) in `specials.rs` as part of its public API via `lib.rs` (`pub use specials::{...}`). However, these functions are purely internal simulation details for the Doom engine and are only used within `doom-game` itself (specifically in `tic.rs` and `linedef_dispatch.rs`). This is a "Leaky Abstraction" that increases the API surface area unnecessarily.
**Blueprint:** Altered the visibility of these 44 functions in `specials.rs` from `pub fn` to `pub(crate) fn`. Removed them from the `pub use specials::*` block in `lib.rs`. Also added `#[allow(dead_code)]` to constants and functions that are not currently used (but port original Doom logic and might be needed later) to fix unused code warnings after lowering visibility. This strictly enforces the domain boundary of the crate.
''')
