# Batch A Gameplay Interactions Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Status:** Completed on 2026-03-11.

**Outcome:** Batch A landed green. Front-side-only use semantics, blocked-use feedback, and exact monster door activation on the true blocking linedef are all implemented and covered by regression tests.

**Goal:** Restore Doom-shaped player use and door interaction semantics for Batch A: front-side-only use activation, blocked-use feedback, and exact door activation for monsters blocked by a real door linedef.

**Architecture:** Keep the changes narrow. Use the existing `p_use_lines()` intercept ordering, thread actual side information into linedef dispatch, emit blocked-use feedback through `GameState::sound_queue`, and replace monster door-opening's coarse blockmap scan with an exact blocking-linedef result from movement. Regressions should live next to the behavior they lock down.

**Tech Stack:** Rust workspace crates `doom-game` and `doom-app`, unit tests with `cargo test`.

---

### Task 1: Front-Side Use Semantics

**Files:**
- Modify: `crates/doom-game/src/specials.rs`
- Modify: `crates/doom-game/src/linedef_dispatch.rs`
- Test: `crates/doom-game/src/specials.rs`
- Test: `crates/doom-game/src/linedef_dispatch.rs`

**Step 1: Write the failing tests**

- Add a `p_use_lines` regression showing that a usable linedef hit from the back side does not activate.
- Add a dispatch-level regression showing that switch-style use activation respects `from_side` for front-only lines.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game p_use_lines_back_side -- --nocapture
cargo test -p doom-game dispatch_linedef_from_side -- --nocapture
```

Expected: failure because `dispatch_linedef()` currently ignores `_from_side` and `p_use_lines()` hardcodes `0`.

**Step 3: Write the minimal implementation**

- Compute the side hit by the use trace in `p_use_lines()`.
- Pass that side into `dispatch_linedef()`.
- Make `dispatch_linedef()` reject backside use activation for the appropriate switch/use-trigger paths.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game p_use_lines -- --nocapture
cargo test -p doom-game dispatch_linedef -- --nocapture
```

Expected: all Batch A use/dispatch tests pass.

**Result:** Passed. Added `p_use_lines_back_side_does_not_activate_front_only_special`, `dispatch_back_side_blocks_non_manual_use_line`, and `dispatch_back_side_allows_manual_door_line`.

### Task 2: Blocked-Use Feedback

**Files:**
- Modify: `crates/doom-game/src/state.rs`
- Modify: `crates/doom-game/src/specials.rs`
- Modify: `crates/doom-app/src/main.rs`
- Test: `crates/doom-game/src/specials.rs`

**Step 1: Write the failing test**

- Add a regression showing that pressing use into a closed ordinary blocker queues a blocked-use sound request exactly once for that tic.

**Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p doom-game blocked_use -- --nocapture
```

Expected: failure because no blocked-use sound request exists yet.

**Step 3: Write the minimal implementation**

- Add a `SoundRequest` variant for blocked player use.
- Queue it from `p_use_lines()` when a blocking line stops use without activation.
- Drain and map it in `doom-app` to the Doom blocked-use SFX lump when present.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game blocked_use -- --nocapture
```

Expected: blocked-use regression passes.

**Result:** Passed. The closed-wall blocker regression now also asserts a single queued `PlayerUseFail`, and `doom-app` maps it to `DSNOWAY`.

### Task 3: Exact Monster Door Blocking Line

**Files:**
- Modify: `crates/doom-game/src/movement.rs`
- Modify: `crates/doom-game/src/actions.rs`
- Modify: `crates/doom-game/src/specials.rs`
- Test: `crates/doom-game/src/actions.rs`

**Step 1: Write the failing test**

- Add a regression where the monster is blocked by one specific door linedef while another door special is nearby; only the true blocking door should open.

**Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p doom-game monster_blocked_by_door -- --nocapture
```

Expected: failure because current logic scans a blockmap cell and may open the wrong linedef.

**Step 3: Write the minimal implementation**

- Extend movement collision evaluation to expose the exact blocking linedef for the attempted move.
- Update monster chase movement to use that exact linedef when trying to open doors.
- Keep the old `p_try_move()` behavior intact for unrelated callers by layering a more detailed helper under it.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game monster_blocked_by_door -- --nocapture
cargo test -p doom-game a_chase -- --nocapture
```

Expected: the new blocker-specific regression passes and existing chase tests stay green.

**Result:** Passed. `p_move_opens_the_actual_blocking_door` now locks down the exact-blocker path.

### Task 4: Batch Verification

**Files:**
- Verify only

**Step 1: Run focused suites**

Run:
```powershell
cargo test -p doom-game p_use_lines -- --nocapture
cargo test -p doom-game dispatch_linedef -- --nocapture
cargo test -p doom-game blocked_use -- --nocapture
cargo test -p doom-game monster_activate_door_linedef -- --nocapture
cargo test -p doom-game a_chase -- --nocapture
```

**Step 2: Run broader gameplay suite**

Run:
```powershell
cargo test -p doom-game --lib
```

Expected: full `doom-game` library suite remains green.

**Actual verification run:**
```powershell
cargo fmt --all
cargo test -p doom-game dispatch_back_side -- --nocapture
cargo test -p doom-game p_use_lines_back_side -- --nocapture
cargo test -p doom-game p_use_lines_closed_nonspecial_wall_blocks_special_behind_it -- --nocapture
cargo test -p doom-game p_move_opens_the_actual_blocking_door -- --nocapture
cargo test -p doom-app sound_request_sfx_maps_player_use_fail_to_noway -- --nocapture
cargo test -p doom-game --lib
cargo test -p doom-app
```

All commands passed.
