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

## M1b + M1c fixes (post-#1276)

These landed after the original M1a write-up above. Each is matched to the exact
vanilla rule it restores. **M1b** shipped as part of draft PR **#1276**
(`m1-demo-sync`, M1a + M1b, mergeable clean); **M1c** is the continuation branch
`m1c-demo-sync` (draft PR **#1283**, base `m1-demo-sync`). Together they roughly
tripled the sync depth (lt 234/389/194 → lt 600/717/409).

### M1b (merged into #1276)

**Sound propagation**
- **Exact `P_RecursiveSound` revisit semantics** — a sector is re-visited when it
  is reached again at a *lower* `soundtraversed` distance (vanilla
  `if (fdist < sec->soundtraversed) ... P_RecursiveSound(...)`), rather than a
  single visited-flag per pass. Removed the far-sector monster waking one AI pass
  early/late that was the first placement drift.

**Movement**
- `P_Move` **spechit door-use path** — when a move is blocked by a two-sided line
  with a special, vanilla halts the move and calls `P_UseSpecialLine` (returning
  `true`) so monsters open doors instead of walking through; this path was
  missing.
- **Monster walk-momentum removed** — monsters no longer carry a synthetic
  per-step velocity; position is integrated through a **faithful monster
  `P_XYMovement`** with vanilla friction/stop thresholds, matching how vanilla
  moves actors between `A_Chase` steps.

**Spawning**
- `P_SpawnMobj` **animated-item deletion bug** — items whose state has `tics == -1`
  (stay forever) were being overwritten with a `1 + P_Random()%tics`-style
  countdown, so animated pickups counted down to zero and **vanished at level
  start**. The `-1` "never expires" sentinel is now preserved.

**Weapons / player**
- **Weapon-fire pre-move position sampling** — vanilla defers player position
  integration to `P_XYMovement`, which runs *after* `P_MovePsprites`, so a
  hitscan samples the player's **pre-move** position. Reordered to sample
  pre-move, fixing autoaim/hit geometry on the firing tic.
- **Weapon raise/lower cadence** — `WEAPONTOP = 32`, and the first `A_Raise`
  fires immediately on bring-up (no one-tic stall), matching vanilla psprite
  timing.

**Monster AI / combat RNG**
- `A_Scream` **death-sound-variant draw** — the death scream draws a `P_Random`
  to pick the sound variant; that draw was missing, shifting the stream on every
  monster death.
- `P_SpawnMissile` **draws** — restored the `lastlook` / `P_CheckMissileSpawn`
  `P_Random` draws vanilla makes when a monster launches a projectile.

**Projectiles**
- **Missile double-move / f32-spawn fix** — replaced the simplified projectile
  move with a **swept `P_XYMovement` / `PIT_CheckThing` / `P_ExplodeMissile`**
  port so a missile's per-tic step, collision test, and explosion match vanilla
  fixed-point exactly (no double-step, no float spawn coordinate).

**Thinkers**
- **Thinker creation-order + interleaved light pass** — thinkers run in vanilla
  creation order with the light specials interleaved into the same pass, so
  per-tic `P_Random` draw ordering across map objects matches vanilla.

**Explosions**
- **Barrel explosion + `P_RadiusAttack`** — vanilla-faithful barrel chain-reaction
  and radius damage (splash falloff and thrust), matching the RNG/damage vanilla
  applies to actors in the blast.

### M1c (this branch, `m1c-demo-sync`)

**Specials / doors**
- **`DOOR_WAIT` = vanilla 150** — the door open-wait delay was wrong; restored to
  the vanilla 150-tic constant so door cycles match.

**Items / drops**
- **Dropped-weapon half-ammo (`MF_DROPPED`)** — a weapon dropped by a killed
  monster carries half the normal clip; the `MF_DROPPED` flag and its half-ammo
  pickup rule were restored so ammo counts (and any pickup-driven state) match.

**Death path**
- **Gib / xdeath over-kill death path** — removed the health clamp that prevented
  going deep negative, added the **xdeath (gib) states** and the **`A_XScream`**
  action, so an over-killed actor takes vanilla's extreme-death path (correct
  states, sound, and RNG draws).

**Weapons**
- **Weapon movement bob** — the psprite bob is now computed in vanilla
  **fixed-point** (`psp->sy` from `player->bob`) instead of an approximation, and
  the **refire** path was fixed, so weapon-sway state matches per tic.

**Movement**
- **Monster `P_Move` full spechit accumulation** — `P_Move` now accumulates the
  full spechit list (all crossed special lines) as vanilla does, clearing the
  monster `movedir` correctly, rather than stopping at the first spechit.

## Current sync status

Fresh measurement (release build at HEAD `f819580` on `m1c-demo-sync`, harness
vs oracle, diffed on `px,py,pz,angle,health,kills,items,secrets,leveltime`; the
cosmetic `rndindex` column is ignored). Measured 2026-07-09:

| Demo  | Map  | Total tics | Position (px/py) 1st diverge | Health 1st diverge | Kills 1st diverge | Final k/i/s (ours) | Final k/i/s (vanilla) | Determinism |
|-------|------|-----------:|------------------------------|--------------------|-------------------|--------------------|-----------------------|-------------|
| DEMO1 | E1M5 | 5026 | tic 599 (lt 600) | tic 613 (lt 614) | tic 703 (lt 704) | 7 / 0 / 0 | 61 / 7 / 1 | PASS (2×) |
| DEMO2 | E1M3 | 3836 | tic 717 (lt 718) | tic 716 (lt 717) | tic 970 (lt 971) | 9 / 0 / 0 | 41 / 14 / 0 | PASS (2×) |
| DEMO3 | E1M7 | 2134 | tic 408 (lt 409) | tic 467 (lt 468) | tic 632 (lt 633) | 8 / 0 / 0 | 20 / 9 / 0 | PASS (2×) |

Reading this table:

- **The M1b + M1c fixes roughly tripled the sync depth.** Position, health, and
  kills now track the oracle exactly for the first several hundred tics of each
  demo — first divergence is at **lt 600 / lt 717 / lt 409** (≈ 12% / 19% / 19%
  of each demo's length), up from lt 234 / 389 / 194 before these fixes. The
  **`P_Random` values still match the oracle through the entire recorded
  window** (zero retval-by-ordinal mismatches); what breaks first is player
  *state* (position/health), not the RNG value stream.
- **Which of position/health/kills breaks first now varies by demo.** In DEMO2
  health leads position by one tic (lt 717 vs 718 — a damage roll lands, then
  the knockback thrust moves the player the next tic). In DEMO1 and DEMO3
  position leads (lt 600 before health lt 614; lt 409 before health lt 468) — a
  residual sub-map-unit movement drift shows in position before any damage
  lands. Kills diverge latest in all three (lt 704 / 971 / 633).
- The demos all **run to completion** (`demo-stream-fully-consumed`, full tic
  counts match the file — 5026 / 3836 / 2134), and **cross-run determinism
  PASSes** on all three (`--verify-runs 2`: PASS) — the harness self-check
  confirms the sim is internally reproducible.
- Final `kills/items/secrets` diverge widely from vanilla because once one
  monster interaction drifts, the rest of a multi-minute playthrough follows a
  different path. The final counts are reported for completeness, **not** as a
  sync metric — the sync depth is the first-divergence tic.

## Remaining divergences (characterized)

Full bit-exact end-to-end sync is **not yet reached.** Be honest about where we
are: the demos currently sync only the **first ~12–19% of their length** (first
divergence lt 600 / 717 / 409 of 5026 / 3836 / 2134 tics). What is solid: the
`P_Random` **value** stream matches the oracle through the whole recorded window,
every fix above is **vanilla-verified against the instrumented Chocolate oracle**,
and every major subsystem is vanilla-faithful. What remains: the multi-minute
tail still diverges, and the remaining failures are **residual monster
positional drift + attack/AI timing that compounds over the run**, not a broken
RNG or a missing subsystem.

The general mechanism: both sides draw the **same RNG rolls**, but a slightly
**drifted geometry** (a monster or projectile a few map units off) turns the same
roll into a hit-vs-miss or an earlier-vs-later hit, or shifts an `A_Chase` step
by a tic. That one difference perturbs health, then knockback position, then
which monster the player faces next — and the differences compound over the run.

Known open items (characterized per demo):

1. **DEMO1 — lt 600 / lt 681, a monster-AI / combat divergence (TBD).** Player
   position first drifts at lt 600 (health follows at lt 614), and a further
   monster-AI/combat divergence shows around lt 681. Root cause not yet pinned;
   it is downstream of sub-map-unit geometry drift in the preceding chase steps,
   so a same-value damage roll or chase step lands on a slightly different tic.
   Re-audit the `A_Chase` / attack hitscan geometry and `P_DamageMobj` thrust
   against `p_map.c` / `p_inter.c` for the last residual fixed-point rounding.

2. **DEMO2 — lt 628, non-gib monster-AI drift (~3 extra `A_Chase` rolls).** A
   monster takes roughly **3 extra `A_Chase` `P_Random` rolls** relative to the
   oracle around lt 628; this is a non-gib (ordinary chase/attack) AI-timing
   divergence that surfaces in player state at lt 717/718. Trace the offending
   monster's `A_Chase` / `P_NewChaseDir` roll count against the oracle.

3. **DEMO3 — lt 409 / lt 451, monster missile exploding one tic early.** Player
   position first drifts at lt 409 (a one-tic `py` movement stall from residual
   positional drift); the visible consequence is a **monster missile that
   explodes one tic early** around lt 451. Downstream of the same residual
   fixed-point positional drift — the projectile's swept move is faithful, but a
   few-map-unit actor offset shifts the explosion tic.

4. **Cosmetic `A_FaceTarget` ATK1 angle transient.** During the first attack
   frame a monster's facing angle can differ by a small BAM delta for one tic
   (`MF_SHADOW` jitter ordering / `R_PointToAngle2` rounding). It self-corrects
   the following tic and does not consume a mismatched number of randoms, so it
   does not gate sync — noted for completeness.

## How to continue

- **Next divergences to chase:** DEMO1 `leveltime 600` (monster-AI / combat
  divergence, item 1), DEMO2 `leveltime 628` (~3 extra `A_Chase` rolls, item 2),
  and DEMO3 `leveltime 409`/`451` (residual positional drift → early missile
  explosion, item 3). The common thread is **residual monster positional drift**
  compounding into attack/AI timing.
- **Tooling:** regenerate the harness CSV for the demo under test and diff it
  against `demoN.choco.csv` on `px,py,pz,angle,health,kills,items,secrets,
  leveltime` (ignore the `rndindex` column) to find the first position/health/
  kills drift. Use **position/health/kills as the signal** — the RNG *values*
  already match through the recorded window, and the rng-trace `leveltime` stamp
  has a known off-by-one, so its per-tic "draw placement" is not a reliable
  signal to diff against. Note the oracle raw trace runs to ~lt 1644, so
  measurement well past lt 400 is possible.
