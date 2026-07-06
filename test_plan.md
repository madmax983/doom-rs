6. **Remove `PaletteFlash` tests from `crates/doom-renderer/src/palette_flash.rs`.**
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
7. **Remove `PaletteFlash` from `crates/doom-renderer/src/lib.rs`.**
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
8. **Update module docs in `crates/doom-renderer/src/palette_flash.rs`.**
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
9. **Verify the structural changes.**
    - Execute `cargo check --all-targets --all-features`, `cargo test --all-targets --all-features`, and `cargo clippy --all-targets --all-features -- -D warnings` via `run_in_bash_session` to ensure all tests pass and no warnings exist.
10. **Document Architectural Change.**
    - Execute `run_in_bash_session` to read `.jules/atlas.md` and append the structural change using `cat << 'EOF' >> .jules/atlas.md`:
    ```
    **[Unify PaletteFlash Tracking]**
    **Tangle:** The codebase had two separate structures for palette flashing: `PaletteFlash` (a simple timer-based trigger) and `PaletteFlashState` (a robust multi-source priority tracker). `doom-app` was using the simpler `PaletteFlash`, requiring manual calculation of damage-to-palette conversions and hardcoded overrides, leading to poor encapsulation and redundant types.
    **Blueprint:** Removed `PaletteFlash` entirely and migrated `doom-app` to use `PaletteFlashState`. The game loop now correctly delegates state tracking (pain, bonus, radsuit, berserk) to `PaletteFlashState`, which handles priority natively. This simplifies the top-level loop and enforces the single-responsibility principle.
    ```
11. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
