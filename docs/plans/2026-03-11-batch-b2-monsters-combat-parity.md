# Batch B2 Monsters Combat Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Status:** In progress on 2026-03-11.

**Outcome so far:** Three B2 slices are landed and green. `p_check_missile_range()` now matches the vanilla Arch-Vile maximum-range rule and the Revenant minimum-range rule, hitscan now uses Doom-shaped vertical slope clipping instead of the old 2D-only actor pick, and player bullet weapons now probe center/right/left before firing so near-off-center targets are no longer ignored.

**Goal:** Finish the remaining monster/combat parity debt from System 2, starting with source-backed `P_CheckMissileRange` fixes, then the vertical hitscan path, and then the remaining `P_BulletSlope` / refire nuances.

**Architecture:** Keep monster missile gating and hitscan parity separate. Missile-range fixes belong in `actions.rs` with direct unit coverage. The hitscan pass now uses an ordered intercept walk in `combat.rs` that clips a Doom-style vertical slope window across lines and actors. Player bullet autoaim now probes the Chocolate Doom center/right/left order in `weapon_fire.rs`, but within the current exact-ray port that probe is realized as choosing the firing angle rather than as a source-identical slope-only pass.

**Tech Stack:** Rust workspace crate `doom-game`, unit tests with `cargo test`.

---

### Task 1: `P_CheckMissileRange` Monster Edge Cases

**Files:**
- Modify: `crates/doom-game/src/actions.rs`
- Test: `crates/doom-game/src/actions.rs`

**Step 1: Write the failing tests**

- Add an Arch-Vile regression showing that far targets beyond `14 * 64` units must not pass missile range.
- Add a Revenant regression showing that targets under `196` units must not pass missile range, even when the random gate would otherwise allow it.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-game p_check_missile_range_archvile_rejects_far_targets -- --nocapture
cargo test -p doom-game p_check_missile_range_revenant_rejects_targets_under_196_units -- --nocapture
```

Expected: fail against the flattened local monster grouping.

**Step 3: Write the minimal implementation**

- Restore the vanilla `-64` distance adjustment before monster-specific handling.
- Make Arch-Viles return `false` when the target is beyond `14 * 64`.
- Make Revenants reject targets under `196` units, then halve distance like vanilla.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-game p_check_missile_range_archvile_rejects_far_targets -- --nocapture
cargo test -p doom-game p_check_missile_range_revenant_rejects_targets_under_196_units -- --nocapture
cargo test -p doom-game a_chase -- --nocapture
```

Expected: the new regressions and surrounding chase tests pass.

**Result:** Passed. The new regressions are green and `cargo test -p doom-game --lib` stays green after the gate fix.

### Task 2: Elevated-Target Hitscan Autoaim

**Files:**
- Modify: `crates/doom-game/src/combat.rs`
- Modify: `crates/doom-game/src/weapon_fire.rs`
- Modify: `crates/doom-game/src/sight.rs`
- Possibly modify: `crates/doom-game/src/trace.rs`
- Test: `crates/doom-game/src/combat.rs`

**Step 1: Audit before coding**

- Compare local `p_line_attack()` and `p_aim_line_slope()` against Chocolate Doom `P_AimLineAttack` / `P_LineAttack`.
- Identify the minimum viable path to add vertical slope-aware hitscan without reopening the earlier 2D hit detection fixes.

**Initial finding:**

- Local hitscan was still effectively 2D. `p_line_attack()` in `combat.rs` did not use slope at all, and `trace::trace_ray()` actor tests ignored actor `z`.
- `p_aim_line_slope()` exists, but it is not currently part of hitscan selection or occlusion.

**Red regressions written:**

- A target entirely below the autoaim window must not be hit.
- A low near target must be skipped so a farther target in the actual autoaim lane can be hit.

**Result:** Passed. `combat.rs` now walks ordered wall and actor intercepts, uses Chocolate Doom-style `shootz = z + height/2 + 8`, starts with the vanilla `±0.625` slope window, narrows that window across two-sided openings, and only damages actors whose vertical span overlaps the surviving shot cone.

### Task 3: Player Bullet Autoaim Probe

**Files:**
- Modify: `crates/doom-game/src/weapon_fire.rs`
- Modify: `crates/doom-game/src/combat.rs`
- Test: `crates/doom-game/src/weapon_fire.rs`

**Red regressions written:**

- Pistol should acquire a near-off-center target on the right probe.
- Pistol should acquire a near-off-center target on the left probe.
- Chaingun should reuse the same probe behavior.

**Result:** Passed. `weapon_fire.rs` now probes `straight`, `+5.625°`, then `-5.625°` before firing pistol, chaingun, shotgun, and SSG hitscan. `combat.rs` exposes a pure target query helper so the probe can happen without dealing damage first.

**Important note:** Chocolate Doom uses this probe to choose bullet slope, not to replace the shot angle. In this port, exact ray/actor intersection means a pure slope-only probe has almost no observable effect, so the side probe is currently applied as angle selection. That is a deliberate approximation, not an attempt to pass it off as source-identical.

**Remaining combat debt after Task 3:**

- Full psprite-state refire cadence is still less source-faithful than Chocolate Doom, even though the gross held-fire behavior is now correct in the tic loop.
- If we want stricter source parity later, the bullet probe can be split into an explicit aim-slope pass plus a separate fire pass instead of the current angle-selection approximation.

### Task 3: Verification

**Files:**
- Verify only

**Actual verification run so far:**
```powershell
cargo fmt --all
cargo test -p doom-game p_check_missile_range_archvile_rejects_far_targets -- --nocapture
cargo test -p doom-game p_check_missile_range_revenant_rejects_targets_under_196_units -- --nocapture
cargo test -p doom-game a_chase -- --nocapture
cargo test -p doom-game line_attack_with_level_skips_ -- --nocapture
cargo test -p doom-game line_attack -- --nocapture
cargo test -p doom-game autoaim_probe_hits_target -- --nocapture
cargo test -p doom-game weapon_fire -- --nocapture
cargo test -p doom-game --lib
```

All commands passed.
