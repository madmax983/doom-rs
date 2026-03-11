# Batch B Monsters Combat Spawn Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Status:** Complete on 2026-03-11 for the planned scope.

**Goal:** Fix the highest-confidence remaining Batch B gameplay parity gaps: monster wakeup and sight rules, Doom-shaped chase missile gating and retaliation, core spawn semantics, and the weapon-fire behavior fixes that fit the current combat boundary.

**Architecture:** Keep this inside `doom-game` and stay regression-first. Use the existing action/sight/spawn/combat modules instead of inventing a new AI layer. For weapons, fix the parts that belong in the current 2D hitscan model now, and leave full vertical bullet-slope parity explicitly tracked if the trace core still lacks the needed shape.

**Tech Stack:** Rust workspace crates `doom-game`, local unit tests with `cargo test`.

## Execution Result

- Landed sight parity for Doom's behind-the-back wakeup rule.
- Landed sound propagation parity for one `ML_SOUNDBLOCK`, including the mixed-topology regression update.
- Landed retaliation and Doom-shaped missile gating in `A_Chase`.
- Landed spawn parity for `MF_SPAWNCEILING`, randomized positive spawn tics, and blocked Nightmare respawn handling.
- Landed weapon parity for accurate first pistol/chaingun shots after release and fist snap-to-target.
- Verified with `cargo test -p doom-game weapon_fire -- --nocapture`, `cargo test -p doom-game tick_player_first_pistol_shot_is_accurate -- --nocapture`, `cargo fmt --all`, and `cargo test -p doom-game --lib`.

## Residual Debt

- Elevated-target autoaim and bullet-slope parity are still open because the current trace/hitscan path is still effectively 2D.
- Full held-fire/refire parity still overlaps the tic loop and frontend/audio behavior, so it should move with the later batch that owns those semantics.

---

### Task 1: Sight And Sound Wakeup Parity

**Files:**
- Modify: `crates/doom-game/src/sight.rs`
- Modify: `crates/doom-game/src/sound.rs`
- Modify: `crates/doom-game/src/actions.rs`
- Test: `crates/doom-game/src/sight.rs`
- Test: `crates/doom-game/src/sound.rs`

**Step 1: Write the failing tests**

- Add a regression where a monster facing away from the player with clear LOS does not acquire the player from behind unless the player is within melee range.
- Add a regression where sound crosses one `ML_SOUNDBLOCK` but not two consecutive `ML_SOUNDBLOCK` lines.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game look_for_players -- --nocapture
cargo test -p doom-game sound_blocked_by_two_consecutive_soundblock_linedefs -- --nocapture
```

Expected: failures because `p_look_for_players()` is still LOS-only and `p_noise_alert()` still starts with a two-block budget.

**Step 3: Write the minimal implementation**

- Teach `p_look_for_players()` the Doom behind-the-back restriction using the monster's facing angle and melee-range exemption.
- Change sound propagation to cross at most one `ML_SOUNDBLOCK`.
- Keep `A_Look` using the same wakeup boundary; do not fork a second wakeup rule.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game look_for_players -- --nocapture
cargo test -p doom-game sound -- --nocapture
```

Expected: sight and sound wakeup regressions pass.

### Task 2: A_Chase Missile Gating And Retaliation

**Files:**
- Modify: `crates/doom-game/src/actions.rs`
- Modify: `crates/doom-game/src/combat.rs`
- Test: `crates/doom-game/src/actions.rs`
- Test: `crates/doom-game/src/combat.rs`

**Step 1: Write the failing tests**

- Add a regression where damaging a monster sets up immediate retaliation so `A_Chase` can enter missile state even when the normal random missile gate would fail.
- Add a regression where far-away missile users do not fire every chance they get because the Doom-style distance/random gate rejects the shot.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game a_chase -- --nocapture
```

Expected: failures because `A_Chase` still uses the simplified `movecount <= 0 && LOS` missile gate and `MF_JUSTHIT` is unused.

**Step 3: Write the minimal implementation**

- Set `MF_JUSTHIT` and Doom-style alert threshold on monster damage when a live inflictor attacks.
- Add a `p_check_missile_range` helper in `actions.rs` that ports the current-needed Doom checks: LOS, melee suppression, `MF_JUSTHIT`, reaction time, coarse distance scaling, and the random gate.
- Route `A_Chase` missile decisions through that helper instead of the current simplified branch.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game a_chase -- --nocapture
```

Expected: retaliation and missile-gating regressions pass without regressing the existing chase tests.

### Task 3: Spawn Semantics

**Files:**
- Modify: `crates/doom-game/src/spawn.rs`
- Test: `crates/doom-game/src/spawn.rs`

**Step 1: Write the failing tests**

- Add a regression that `MF_SPAWNCEILING` things spawn hanging from the ceiling rather than snapping to the floor.
- Add a regression that non-Nightmare spawned map things randomize initial `tics` when the spawn state has positive duration.
- Add a regression that blocked Nightmare respawn leaves the corpse in place and does not create a fresh monster.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game spawn -- --nocapture
```

Expected: failures because spawn always floor-snaps, spawn tics are currently fixed, and Nightmare respawn does not test occupancy before spawning.

**Step 3: Write the minimal implementation**

- Make `sync_mobj_to_level()` honor `MF_SPAWNCEILING`.
- Randomize initial positive spawn `tics` for map things outside Nightmare difficulty using the game RNG.
- Add a small spawn-position occupancy check for Nightmare respawn using the existing level/mobj data before creating the fresh monster.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game spawn -- --nocapture
```

Expected: spawn regressions pass.

### Task 4: Weapon Behavior Within The Current Combat Boundary

**Files:**
- Modify: `crates/doom-game/src/weapon_fire.rs`
- Test: `crates/doom-game/src/weapon_fire.rs`

**Step 1: Write the failing tests**

- Add a regression that the first pistol or chaingun shot after attack release is accurate.
- Add a regression that fist attack snaps the player to the struck target the same way chainsaw already does.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game weapon_fire -- --nocapture
```

Expected: failures because pistol and chaingun always apply spread and fist does not turn toward the hit target.

**Step 3: Write the minimal implementation**

- Use the current `attack_down` state to suppress horizontal spread on the first pistol/chaingun shot after release.
- Reuse the existing chainsaw snap-to-target behavior for fist hits.
- Do not claim full vertical bullet-slope parity here; that remains a deeper combat/trace task if still needed after this batch.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game weapon_fire -- --nocapture
```

Expected: weapon behavior regressions pass.

### Task 5: Batch Verification

**Files:**
- Verify only

**Step 1: Run focused suites**

Run:
```powershell
cargo test -p doom-game look_for_players -- --nocapture
cargo test -p doom-game sound -- --nocapture
cargo test -p doom-game a_chase -- --nocapture
cargo test -p doom-game spawn -- --nocapture
cargo test -p doom-game weapon_fire -- --nocapture
```

**Step 2: Run the broader game suite**

Run:
```powershell
cargo fmt --all
cargo test -p doom-game --lib
```

Expected: focused regressions and the broader `doom-game` library suite stay green.
