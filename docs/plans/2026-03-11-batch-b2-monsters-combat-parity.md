# Batch B2 Monsters Combat Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Status:** In progress on 2026-03-11.

**Outcome so far:** The first B2 slice is landed and green. `p_check_missile_range()` now matches the vanilla Arch-Vile maximum-range rule and the Revenant minimum-range rule, with regressions to keep both from drifting.

**Goal:** Finish the remaining monster/combat parity debt from System 2, starting with source-backed `P_CheckMissileRange` fixes and then tackling elevated-target hitscan autoaim / bullet slope behavior.

**Architecture:** Keep monster missile gating and hitscan parity separate. Missile-range fixes belong in `actions.rs` with direct unit coverage. Elevated-target autoaim is a larger combat-path change: it needs a more Doom-shaped vertical slope model instead of the current 2D-only hitscan path.

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

**Current finding:**

- Local hitscan is still effectively 2D. `p_line_attack()` in `combat.rs` does not use slope at all, and `trace::trace_ray()` actor tests ignore actor `z`.
- `p_aim_line_slope()` exists, but it is not currently part of hitscan selection or occlusion.

**Next regression to write:**

- An elevated target directly ahead in open space should be auto-aimed and hit.
- A target that is only aligned in 2D but vertically outside the shot window should not be hit.

### Task 3: Verification

**Files:**
- Verify only

**Actual verification run so far:**
```powershell
cargo fmt --all
cargo test -p doom-game p_check_missile_range_archvile_rejects_far_targets -- --nocapture
cargo test -p doom-game p_check_missile_range_revenant_rejects_targets_under_196_units -- --nocapture
cargo test -p doom-game a_chase -- --nocapture
cargo test -p doom-game --lib
```

All commands passed.
