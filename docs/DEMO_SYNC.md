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
  (`leveltime,seq,retval,caller`), one row per draw, stamped with the
  pre-increment `leveltime` so draws land on the same tic vanilla assigns them.

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
raw "retval by ordinal" diff is trivially identical and tells you nothing. The
real signal is:

1. **Per-tic draw PLACEMENT** — `num_draws` per `leveltime` (how many `P_Random`
   calls fire on each tic, and which callers). If two runs draw the same values
   but on different tics, the sim has already diverged.
2. **The oracle's player position / health / kills** — the ground truth that a
   placement drift eventually perturbs.

Note also that caller *labels* differ between the two codebases (Chocolate Doom
splits `P_SubRandom`/`P_SpawnMapThing`; doom-rs folds some of these under
`spawn`/`p_spawn_mobj`), so placement is compared primarily by **`num_draws` per
leveltime**, not by string-matching caller names.

## Fixes landed (this branch, ~29 commits)

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

## Current sync status

Fresh measurement (release build, harness vs oracle; `--verify-rng-trace`
placement gated to `leveltime ≤ 400`):

| Demo  | Map  | Total tics | Draw-placement 1st diverge (`num_draws`) | Position (px/py) 1st diverge | Health 1st diverge | Final kills (ours / vanilla) | Final items | Final secrets | Determinism |
|-------|------|-----------:|------------------------------------------|------------------------------|--------------------|------------------------------|-------------|---------------|-------------|
| DEMO1 | E1M5 | 5026 | lt 88 (choco 2 / ours 4) | tic 110 (lt 111) | tic 109 (lt 110) | 6 / 61 | 0 / 7 | 0 / 1 | PASS (2×) |
| DEMO2 | E1M3 | 3836 | lt 141 (choco 3 / ours 5) | tic 190 (lt 191) | tic 189 (lt 190) | 2 / 41 | 0 / 14 | 0 / 0 | PASS (2×) |
| DEMO3 | E1M7 | 2134 | lt 142 (choco 59 / ours 60) | tic 190 (lt 191) | tic 189 (lt 190) | 4 / 20 | 0 / 9 | 0 / 0 | PASS (2×) |

Reading this table:

- **Placement diverges before player state.** In every demo the first
  `P_Random`-placement drift (extra draws on a tic) appears ~20–50 tics *before*
  the tracked player position/health moves. This is the signature of a
  **far-sector monster** waking or turning one AI pass early/late: it draws a
  couple of extra `A_Chase`/direction randoms, and only later does that monster
  reach and shoot the player, at which point health and then position diverge.
- The demos all **run to completion** (`demo-stream-fully-consumed`, full tic
  counts match the file), and **cross-run determinism PASSes** on all three —
  the harness self-check confirms the sim is internally reproducible.
- Final `kills/items/secrets` diverge widely from vanilla because once monster
  timing drifts, the rest of a multi-minute playthrough follows a different
  path. The final counts are reported for completeness, **not** as a sync
  metric — the sync depth is the first-divergence tic.

## Remaining divergences (precisely characterized)

Full bit-exact end-to-end sync is **not yet reached**. The `P_Random` value
stream and every major subsystem are vanilla-faithful; what remains is a
shrinking tail of subtle **monster-interaction timing**. The known open items:

1. **Sound flood-extent revisit semantics (primary — chase this next).**
   `P_RecursiveSound`/`P_NoiseAlert` in vanilla re-visits a sector when it is
   reached again at a *lower* `soundtraversed` distance, whereas our flood does
   one visit per sector per pass. On the larger maps this wakes a **far-sector
   monster** one pass early/late, which is exactly the first placement drift:
   - DEMO1 (E1M5): first extra draws at **lt 88** (choco 2 → ours 4).
   - DEMO2 (E1M3): first extra draws at **lt 141** (choco 3 → ours 5).
   - DEMO3 (E1M7): first extra draws at **lt 142** (choco 59 → ours 60).
   Root-cause hypothesis: `P_RecursiveSound` in `p_enemy.c` must re-enqueue a
   sector whose newly-computed `soundtraversed` is smaller than its stored one
   (`if (fdist < sec->soundtraversed) ... P_RecursiveSound(...)`), not gate on a
   single visited-flag. Matching that changes which monsters are in
   `A_Chase`/awake on those tics and removes the extra draws.

2. **Residual combat / damage-thrust interaction gating position.** Once a
   monster wakes on a different tic, its `A_PosAttack`/`A_SPosAttack` hitscan and
   the resulting `P_DamageMobj` knockback thrust land on a different tic, which
   is why **health diverges (~tic 109 / 189 / 189) one tic before position
   (~tic 110 / 190 / 190)**. This is a *downstream* effect of item (1), not an
   independent bug, but the thrust/aim fixed-point path should be re-audited
   against `p_map.c`/`p_inter.c` once the wake timing is exact.

3. **Cosmetic `A_FaceTarget` ATK1 angle transient.** During the first attack
   frame a monster's facing angle can differ by a small BAM delta for one tic
   (`MF_SHADOW` jitter ordering / `R_PointToAngle2` rounding). It self-corrects
   the following tic and does not itself consume a mismatched number of randoms,
   so it does not gate sync — noted for completeness.

## How to continue

- **Next first-divergence to chase:** DEMO1 `leveltime 88`, where our sim draws
  4 randoms vs vanilla's 2. Fix the `P_RecursiveSound` re-visit rule (item 1),
  then re-run the table above.
- **Tooling:** regenerate the harness CSV + rng-trace for the demo under test,
  then compare `num_draws` per `leveltime` against `demoN.choco.rngsummary.csv`
  to find the first placement drift, and the per-tic CSV against
  `demoN.choco.csv` for the first position/health drift. Because the value
  stream is already bit-exact, the *count and placement* of draws per tic — not
  the returned values — is the signal to drive to zero.
