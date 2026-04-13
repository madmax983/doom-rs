1. **Modify `crates/doom-tui/Cargo.toml`**:
   - Use `replace_with_git_merge_diff` to add `doom-types = { workspace = true }` under `[dependencies]`.
     ```diff
     <<<<<<< SEARCH
     [dependencies]
     doom-renderer = { workspace = true }
     ratatui       = { workspace = true }
     =======
     [dependencies]
     doom-types    = { workspace = true }
     doom-renderer = { workspace = true }
     ratatui       = { workspace = true }
     >>>>>>> REPLACE
     ```
   - Run `cargo check -p doom-tui` to validate syntax.

2. **Modify `crates/doom-tui/src/input.rs`**:
   - Use `replace_with_git_merge_diff` to add `impl From<TicInput> for doom_types::TicCmd` below the `TicInput` struct definition (lines 60-75).
     ```diff
     <<<<<<< SEARCH
         /// Wait in place for one turn (turn-based mode only).
         pub wait_pressed: bool,
     }

     /// Tracks which keys are currently held and produces `TicInput` on demand.
     =======
         /// Wait in place for one turn (turn-based mode only).
         pub wait_pressed: bool,
     }

     impl From<TicInput> for doom_types::TicCmd {
         fn from(input: TicInput) -> Self {
             Self {
                 forward_move: input.forward_move,
                 side_move: input.side_move,
                 angle_turn: input.angle_turn,
                 buttons: input.buttons,
                 chatchar: input.chatchar,
                 ..Default::default()
             }
         }
     }

     /// Tracks which keys are currently held and produces `TicInput` on demand.
     >>>>>>> REPLACE
     ```
   - Run `cargo check -p doom-tui` to validate syntax.

3. **Update `crates/doom-app/src/main.rs`**:
   - Use `replace_with_git_merge_diff` to replace `crate::net_mode::ticinput_to_ticcmd(input)` with `input.into()` at line 1152.
     ```diff
     <<<<<<< SEARCH
                 }
             }
         }

         let cmd = crate::net_mode::ticinput_to_ticcmd(input);

         // Pause the game simulation while the menu is open during gameplay.
     =======
                 }
             }
         }

         let cmd = input.into();

         // Pause the game simulation while the menu is open during gameplay.
     >>>>>>> REPLACE
     ```
   - Run `cargo check -p doom-app` to validate syntax.

4. **Update `crates/doom-app/src/demo_mode.rs`**:
   - Use `replace_with_git_merge_diff` to remove the import `use crate::net_mode::ticinput_to_ticcmd;` and replace the usage with `input.into()`.
     ```diff
     <<<<<<< SEARCH
     use doom_types::TicCmd;

     use crate::DoomGame;
     use crate::net_mode::ticinput_to_ticcmd;

     // ---------------------------------------------------------------------------
     // DemoRecordingWrapper
     =======
     use doom_types::TicCmd;

     use crate::DoomGame;

     // ---------------------------------------------------------------------------
     // DemoRecordingWrapper
     >>>>>>> REPLACE
     ```
     ```diff
     <<<<<<< SEARCH
             CompatibilityProfile::Extended | CompatibilityProfile::VanillaStrict => {
                 // Demo behavior is intentionally shared across profiles for now.
                 // The profile is still threaded here so the seam stays explicit.
                 let cmd = ticinput_to_ticcmd(input);
                 self.recorder.record_tic(&cmd);
                 self.inner.tick(input);
     =======
             CompatibilityProfile::Extended | CompatibilityProfile::VanillaStrict => {
                 // Demo behavior is intentionally shared across profiles for now.
                 // The profile is still threaded here so the seam stays explicit.
                 let cmd = input.into();
                 self.recorder.record_tic(&cmd);
                 self.inner.tick(input);
     >>>>>>> REPLACE
     ```
   - Run `cargo check -p doom-app` to validate syntax.

5. **Update `crates/doom-app/src/net_mode.rs`**:
   - Use `replace_with_git_merge_diff` to remove `ticinput_to_ticcmd` definition and its test, and replace its usages with `.into()`.
     ```diff
     <<<<<<< SEARCH
     /// Convert a [`TicInput`] (from doom-tui) to a [`TicCmd`] (for doom-game).
     ///
     /// Only the wire-compatible fields are copied; console/UI fields are dropped.
     pub(crate) fn ticinput_to_ticcmd(input: TicInput) -> TicCmd {
         TicCmd {
             forward_move: input.forward_move,
             side_move: input.side_move,
             angle_turn: input.angle_turn,
             buttons: input.buttons,
             chatchar: input.chatchar,
             ..Default::default()
         }
     }
     =======
     >>>>>>> REPLACE
     ```
     ```diff
     <<<<<<< SEARCH
         fn tick(&mut self, input: TicInput) {
             // Convert local input to wire format and send to the server.
             let wire_cmd = ticinput_to_ticcmd(input);
             let slot = self.client.player_slot();
             let mut cmds = [doom_types::TicCmd::default(); MAX_PLAYERS];
     =======
         fn tick(&mut self, input: TicInput) {
             // Convert local input to wire format and send to the server.
             let wire_cmd = input.into();
             let slot = self.client.player_slot();
             let mut cmds = [doom_types::TicCmd::default(); MAX_PLAYERS];
     >>>>>>> REPLACE
     ```
     ```diff
     <<<<<<< SEARCH
             let auth_cmd = if (slot as usize) < MAX_PLAYERS {
                 server_pkt.cmds[slot as usize]
             } else {
                 ticinput_to_ticcmd(input)
             };
     =======
             let auth_cmd = if (slot as usize) < MAX_PLAYERS {
                 server_pkt.cmds[slot as usize]
             } else {
                 input.into()
             };
     >>>>>>> REPLACE
     ```
     ```diff
     <<<<<<< SEARCH
         // -- Conversion helper tests (preserved from original) --

         #[test]
         fn ticinput_to_ticcmd_copies_all_fields() {
             let input = TicInput {
                 forward_move: 100,
                 side_move: -50,
                 angle_turn: 640,
                 buttons: 0x03,
                 chatchar: b'z',
                 ..Default::default()
             };
             let cmd = ticinput_to_ticcmd(input);
             assert_eq!(cmd.forward_move, 100);
             assert_eq!(cmd.side_move, -50);
             assert_eq!(cmd.angle_turn, 640);
             assert_eq!(cmd.buttons, 0x03);
             assert_eq!(cmd.chatchar, b'z');
         }
     =======
     >>>>>>> REPLACE
     ```
   - Run `cargo check -p doom-app` to validate syntax.

6. **Add a test to `crates/doom-tui/src/input.rs`**:
   - Use `replace_with_git_merge_diff` to add `fn ticinput_into_ticcmd_copies_all_fields()` inside the `mod tests` block right before `fn forward_key_sets_forward_move()`.
     ```diff
     <<<<<<< SEARCH
         #[test]
         fn forward_key_sets_forward_move() {
             let mut s = InputState::new();
     =======
         #[test]
         fn ticinput_into_ticcmd_copies_all_fields() {
             let input = TicInput {
                 forward_move: 100,
                 side_move: -50,
                 angle_turn: 640,
                 buttons: 0x03,
                 chatchar: b'z',
                 ..Default::default()
             };
             let cmd: doom_types::TicCmd = input.into();
             assert_eq!(cmd.forward_move, 100);
             assert_eq!(cmd.side_move, -50);
             assert_eq!(cmd.angle_turn, 640);
             assert_eq!(cmd.buttons, 0x03);
             assert_eq!(cmd.chatchar, b'z');
         }

         #[test]
         fn forward_key_sets_forward_move() {
             let mut s = InputState::new();
     >>>>>>> REPLACE
     ```
   - Run `cargo test -p doom-tui` to validate.

7. **Verify & Update Atlas Journal**:
   - Run `cargo test` and `cargo check` for the entire workspace to verify changes are successful and safe.
   - Use `run_in_bash_session` to append a journal entry summarizing the cleanup in `.jules/atlas.md`:
     ```sh
     cat << 'JEOF' >> .jules/atlas.md

     **[TicInput Conversion Boundary]**
     **Tangle:** `doom-app` contained a `pub(crate) fn ticinput_to_ticcmd` helper that manually converted `TicInput` (from `doom-tui`) to `TicCmd` (from `doom-types`), creating tight coupling where the app orchestrated the field mapping. This bypassed Rust's idiomatic type conversion boundaries.
     **Blueprint:** Added `doom-types` to `doom-tui` dependencies and implemented `From<TicInput> for TicCmd` directly on the input struct. Removed the helper from `doom-app` and changed all usages to `input.into()`, enforcing a cleaner dependency arrow where UI types natively know how to lower themselves to core types.
     JEOF
     ```

8. **Pre commit step**: Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
