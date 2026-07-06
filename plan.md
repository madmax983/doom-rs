1. **Update `DoomGame` struct definition in `crates/doom-app/src/main.rs`.**
   - Execute `replace_with_git_merge_diff` to replace `PaletteFlash` with `PaletteFlashState` in the `use` imports at the top of the file.
   ```
   <<<<<<< SEARCH
       IntermissionRenderer, PLAYER_HEIGHT, PaletteFlash, PaletteLut, PatchCache, RenderOut,
   =======
       IntermissionRenderer, PLAYER_HEIGHT, PaletteFlashState, PaletteLut, PatchCache, RenderOut,
   >>>>>>> REPLACE
   ```
2. **Update `DoomGame` struct initialization in `crates/doom-app/src/main.rs`.**
   - Execute `replace_with_git_merge_diff` to change `palette_flash: PaletteFlash` to `palette_flash: PaletteFlashState` in the struct definition.
   ```
   <<<<<<< SEARCH
       /// Palette flash controller (pain/pickup/rad-suit full-screen tints).
       palette_flash: PaletteFlash,
   =======
       /// Palette flash controller (pain/pickup/rad-suit full-screen tints).
       palette_flash: PaletteFlashState,
   >>>>>>> REPLACE
   ```
3. **Update `DoomGame` struct initialization in `make_doom_game`.**
   - Execute `replace_with_git_merge_diff` to initialize `palette_flash: PaletteFlashState::new()` instead of `PaletteFlash::new()` in `make_doom_game`.
   ```
   <<<<<<< SEARCH
               anim_state: AnimState::new(),
               palette_flash: PaletteFlash::new(),
               #[cfg(test)]
   =======
               anim_state: AnimState::new(),
               palette_flash: PaletteFlashState::new(),
               #[cfg(test)]
   >>>>>>> REPLACE
   ```
4. **Update game ticking logic in `crates/doom-app/src/main.rs`.**
   - Execute `replace_with_git_merge_diff` to replace the damage detection and `PaletteFlash` logic.
   ```
   <<<<<<< SEARCH
           // Advance animated texture state (flat and wall animations).
           self.anim_state.tick();

           // Advance palette flash timer (pain/pickup/rad-suit tints).
           self.palette_flash.tick();

           // Detect player damage and trigger a pain flash + hurt sound.
           {
               let cur_health = self.gs.player.health();
               if cur_health < self.prev_health {
                   let damage = self.prev_health - cur_health;
                   // Pain palette indices 1-8 (increasing red tint).
                   // Simple formula: one palette step per 8 HP lost, clamped.
                   let palette = ((damage / 8) as usize).clamp(1, 8);
                   self.palette_flash.trigger(palette, 12);
                   // Signal face FSM about damage.
                   self.face_state.on_damage(damage, Bam::ZERO);
                   // Play DSPLPAIN on any damage taken.
                   if let (Some(audio), Some(sfx_id)) = (&self.audio, self.pain_sfx_id) {
                       audio.play_sfx(
                           sfx_id,
                           SfxPriority::High,
                           1.0,
                           0.0,
                           Some(self.gs.player.handle),
                       );
                   }
                   if self.debug_log.is_some() {
                       let msg = format!("damage -{} health={}", damage, cur_health);
                       self.dlog(&msg);
                   }
               }
               // Tick face FSM every tic.
               {
                   let is_firing = self.gs.player.attack_down;
                   let is_invulnerable =
                       self.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] > 0;
                   self.face_state
                       .tick(cur_health, is_firing, is_invulnerable, None);
               }
   =======
           // Advance animated texture state (flat and wall animations).
           self.anim_state.tick();

           // Advance palette flash timer (pain/pickup/rad-suit tints).
           self.palette_flash.tick();

           // Synchronize state-based flashes.
           let rad_suit_tics = self.gs.player.powers[doom_game::player::powers::PW_IRONFEET];
           self.palette_flash.set_rad_suit(rad_suit_tics as i32);
           let berserk_tics = self.gs.player.powers[doom_game::player::powers::PW_STRENGTH];
           self.palette_flash.set_berserk(berserk_tics as i32);

           // If the player picked up an item this tic, trigger bonus flash.
           if self.gs.player.bonus_count > 0 {
               self.palette_flash.add_bonus();
               self.gs.player.bonus_count = 0; // Consume the event
           }

           // Detect player damage and trigger a pain flash + hurt sound.
           {
               let cur_health = self.gs.player.health();
               if cur_health < self.prev_health {
                   let damage = self.prev_health - cur_health;
                   self.palette_flash.add_pain(damage);

                   // Signal face FSM about damage.
                   self.face_state.on_damage(damage, Bam::ZERO);
                   // Play DSPLPAIN on any damage taken.
                   if let (Some(audio), Some(sfx_id)) = (&self.audio, self.pain_sfx_id) {
                       audio.play_sfx(
                           sfx_id,
                           SfxPriority::High,
                           1.0,
                           0.0,
                           Some(self.gs.player.handle),
                       );
                   }
                   if self.debug_log.is_some() {
                       let msg = format!("damage -{} health={}", damage, cur_health);
                       self.dlog(&msg);
                   }
               }
               // Tick face FSM every tic.
               {
                   let is_firing = self.gs.player.attack_down;
                   let is_invulnerable =
                       self.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] > 0;
                   self.face_state
                       .tick(cur_health, is_firing, is_invulnerable, None);
               }
   >>>>>>> REPLACE
   ```
5. **Update test 14 in `crates/doom-app/src/main.rs`.**
   - Execute `replace_with_git_merge_diff` to modify test 14 `doom_game_creates_with_palette_flash_initialized` to check `pain_count() == 0` instead of `remaining() == 0` and update assertions.
   ```
   <<<<<<< SEARCH
       // -----------------------------------------------------------------------
       // Test 14: DoomGame creates with PaletteFlash initialized
       // -----------------------------------------------------------------------

       #[test]
       fn doom_game_creates_with_palette_flash_initialized() {
           let game = make_doom_game();
           assert_eq!(
               game.palette_flash.active_palette(),
               0,
               "PaletteFlash must start at palette 0 (no flash)"
           );
           assert_eq!(
               game.palette_flash.remaining(),
               0,
               "PaletteFlash remaining must be 0 at creation"
           );
       }
   =======
       // -----------------------------------------------------------------------
       // Test 14: DoomGame creates with PaletteFlashState initialized
       // -----------------------------------------------------------------------

       #[test]
       fn doom_game_creates_with_palette_flash_initialized() {
           let game = make_doom_game();
           assert_eq!(
               game.palette_flash.active_palette(),
               0,
               "PaletteFlashState must start at palette 0 (no flash)"
           );
           assert_eq!(
               game.palette_flash.pain_count(),
               0,
               "PaletteFlashState pain_count must be 0 at creation"
           );
       }
   >>>>>>> REPLACE
   ```
6. **Update test 16 in `crates/doom-app/src/main.rs`.**
   - Execute `replace_with_git_merge_diff` to update test 16 `palette_flash_tick_integration` to use `add_pain(32)` instead of `trigger(4, 3)`, check that `pain_count() == 32`, tick 32 times (instead of 3), and assert `pain_count() == 0` and `active_palette() == 0`.
   ```
   <<<<<<< SEARCH
       // -----------------------------------------------------------------------
       // Test 16: PaletteFlash tick integration (trigger -> non-zero -> decays)
       // -----------------------------------------------------------------------

       #[test]
       fn palette_flash_tick_integration() {
           let mut game = make_doom_game();

           // Manually trigger a pain flash (palette 4, 3 tics duration).
           game.palette_flash.trigger(4, 3);
           assert_eq!(
               game.active_palette(),
               4,
               "active_palette must be 4 after trigger"
           );

           // Tick 3 times (palette_flash.tick is called inside game.tick).
           for _ in 0..3 {
               game.tick(TicInput::default());
           }

           // After 3 tics the flash should have expired back to 0.
           assert_eq!(
               game.active_palette(),
               0,
               "active_palette must return to 0 after flash duration expires"
           );
       }
   =======
       // -----------------------------------------------------------------------
       // Test 16: PaletteFlash tick integration (add_pain -> non-zero -> decays)
       // -----------------------------------------------------------------------

       #[test]
       fn palette_flash_tick_integration() {
           let mut game = make_doom_game();

           // Manually trigger a pain flash (32 damage maps to palette 4).
           game.palette_flash.add_pain(32);
           assert_eq!(
               game.active_palette(),
               4,
               "active_palette must be 4 after trigger"
           );

           // Tick 32 times (palette_flash.tick is called inside game.tick).
           for _ in 0..32 {
               game.tick(TicInput::default());
           }

           // After 32 tics the flash should have expired back to 0.
           assert_eq!(
               game.active_palette(),
               0,
               "active_palette must return to 0 after flash duration expires"
           );
       }
   >>>>>>> REPLACE
   ```
7. **Update test 19 in `crates/doom-app/src/main.rs`.**
   - Execute `replace_with_git_merge_diff` to update test 19 `pain_flash_triggers_on_health_decrease` to assert against `game.palette_flash.pain_count() > 0` instead of `remaining() > 0`.
   ```
   <<<<<<< SEARCH
           assert!(
               game.palette_flash.remaining() > 0,
               "pain flash should have remaining tics"
           );
       }
   =======
           assert!(
               game.palette_flash.pain_count() > 0,
               "pain flash should have remaining tics"
           );
       }
   >>>>>>> REPLACE
   ```
8. **Update test 22 in `crates/doom-app/src/main.rs`.**
   - Execute `replace_with_git_merge_diff` to update test 22 `pain_flash_palette_scales_with_damage`. We'll just change the assertions to match `PaletteFlashState` behavior. `add_pain` maps `pain_count * 8 / MAX_PAIN_COUNT` (MAX_PAIN_COUNT=64). 7 damage -> `7 * 8 / 64 = 0`, clamped to 1 -> palette 1. 80 damage -> capped at 64 -> `64 * 8 / 64 = 8` -> palette 8. So the logic stays exactly the same, we just update the comments/messages.
   ```
   <<<<<<< SEARCH
       // -----------------------------------------------------------------------
       // Test 22: Pain flash palette scales with damage amount
       // -----------------------------------------------------------------------

       #[test]
       fn pain_flash_palette_scales_with_damage() {
           // Small damage (7 HP): palette = max(7/8, 1) = 1
           {
               let mut game = make_doom_game();
               let handle = game.gs.player.handle;
               if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
                   mo.health = 93;
               }
               game.gs.player.set_health_capped(93, 100);
               game.tick(TicInput::default());
               assert_eq!(
                   game.palette_flash.active_palette(),
                   1,
                   "7 damage should give palette 1 (7/8=0, clamped to 1)"
               );
           }

           // Large damage (80 HP): palette = min(80/8, 8) = 8
           {
               let mut game = make_doom_game();
               let handle = game.gs.player.handle;
               if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
                   mo.health = 20;
               }
               game.gs.player.set_health_capped(20, 100);
               game.tick(TicInput::default());
               assert_eq!(
                   game.palette_flash.active_palette(),
                   8,
                   "80 damage should give palette 8 (80/8=10, clamped to 8)"
               );
           }
       }
   =======
       // -----------------------------------------------------------------------
       // Test 22: Pain flash palette scales with damage amount
       // -----------------------------------------------------------------------

       #[test]
       fn pain_flash_palette_scales_with_damage() {
           // Small damage (7 HP): palette = max(7*8/64, 1) = 1
           {
               let mut game = make_doom_game();
               let handle = game.gs.player.handle;
               if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
                   mo.health = 93;
               }
               game.gs.player.set_health_capped(93, 100);
               game.tick(TicInput::default());
               assert_eq!(
                   game.palette_flash.active_palette(),
                   1,
                   "7 damage should give palette 1 (7*8/64=0, clamped to 1)"
               );
           }

           // Large damage (80 HP): palette = min(80*8/64, 8) = 8
           {
               let mut game = make_doom_game();
               let handle = game.gs.player.handle;
               if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
                   mo.health = 20;
               }
               game.gs.player.set_health_capped(20, 100);
               game.tick(TicInput::default());
               assert_eq!(
                   game.palette_flash.active_palette(),
                   8,
                   "80 damage should give palette 8 (80*8/64=10, clamped to 8)"
               );
           }
       }
   >>>>>>> REPLACE
   ```
9. **Verify `doom-app` changes.**
   - Execute `cargo check -p doom-app` via `run_in_bash_session` to ensure the modifications were applied correctly before touching `doom-renderer`.
10. **Remove `PaletteFlash` struct from `crates/doom-renderer/src/palette_flash.rs`.**
   - Execute `replace_with_git_merge_diff` to delete `pub struct PaletteFlash` and its `impl` blocks.
   ```
   <<<<<<< SEARCH
   /// Palette flash state machine.
   ///
   /// Tracks which palette tint is active and how many tics remain before
   /// reverting to the default (index 0) palette.
   #[derive(Clone, Debug)]
   pub struct PaletteFlash {
       /// Currently active palette index (0 = normal).
       palette_index: usize,
       /// Remaining tics before the flash expires and palette returns to 0.
       remaining_tics: u32,
   }

   impl PaletteFlash {
       /// Create a new flash controller starting at palette 0 (no flash).
       pub fn new() -> Self {
           Self {
               palette_index: 0,
               remaining_tics: 0,
           }
       }

       /// Trigger a palette flash.
       ///
       /// - `palette_index` — which PLAYPAL palette to use (1-13 for effects;
       ///   0 effectively cancels any active flash).
       /// - `duration` — how many tics the flash should last before reverting
       ///   to palette 0.
       ///
       /// A new trigger *always* overrides any ongoing flash.
       pub fn trigger(&mut self, palette_index: usize, duration: u32) {
           self.palette_index = palette_index.min(MAX_PALETTE_INDEX);
           self.remaining_tics = duration;
       }

       /// Advance the flash by one tic.
       ///
       /// Decrements the remaining duration. When it reaches zero, the palette
       /// index is reset to 0 (normal).
       pub fn tick(&mut self) {
           if self.remaining_tics > 0 {
               self.remaining_tics -= 1;
               if self.remaining_tics == 0 {
                   self.palette_index = 0;
               }
           }
       }

       /// Return the currently active palette index.
       ///
       /// Callers use this to select which PLAYPAL palette to blit with.
       pub fn active_palette(&self) -> usize {
           self.palette_index
       }

       /// Return the remaining duration in tics.
       pub fn remaining(&self) -> u32 {
           self.remaining_tics
       }
   }

   impl Default for PaletteFlash {
       fn default() -> Self {
           Self::new()
       }
   }

   // ---------------------------------------------------------------------------
   // PaletteFlashState — enhanced multi-source flash manager
   // ---------------------------------------------------------------------------
   =======
   // ---------------------------------------------------------------------------
   // PaletteFlashState — enhanced multi-source flash manager
   // ---------------------------------------------------------------------------
   >>>>>>> REPLACE
   ```
11. **Remove `PaletteFlash` tests from `crates/doom-renderer/src/palette_flash.rs`.**
    - Execute `replace_with_git_merge_diff` to delete all `PaletteFlash` tests.
    ```
    <<<<<<< SEARCH
       // --- PaletteFlash tests (existing) ---

       #[test]
       fn starts_at_palette_zero() {
           let flash = PaletteFlash::new();
           assert_eq!(flash.active_palette(), 0);
           assert_eq!(flash.remaining(), 0);
       }

       #[test]
       fn trigger_sets_palette_and_duration() {
           let mut flash = PaletteFlash::new();
           flash.trigger(3, 10);
           assert_eq!(flash.active_palette(), 3);
           assert_eq!(flash.remaining(), 10);
       }

       #[test]
       fn tick_decrements_duration() {
           let mut flash = PaletteFlash::new();
           flash.trigger(5, 3);
           flash.tick();
           assert_eq!(flash.remaining(), 2);
           assert_eq!(flash.active_palette(), 5);
       }

       #[test]
       fn returns_to_palette_zero_when_duration_expires() {
           let mut flash = PaletteFlash::new();
           flash.trigger(8, 2);

           flash.tick(); // remaining = 1, still active
           assert_eq!(flash.active_palette(), 8);

           flash.tick(); // remaining = 0, reset to palette 0
           assert_eq!(flash.active_palette(), 0);
           assert_eq!(flash.remaining(), 0);
       }

       #[test]
       fn active_palette_returns_current_index() {
           let mut flash = PaletteFlash::new();
           assert_eq!(flash.active_palette(), 0);

           flash.trigger(12, 5);
           assert_eq!(flash.active_palette(), 12);

           // Tick down partially — still active.
           flash.tick();
           flash.tick();
           assert_eq!(flash.active_palette(), 12);
       }

       #[test]
       fn new_trigger_overrides_old_one() {
           let mut flash = PaletteFlash::new();
           flash.trigger(3, 100);
           assert_eq!(flash.active_palette(), 3);
           assert_eq!(flash.remaining(), 100);

           // Override with a new flash.
           flash.trigger(9, 5);
           assert_eq!(flash.active_palette(), 9);
           assert_eq!(flash.remaining(), 5);
       }

       #[test]
       fn tick_is_noop_when_no_flash_active() {
           let mut flash = PaletteFlash::new();
           flash.tick(); // Should not panic or underflow.
           assert_eq!(flash.active_palette(), 0);
           assert_eq!(flash.remaining(), 0);
       }

       #[test]
       fn trigger_clamps_palette_index_to_max() {
           let mut flash = PaletteFlash::new();
           flash.trigger(999, 10);
           assert_eq!(flash.active_palette(), MAX_PALETTE_INDEX);
       }

       #[test]
       fn trigger_with_zero_duration_resets_immediately() {
           let mut flash = PaletteFlash::new();
           flash.trigger(5, 0);
           // Duration is 0, so after next tick it should be at 0.
           // But even before tick, palette_index is 5 with duration 0.
           // Next tick won't decrement past 0.
           assert_eq!(flash.active_palette(), 5);
           assert_eq!(flash.remaining(), 0);
           flash.tick();
           // Tick with remaining=0 is a noop — palette stays as-is.
           assert_eq!(flash.active_palette(), 5);
       }

       #[test]
       fn full_pain_flash_lifecycle() {
           let mut flash = PaletteFlash::new();

           // Take damage: pain flash (palette 4, red tint) for 10 tics.
           flash.trigger(4, 10);
           for _ in 0..9 {
               assert_eq!(flash.active_palette(), 4);
               flash.tick();
           }
           // 10th tick → expires.
           assert_eq!(flash.active_palette(), 4);
           flash.tick();
           assert_eq!(flash.active_palette(), 0);
       }

       #[test]
       fn default_matches_new() {
           let default_flash = PaletteFlash::default();
           let new_flash = PaletteFlash::new();
           assert_eq!(default_flash.active_palette(), new_flash.active_palette());
           assert_eq!(default_flash.remaining(), new_flash.remaining());
       }

       // --- PaletteFlashState tests ---
    =======
       // --- PaletteFlashState tests ---
    >>>>>>> REPLACE
    ```
12. **Remove `PaletteFlash` from `crates/doom-renderer/src/lib.rs`.**
    - Execute `replace_with_git_merge_diff` to remove `PaletteFlash` from `pub use palette_flash::{PaletteFlash, PaletteFlashState};` in `crates/doom-renderer/src/lib.rs`.
    ```
    <<<<<<< SEARCH
    pub use palette::{PaletteLut, Rgb};
    pub use palette_flash::{PaletteFlash, PaletteFlashState};
    pub use patch_cache::PatchCache;
    =======
    pub use palette::{PaletteLut, Rgb};
    pub use palette_flash::PaletteFlashState;
    pub use patch_cache::PatchCache;
    >>>>>>> REPLACE
    ```
13. **Update module docs in `crates/doom-renderer/src/palette_flash.rs`.**
    - Execute `replace_with_git_merge_diff` to remove mentions of `PaletteFlash`.
    ```
    <<<<<<< SEARCH
    //! | 9-12          | Pickup flash (gold/yellow tint)         |
    //! | 13            | Radiation suit (green tint)             |
    //!
    //! `PaletteFlash` tracks the active palette index and a duration in tics.
    //! Each call to `tick()` decrements the remaining duration; when it hits
    //! zero the palette snaps back to index 0.  A new `trigger()` call
    //! overrides any ongoing flash.
    =======
    //! | 9-12          | Pickup flash (gold/yellow tint)         |
    //! | 13            | Radiation suit (green tint)             |
    >>>>>>> REPLACE
    ```
14. **Verify the structural changes.**
    - Execute `cargo check --all-targets --all-features`, `cargo test --all-targets --all-features`, and `cargo clippy --all-targets --all-features -- -D warnings` via `run_in_bash_session` to ensure all tests pass and no warnings exist.
15. **Document Architectural Change.**
    - Read `.jules/atlas.md` with `cat`. Append the structural change using `cat << 'EOF' >> .jules/atlas.md`.
    ```
    **[Unify PaletteFlash Tracking]**
    **Tangle:** The codebase had two separate structures for palette flashing: `PaletteFlash` (a simple timer-based trigger) and `PaletteFlashState` (a robust multi-source priority tracker). `doom-app` was using the simpler `PaletteFlash`, requiring manual calculation of damage-to-palette conversions and hardcoded overrides, leading to poor encapsulation and redundant types.
    **Blueprint:** Removed `PaletteFlash` entirely and migrated `doom-app` to use `PaletteFlashState`. The game loop now correctly delegates state tracking (pain, bonus, radsuit, berserk) to `PaletteFlashState`, which handles priority natively. This simplifies the top-level loop and enforces the single-responsibility principle.
    ```
16. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
