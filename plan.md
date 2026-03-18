1. **Remove `.collect::<Vec<_>>()` in `dlog_live_enemies` (crates/doom-app/src/main.rs:632)**
   - Iterate over `iter_handles()` directly instead of creating an intermediate `Vec`.

2. **Remove `.collect::<Vec<_>>()` in `dlog_death_events` (crates/doom-app/src/main.rs:676)**
   - Iterate over `iter_handles()` directly instead of creating an intermediate `Vec`.

3. **Pre-allocate `Vec` in `save_game` (crates/doom-game/src/savegame.rs:1045)**
   - Use `Vec::with_capacity(gs.mobjslab.len())` and `.extend()` instead of `.collect()`.

4. **Remove `.collect::<Vec<_>>()` in `specials::conveyor_belts` (crates/doom-game/src/specials.rs:3608)**
   - Iterate over `iter_handles()` directly instead of creating an intermediate `Vec`.

5. **Remove `.collect::<Vec<_>>()` in `projectile::missile_move` (crates/doom-game/src/projectile.rs:318)**
   - Iterate over `iter_handles()` directly instead of creating an intermediate `Vec`.

6. **Remove `.collect::<Vec<_>>()` in `combat::radius_attack` (crates/doom-game/src/combat.rs:471)**
   - Iterate over `iter_handles()` directly instead of creating an intermediate `Vec`.

7. **Remove `.collect::<Vec<_>>()` in `handle_sound_events` call inside `doom-app/src/main.rs:986`**
   - Refactor `handle_sound_events` to take `impl Iterator<Item = doom_game::SoundRequest>` instead of a slice, avoiding `Vec` allocation from `drain`.

8. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
