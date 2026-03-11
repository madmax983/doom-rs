# Batch D Frontend Audio Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Status:** Complete on 2026-03-11 for the planned scope.

**Goal:** Fix the highest-confidence remaining Batch D parity gaps: held-fire semantics in the tic loop, weapon SFX being keyed to actual shots instead of button edges, music re-resolution from current level state, and title-screen demo phases that currently exist without playback.

**Architecture:** Keep deterministic gameplay behavior in `doom-game`. The frontend should stop inferring weapon fire from raw input edges and instead consume explicit game-generated sound events. For music, move from startup-captured bytes to level-name-based resolution from app-owned music data. Do not claim full mixer/channel parity or real attract-mode demo playback in this batch.

**Tech Stack:** Rust workspace crates `doom-game`, `doom-app`, and existing unit tests with `cargo test`.

---

## Execution Result

- Landed deterministic held-fire cadence in `doom-game` with a per-player attack cooldown, including hold-refire for pistol and shotgun and release-gated rocket launcher/BFG behavior.
- Landed actual fire-driven player weapon sound events through `SoundRequest::PlayerWeaponFire`, and removed the old attack-edge SFX guesswork from `doom-app`.
- Landed map-music re-resolution from current `GameState.level_name` using an app-owned music library, with restart hooks on gameplay entry and load paths.
- Landed title-loop sanity: timer-driven attract mode now stays in `Title <-> Credits` until real demo playback exists, while manual `Demo(_)` phases remain representable and renderer-safe.
- Verified with `cargo fmt --all`, `cargo test -p doom-game --lib`, `cargo test -p doom-renderer --lib`, and `cargo test -p doom-app`.

## Residual Debt

- Sound origin/channel reuse is still far simpler than Doom's full `S_StartSound` behavior.
- Weapon cadence is now deterministic and closer to Doom, but it still uses a simplified cooldown model instead of a full psprite state machine.
- Real attract-mode demo playback is still deferred; this batch only removed the fake timer-driven demo walk.

---

### Task 1: Tic-Loop Held-Fire Cadence

**Files:**
- Modify: `crates/doom-game/src/player.rs`
- Modify: `crates/doom-game/src/tic.rs`
- Modify: `crates/doom-game/src/savegame.rs`
- Modify: `crates/doom-app/src/savegame.rs`
- Test: `crates/doom-game/src/tic.rs`
- Test: `crates/doom-game/src/savegame.rs`

**Step 1: Write the failing tests**

- Add regressions that held pistol and held shotgun keep firing after their cooldown expires without releasing `BT_ATTACK`.
- Add a regression that held rocket launcher does not auto-refire just because the button stays held.
- Add save/load regressions if a new player cooldown field becomes part of deterministic simulation state.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game tick_player -- --nocapture
```

Expected: failures because `tick_player()` still treats most weapons as edge-trigger only and the current player state has no real refire cadence.

**Step 3: Write the minimal implementation**

- Add a per-player attack/refire cooldown owned by `doom-game`.
- Decrement cooldown once per tic and let held attack refire only when the current weapon allows auto-fire and its cooldown has elapsed.
- Keep BFG and rocket launcher out of hold-to-refire.
- Persist any new deterministic player field through the existing save/load paths that serialize `PlayerState`.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game tick_player -- --nocapture
```

Expected: held-fire regressions pass and non-auto weapons do not degenerate into per-tic spam.

### Task 2: Actual Weapon-Fire Sound Events

**Files:**
- Modify: `crates/doom-game/src/state.rs`
- Modify: `crates/doom-game/src/weapon_fire.rs`
- Modify: `crates/doom-app/src/audio_system.rs`
- Modify: `crates/doom-app/src/main.rs`
- Test: `crates/doom-game/src/weapon_fire.rs`
- Test: `crates/doom-app/src/audio_system.rs`

**Step 1: Write the failing tests**

- Add regressions that actual player weapon fire queues a player-weapon sound event in `sound_queue`.
- Add a regression that the audio mapping resolves those new events to the expected DS* lumps.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game weapon_fire -- --nocapture
cargo test -p doom-app audio_system -- --nocapture
```

Expected: failures because weapon sounds are still inferred from button edges in `doom-app` instead of being emitted by the game.

**Step 3: Write the minimal implementation**

- Add a `SoundRequest` variant for player weapon fire.
- Emit that event only when a weapon actually fires, not when attack is merely pressed.
- Remove the button-edge weapon SFX path from `doom-app` and route those sounds through the same sound-event drain as the monster/player sounds.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game weapon_fire -- --nocapture
cargo test -p doom-app audio_system -- --nocapture
```

Expected: player weapon sounds now track real fire cadence, including held-fire repeats.

### Task 3: Level Music Re-Resolution

**Files:**
- Modify: `crates/doom-app/src/main.rs`
- Modify: `crates/doom-app/src/audio_system.rs`
- Test: `crates/doom-app/src/main.rs`

**Step 1: Write the failing tests**

- Add a regression that `start_level_music()` resolves music from the current `GameState.level_name`, not only the startup-captured bytes.
- Add a regression that loading or otherwise changing the current level state can restart music using the re-resolved map name.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-app starts_level_music -- --nocapture
```

Expected: failures because `DoomGame` still stores one `level_music` blob chosen at startup.

**Step 3: Write the minimal implementation**

- Replace the startup-only `level_music` bytes with app-owned map music data that can be resolved by level name on demand.
- Make `start_level_music()` resolve through `music_lump_for_map(self.gs.level_name.as_str())`.
- Restart map music at the existing gameplay entry points that should re-enter the level soundtrack.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-app starts_level_music -- --nocapture
```

Expected: map music startup follows current level state instead of stale startup data.

### Task 4: Title Screen Demo Sanity

**Files:**
- Modify: `crates/doom-game/src/menu.rs`
- Modify: `crates/doom-renderer/src/menu_render.rs`
- Test: `crates/doom-game/src/menu.rs`
- Test: `crates/doom-renderer/src/menu_render.rs`

**Step 1: Write the failing tests**

- Add regressions that the normal timer-driven title loop does not enter `Demo(_)` phases when no playback path is wired.
- Keep a regression that manual `Demo(_)` phases still draw as a no-op until real playback exists.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game title_screen -- --nocapture
cargo test -p doom-renderer demo_phase -- --nocapture
```

Expected: failures because `TitleScreen::tick()` still walks into fake demo phases.

**Step 3: Write the minimal implementation**

- Keep `TitlePhase::Demo(_)` representable for future real playback.
- Change the automatic title/credits attract loop to stay out of demo phases until playback exists.
- Update comments and tests so the code reflects the current reality instead of the fake cycle.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game title_screen -- --nocapture
cargo test -p doom-renderer demo_phase -- --nocapture
```

Expected: timer-driven title behavior stops wandering into a blank “demo” phase.

### Task 5: Batch Verification

**Files:**
- Verify only

**Step 1: Run focused suites**

Run:
```powershell
cargo test -p doom-game tick_player -- --nocapture
cargo test -p doom-game weapon_fire -- --nocapture
cargo test -p doom-app starts_level_music -- --nocapture
cargo test -p doom-game title_screen -- --nocapture
cargo test -p doom-renderer demo_phase -- --nocapture
```

**Step 2: Run broader verification**

Run:
```powershell
cargo fmt --all
cargo test -p doom-game --lib
cargo test -p doom-app
cargo test -p doom-renderer --lib
```

Expected: Batch D regressions and the broader impacted suites stay green.

## Explicitly Deferred

- Full `S_StartSound`-style channel reuse and source-aware stealing parity.
- True attract-mode demo playback during the title loop.
- Elevated-target bullet slope and other deeper combat parity still tracked outside Batch D.
