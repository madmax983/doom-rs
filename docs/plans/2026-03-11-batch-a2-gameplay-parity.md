# Batch A2 Gameplay Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Status:** Completed on 2026-03-11.

**Outcome:** Batch A2 landed green. Locked doors now emit Doom-style player-only `sfx_oof` feedback with the classic keyed-door HUD message, and walk-trigger processing moved back into the game tick with deterministic reverse-crossing ordering instead of app-wrapper lump-order accidents.

**Goal:** Close the remaining gameplay interaction parity gaps from System 1: locked-door fail semantics and walk-trigger sequencing.

**Architecture:** Keep the fixes source-shaped. Locked doors should emit Doom-style player-only fail feedback (`sfx_oof` plus the classic key message) without changing non-player behavior. Walk-trigger processing should move back into the game tick and use a deterministic crossed-line order that matches the simplified port's nearest approximation to vanilla `spechit` processing instead of raw linedef order in the app wrapper.

**Tech Stack:** Rust workspace crates `doom-game` and `doom-app`, unit tests with `cargo test`.

---

### Task 1: Locked Door Fail Feedback

**Files:**
- Modify: `crates/doom-game/src/state.rs`
- Modify: `crates/doom-game/src/linedef_dispatch.rs`
- Modify: `crates/doom-app/src/audio_system.rs`
- Modify: `crates/doom-app/src/main.rs`
- Test: `crates/doom-game/src/linedef_dispatch.rs`
- Test: `crates/doom-app/src/audio_system.rs`
- Test: `crates/doom-app/src/main.rs`

**Step 1: Write the failing tests**

- Add a dispatch regression showing that a player using a blue locked door without the key queues Doom-style locked-door feedback instead of failing silently.
- Add a dispatch regression showing that non-player activators still fail silently on locked doors.
- Add app-side regressions for the locked-door SFX mapping and the top-of-screen message text.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game dispatch_locked_blue_door_without_key_queues_player_feedback -- --nocapture
cargo test -p doom-game dispatch_locked_door_without_player_feedback_for_monsters -- --nocapture
cargo test -p doom-app locked_door -- --nocapture
```

Expected: failure because the current dispatcher just returns `false` for missing keys and the app has no keyed-door feedback path.

**Step 3: Write the minimal implementation**

- Add a color-specific locked-door feedback event to `SoundRequest`.
- Queue it only when the activator is the player and the required key color is missing.
- Map that event to `DSOOF` in `doom-app` and surface the classic Doom keyed-door text in the HUD overlay.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game dispatch_locked -- --nocapture
cargo test -p doom-app locked_door -- --nocapture
```

Expected: all new locked-door feedback regressions pass.

**Result:** Passed. Added `PlayerUseLockedDoor(LockedDoorColor)` in `doom-game`, mapped it to `DSOOF` in `doom-app`, and surfaced the classic keyed-door message through the existing top-of-screen HUD overlay path.

### Task 2: Walk Trigger Sequencing

**Files:**
- Modify: `crates/doom-game/src/linedef_dispatch.rs`
- Modify: `crates/doom-game/src/tic.rs`
- Modify: `crates/doom-app/src/main.rs`
- Test: `crates/doom-game/src/linedef_dispatch.rs`
- Test: `crates/doom-game/src/tic.rs`

**Step 1: Write the failing tests**

- Add a `check_cross_lines()` regression where two crossed walk exits are laid out in one order geometrically and the opposite order in the linedef lump; the result should follow the crossed-line order, not raw linedef order.
- Add a `GameState::tick()` regression showing that crossing a walk trigger during player movement sets `exit_request` before the tick returns.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game cross_lines_reverse_crossing_order_matches_vanilla_spechit_processing -- --nocapture
cargo test -p doom-game tick_sets_exit_request_when_player_crosses_walk_line -- --nocapture
```

Expected: failure because `check_cross_lines()` currently iterates walk lines in raw linedef order and `doom-app` still performs crossed-line processing after the game tick.

**Step 3: Write the minimal implementation**

- Update `check_cross_lines()` to compute crossed-line fractions, sort them by crossing distance, and dispatch in reverse crossing order to match the simplified vanilla `spechit` model.
- Move player crossed-line processing into `p_move_player()` / `tick_player()` and remove the duplicate app-level pass.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game cross_lines -- --nocapture
cargo test -p doom-game tick_sets_exit_request_when_player_crosses_walk_line -- --nocapture
```

Expected: the ordering and same-tick regressions pass together.

**Result:** Passed. `check_cross_lines()` now sorts by crossing fraction and dispatches in reverse path order to mirror the simplified vanilla `spechit` model, and player crossed-line activation now happens inside `p_move_player()` during the game tick.

### Task 3: Batch Verification

**Files:**
- Verify only

**Step 1: Run focused suites**

Run:
```powershell
cargo test -p doom-game dispatch_locked -- --nocapture
cargo test -p doom-game cross_lines -- --nocapture
cargo test -p doom-game tick_sets_exit_request_when_player_crosses_walk_line -- --nocapture
cargo test -p doom-app locked_door -- --nocapture
```

**Step 2: Run broader suites**

Run:
```powershell
cargo test -p doom-game --lib
cargo test -p doom-app
```

Expected: gameplay and app suites remain green after the A2 parity fixes.

**Actual verification run:**
```powershell
cargo fmt --all
cargo test -p doom-game dispatch_locked_blue_door_without_key_queues_player_feedback -- --nocapture
cargo test -p doom-game cross_lines_reverse_crossing_order_matches_vanilla_spechit_processing -- --nocapture
cargo test -p doom-game tick_sets_exit_request_when_player_crosses_walk_line -- --nocapture
cargo test -p doom-app sound_request_sfx_maps_locked_door_feedback_to_oof -- --nocapture
cargo test -p doom-app locked_door_feedback_sets_overlay_message -- --nocapture
cargo test -p doom-game --lib
cargo test -p doom-app
```

All commands passed.
