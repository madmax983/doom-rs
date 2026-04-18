2. **Refactor `handle_sound_events`**:
   Replace the two `match ev { ... }` blocks in `crates/doom-app/src/main.rs` (`handle_sound_events`) with calls to `ev.emitter(pl_x, pl_y)` and `ev.origin_handle(player_origin)`.

   We will use `replace_with_git_merge_diff` with this exact block:
   ```
<<<<<<< SEARCH
            let emitter = match ev {
                SoundRequest::MonsterWake(_, _, x, y)
                | SoundRequest::MonsterAttack(_, _, x, y)
                | SoundRequest::MonsterDie(_, _, x, y) => Some((x, y)),
                SoundRequest::PlayerWeaponFire(_)
                | SoundRequest::PlayerSuperShotgunOpen
                | SoundRequest::PlayerSuperShotgunLoad
                | SoundRequest::PlayerSuperShotgunClose => Some((pl_x, pl_y)),
                SoundRequest::PlayerDie
                | SoundRequest::PlayerUseFail
                | SoundRequest::PlayerUseLockedDoor(_) => None,
            };
            let origin = match ev {
                SoundRequest::MonsterWake(_, handle, _, _)
                | SoundRequest::MonsterAttack(_, handle, _, _)
                | SoundRequest::MonsterDie(_, handle, _, _) => Some(handle),
                SoundRequest::PlayerWeaponFire(_)
                | SoundRequest::PlayerSuperShotgunOpen
                | SoundRequest::PlayerSuperShotgunLoad
                | SoundRequest::PlayerSuperShotgunClose => player_origin,
                SoundRequest::PlayerDie
                | SoundRequest::PlayerUseFail
                | SoundRequest::PlayerUseLockedDoor(_) => None,
            };

            if lump.is_empty() {
=======
            let emitter = ev.emitter(pl_x, pl_y);
            let origin = ev.origin_handle(player_origin);

            if lump.is_empty() {
>>>>>>> REPLACE
   ```

3. **Run tests**: Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo fmt --all` to verify the refactor.
4. **Complete pre-commit steps**: Complete pre commit steps to make sure proper testing, verifications, reviews and reflections are done.
5. **Submit the change.** Create a PR titled "⚒️ Forge: Extract `SoundRequest` helper methods to reduce `match` duplication" detailing the Smell, Solution, Benefit, and Verification.
