**Extract logic and flatten match in activate_doors**
**Learning:** `activate_doors` in `specials.rs` was a 300+ line "God Function" with a massive `match` statement. Every key door logic arm repeated identical code to fetch the `sector_idx` from the `left_sidedef` and to perform the item check logic.
**Action:** Created `check_door_lock` as a centralized helper function to handle the key lookups with an early return, mapped identical logic paths to identical match arms (e.g. `26 | 27 | 28 =>`), and utilized a lazily evaluated local closure to eliminate boiler plate.
