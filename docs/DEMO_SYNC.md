# Vanilla Demo-Sync (M1)

Status of the Milestone-1 effort to make the doom-rs playsim bit-exact against
vanilla Chocolate Doom, validated by replaying the shareware demos tic-by-tic.

## Overview

The playsim is validated with a headless demo-verification harness built into
`doom-app` (`crates/doom-app/src/main.rs`). It replays a recorded `.lmp` demo
with **no rendering and no audio**, driving level/skill/flags from the demo
header, and records what the simulation did on every tic.

Flags:

- `--verify-demo <SOURCE>` — replay a demo. `SOURCE` is either a path to an
  external `.lmp` or a lump name (`DEMO1`/`DEMO2`/`DEMO3`) resolved from the
  loaded IWAD.
- `--verify-log <PATH>` — write the per-tic CSV
  (`i,rndindex,px,py,pz,angle,health,kills,items,secrets,leveltime`).
- `--verify-runs N` — replay the demo `N` times from scratch and assert every
  run's CSV is byte-identical (cross-run determinism self-check; default 2).
- `--verify-rng-trace <PATH>` — write a per-draw `P_Random` call-trace
  (`leveltime,seq,retval,caller`), one row per draw. The `leveltime` stamp has a
  known off-by-one (see the methodology gotcha), so use it for the *value*
  stream, not for per-tic draw placement.

Run it like:

```
cargo run -p doom-app --release -- \
  --iwad test-wads/doom1.wad --verify-demo DEMO1 \
  --verify-log demo1.doomrs.csv \
  --verify-rng-trace demo1.doomrs.rngtrace.csv
```

What it measures:

- **Per-tic player state** — position (`px,py,pz`), `angle`, `health`, and
  `kills/items/secrets` at every tic.
- **Every `P_Random` draw** — sequence, returned value, and the caller context
  that consumed it, tagged with the leveltime it fired on.
- **Cross-run determinism** — the same demo replayed N times must produce a
  byte-identical CSV. The harness prints `determinism (Nx): PASS/FAIL`.

### Test corpus

The corpus is the three shareware demos in `doom1.wad`:

| Lump  | Map  |
|-------|------|
| DEMO1 | E1M5 |
| DEMO2 | E1M3 |
| DEMO3 | E1M7 |

The IWAD is **not committed** — `test-wads/` is gitignored (`*.wad` and
`/test-wads/`). Use the shareware `doom1.wad` with md5
`f0cefca49926d00903cf57551d901abe` and place it at `test-wads/doom1.wad`.

## Reference oracle

The comparison baseline is an **instrumented Chocolate Doom**, used as
methodology only (its patched source is not committed here). It was patched to
emit, on every tic of a demo replay:

1. Per-tic player state in the same column layout as the harness CSV
   (`px,py,pz,angle,health,kills,items,secrets,leveltime`), and
2. A resolved `P_Random` consumption trace — every `P_Random()` call with the
   function that consumed it, collapsed to a per-tic summary
   (`leveltime,num_draws,first_func,funcs`).

Those oracle CSVs (`demoN.choco.csv`, `demoN.choco.rngsummary.csv`) are diffed
against the harness output to find the first tic where anything diverges.

## KEY METHODOLOGY GOTCHA

**Two independent RNG streams share one `rndtable`; only one governs demo sync.**

- Vanilla's global `rndindex` counter — the one exposed in most debug dumps and
  the oracle's `rndindex` **column** — is the **cosmetic `M_Random`** counter.
  It drives the status-bar face, the screen-wipe melt, and sound pitch. It has
  **no effect on the simulation** and is not deterministic w.r.t. demo playback.
  **Comparing against the oracle's `rndindex` column is invalid** — ignore it.
- The playsim RNG that actually governs demo sync is **`P_Random`**, indexed by
  `prndindex`. This is what spawns, AI, movement, damage, and specials draw
  from. This is the stream the harness's `--verify-rng-trace` captures and the
  only one that matters for parity.

**The retval-by-ordinal stream is NOT a divergence signal.** Because both
implementations cycle the *same* 256-byte `rndtable` by draw count, the Nth draw
returns an identical byte in both — regardless of *who* drew it or *when*. So a
raw "retval by ordinal" diff is trivially identical and tells you nothing. In
fact the RNG **values match the oracle through the entire recorded window** on
all three demos (verified: 1807 / 1256 / 2813 draws match retval-by-ordinal with
zero mismatches). Matching values is necessary but not sufficient — it does not
by itself prove sync.

**The real signal is the oracle's player position / health / kills.** These are
the ground truth that any placement/timing drift eventually perturbs, and they
are what the "Current sync status" table below is measured against.

**The rng-trace `leveltime` stamp has a known off-by-one, so its "draw
placement" column is NOT a reliable signal.** The trace stamps each draw with a
`leveltime` that can be one tic off from the tic vanilla attributes it to, so
comparing `num_draws` per `leveltime` against the oracle summary produces
spurious ±1-tic drifts that are artifacts of the stamp, not real divergences.
**Do not chase placement drift; use position/health/kills as the signal.**

Note also that caller *labels* differ between the two codebases (Chocolate Doom
splits `P_SubRandom`/`P_SpawnMapThing`; doom-rs folds some of these under
`spawn`/`p_spawn_mobj`), so caller-name string-matching is likewise unreliable.

## Fixes landed (this branch, ~31 commits)

Each fix was matched to the exact vanilla rule it restores.

**RNG core**
- Byte-exact vanilla `rndtable[256]` — 231 of 256 bytes were wrong past index 22.
- `P_Random` **increment-before-read** ordering (`prndindex = (prndindex+1)&0xff;
  return rndtable[prndindex]`).
- Restored vanilla **spawn-time draws**: `lastlook = P_Random() % MAXPLAYERS` per
  mobj, and `tics = 1 + (P_Random() % tics)` per spawned thing.
- Vanilla **`ANG45 * (deg/45)`** spawn-angle formula (no RNG at spawn angle).

**Trig**
- Bit-exact vanilla `finesine`/`finecosine` — the generated table mismatched
  8090/8192 entries; replaced with the vanilla LUT.

**Movement**
- `MAXMOVE` 15→30 (`p_local.h`).
- Vanilla `P_XYMovement` stepping + `P_SlideMove` wall-slide (with
  `tantoangle`/`R_PointToAngle2` LUTs), `P_BoxOnLineSide` fixed-point straddle
  test.
- Player `P_ZMovement` gravity + on-ground thrust/friction gating.

**Line-of-sight**
- Vanilla-exact `P_CheckSight` via BSP traversal with accumulated slope
  clipping (replaces the approximate visibility test).

**Specials / sectors**
- Light specials (`T_LightFlash`/strobe/glow/fireflicker) consume `P_Random`
  per vanilla.
- `P_PlayerInSpecialSector` runs on the player's *actual* sector and in vanilla
  order (before player z-integration) — fixes a spurious tic-0 −2hp and a
  secret over-count.

**Fixed-point**
- `FixedDiv` overflow guard matches vanilla `INT_MIN/MAX` saturation (no panic).

**Monster AI**
- Vanilla `P_Random` in `A_Look`/`A_Chase`/`P_NewChaseDir`/`P_TryWalk` and the
  attack actions (`A_PosAttack`/`A_SPosAttack`/`A_TroopAttack`/`A_SargAttack`).
- Vanilla RUN-state `tics` values (from `info.c`) so A_Chase cadence matches.
- `A_FaceTarget` uses exact `R_PointToAngle2` + `MF_SHADOW` angle jitter.

**Combat / hitscan**
- Vanilla-exact fixed-point hitscan: `P_PathTraverse` autoaim, `P_GunShot`
  damage, `P_SpawnPuff`/`P_SpawnBlood`, and knockback thrust in `P_DamageMobj`.
- Vanilla `P_KillMobj` kill-credit, corpse flags, and item-drop RNG.
- `PIT_CheckThing` mobj-vs-mobj collision (was missing) + barrel solidity.

**Sound propagation**
- `init_sound_state` + `P_NoiseAlert` timing and the `P_LineOpening` flood gate
  wired into the weapon-fire noise alert.
- **Exact `P_RecursiveSound` revisit semantics** — a sector is re-visited when it
  is reached again at a *lower* `soundtraversed` distance (vanilla
  `if (fdist < sec->soundtraversed) ... P_RecursiveSound(...)`), rather than a
  single visited-flag per pass. This removed the far-sector monster waking one
  AI pass early/late that used to be the first placement drift (old lt 88/141/142)
  and pushed the first *player-state* divergence out to lt 234/389/194.

### Fixes landed since the first write-up (batches 11–18)

Each is matched to the exact vanilla rule it restores.

**Movement**
- `P_Move` **spechit door-use path** — when a move is blocked by a two-sided
  line with a special, vanilla halts the move and calls `P_UseSpecialLine`
  (returning `true`) so monsters open doors instead of walking through; this path
  was missing.
- **Monster walk-momentum removed** — monsters no longer carry a synthetic
  per-step velocity; their position is integrated through a **faithful monster
  `P_XYMovement`** with vanilla friction/stop thresholds, matching how vanilla
  moves actors between `A_Chase` steps.

**Spawning**
- `P_SpawnMobj` **animated-item deletion bug** — items whose state has `tics == -1`
  (stay forever) were being overwritten with a `1 + P_Random()%tics`-style
  countdown, so animated pickups counted down to zero and **vanished at level
  start**. The `-1` "never expires" sentinel is now preserved.

**Weapons / player**
- **Weapon-fire position-integration order** — vanilla defers player position
  integration to `P_XYMovement`, which runs *after* `P_MovePsprites`, so a
  hitscan samples the player's **pre-move** position. Our ordering was integrating
  first; reordered to sample pre-move, fixing autoaim/hit geometry on the firing
  tic.
- **Weapon raise/lower cadence** — `WEAPONTOP = 32`, and the first `A_Raise`
  fires immediately on bring-up (no one-tic stall), matching vanilla psprite
  timing.

**Monster AI / combat RNG**
- `A_Scream` **death-sound-variant `P_Random` draw** — the death scream draws a
  `P_Random` to pick the sound variant; that draw was missing, shifting the
  stream on every monster death.
- `P_SpawnMissile` draws — restored the `lastlook`/`P_CheckMissileSpawn`
  `P_Random` draws that vanilla makes when a monster launches a projectile.

## Current sync status

Fresh measurement (release build at HEAD `8ec5aef`, harness vs oracle, diffed on
`px,py,pz,angle,health,kills,items,secrets,leveltime`; the cosmetic `rndindex`
column is ignored). Measured 2026-07-09:

| Demo  | Map  | Total tics | Position (px/py) 1st diverge | Health 1st diverge | Kills 1st diverge | Final k/i/s (ours) | Final k/i/s (vanilla) | Determinism |
|-------|------|-----------:|------------------------------|--------------------|-------------------|--------------------|-----------------------|-------------|
| DEMO1 | E1M5 | 5026 | tic 233 (lt 234) | tic 232 (lt 233) | tic 490 (lt 491) | 6 / 0 / 0 | 61 / 7 / 1 | PASS (2×) |
| DEMO2 | E1M3 | 3836 | tic 388 (lt 389) | tic 387 (lt 388) | tic 370 (lt 371) | 5 / 0 / 0 | 41 / 14 / 0 | PASS (2×) |
| DEMO3 | E1M7 | 2134 | tic 193 (lt 194) | tic 192 (lt 193) | tic 183 (lt 184) | 8 / 0 / 0 | 20 / 9 / 0 | PASS (2×) |

Reading this table:

- **Sync now holds for the first few hundred tics of each demo.** Player
  position, health, and kills track the oracle exactly until the tics above —
  and the **`P_Random` values match the oracle through the entire recorded
  window** (1807 / 1256 / 2813 draws, zero retval-by-ordinal mismatches). What
  breaks first is player *state* (position/health), not the RNG value stream.
- **Health diverges one tic before position** in DEMO1/DEMO2/DEMO3 (lt 233 vs
  234, 388 vs 389, 193 vs 194): a monster's damage roll lands, then the resulting
  knockback thrust moves the player the next tic. Kills can diverge earlier or
  later than position (DEMO2 lt 371, DEMO3 lt 184 before position; DEMO1 lt 491
  after) depending on which monster interaction drifts first.
- The demos all **run to completion** (`demo-stream-fully-consumed`, full tic
  counts match the file), and **cross-run determinism PASSes** on all three —
  the harness self-check confirms the sim is internally reproducible.
- Final `kills/items/secrets` diverge widely from vanilla because once one
  monster interaction drifts, the rest of a multi-minute playthrough follows a
  different path. The final counts are reported for completeness, **not** as a
  sync metric — the sync depth is the first-divergence tic.

## Remaining divergences (characterized)

Full bit-exact end-to-end sync is **not yet reached.** Be honest about where we
are: the `P_Random` **value** stream matches the oracle through the whole
recorded window, every major subsystem is vanilla-faithful, and sync holds for
the first few hundred tics of each demo — but the multi-minute tail still
diverges. The remaining failures are **monster drift-coupling / timing
micro-bugs**, not a broken RNG or a missing subsystem.

The general mechanism: both sides draw the **same RNG damage rolls**, but a
slightly **drifted geometry** (a monster or projectile a few map units off)
turns the same roll into a hit-vs-miss or an earlier-vs-later hit. That one
difference perturbs health, then knockback position, then which monster the
player faces next — and the differences compound over the run.

Known open items:

1. **DEMO1 lt 233 — monster attack / damage-roll timing (primary).** The first
   divergence is a monster attack whose damage lands about **3 tics off** the
   oracle (health drops at lt 233 vs the oracle's later tic; position follows at
   lt 234). This is downstream of sub-map-unit geometry drift in the preceding
   chase steps, so the damage roll — same value — is applied on a slightly
   different tic. Re-audit the `A_PosAttack`/`A_SPosAttack` hitscan geometry and
   the `P_DamageMobj` thrust against `p_map.c`/`p_inter.c` for the last residual
   fixed-point rounding.

2. **DEMO3 — projectile movement not vanilla-faithful.** `p_move_projectiles`
   is not a faithful port: an imp fireball **hits early** on E1M7, which is what
   drives DEMO3's earlier (lt 194) divergence. The fix is a **swept
   `P_XYMovement` / `PIT_CheckThing`** port for missiles so a projectile's
   per-tic step and collision test match vanilla exactly, instead of the current
   simplified move.

3. **Cosmetic `A_FaceTarget` ATK1 angle transient.** During the first attack
   frame a monster's facing angle can differ by a small BAM delta for one tic
   (`MF_SHADOW` jitter ordering / `R_PointToAngle2` rounding). It self-corrects
   the following tic and does not consume a mismatched number of randoms, so it
   does not gate sync — noted for completeness.

## How to continue

- **Next divergences to chase:** DEMO1 `leveltime 233` (monster-attack /
  damage-roll timing, item 1) and DEMO3's projectile-movement port (item 2).
- **Tooling:** regenerate the harness CSV for the demo under test and diff it
  against `demoN.choco.csv` on `px,py,pz,angle,health,kills,items,secrets,
  leveltime` (ignore the `rndindex` column) to find the first position/health/
  kills drift. Use **position/health/kills as the signal** — the RNG *values*
  already match through the recorded window, and the rng-trace `leveltime` stamp
  has a known off-by-one, so its per-tic "draw placement" is not a reliable
  signal to diff against.
