# Demo-Sync Oracle (instrumented Chocolate Doom)

Ground-truth reference for validating doom-rs demo parity. An instrumented
Chocolate Doom emits a per-tic CSV in the **exact** column layout and sampling
point as the doom-rs harness (`doom-app --verify-demo … --verify-log`), so the
two are directly row-by-row diffable.

doom-rs generates its side of the comparison with
`cargo run -p doom-app --release -- --iwad test-wads/doom1.wad --verify-demo DEMONAME --verify-log out.csv`
(see `docs/DEMO_SYNC.md`); this tree produces the vanilla side.

## Reproduction recipe

| Item | Value |
|------|-------|
| Upstream | `https://github.com/chocolate-doom/chocolate-doom.git` |
| Pinned commit | `353cf5001dfd5777c13327010fa58acb57b913b2` (master, 2026-06-08) |
| IWAD | shareware `doom1.wad`, md5 `f0cefca49926d00903cf57551d901abe` |
| Oracle binary | `$ORACLE_BUILD_DIR/src/chocolate-doom` (default `./chocolate-doom/src/chocolate-doom`) |

The clone/build tree lives in `$ORACLE_BUILD_DIR` (default `tools/oracle/chocolate-doom`,
which is gitignored). Set `ORACLE_BUILD_DIR` to a temp dir to keep the working
tree pristine, e.g. `ORACLE_BUILD_DIR=$(mktemp -d)/oracle`.

### apt deps (Ubuntu/Debian)

```
sudo apt-get update          # REQUIRED first — stale index 404s on libsystemd-dev
sudo apt-get install -y automake autoconf pkg-config \
                        libsdl2-dev libsdl2-mixer-dev libsdl2-net-dev
```

### Build

```
./build.sh                   # clone+pin, apply patch, autogen, make -j
# equivalently, by hand:
git clone https://github.com/chocolate-doom/chocolate-doom.git
cd chocolate-doom && git checkout 353cf5001dfd5777c13327010fa58acb57b913b2
git apply ../oracle-instrumentation.patch
./autogen.sh && make -j$(nproc)
```

### Run (headless)

```
./run.sh demo2 demo2.choco.csv ../../test-wads/doom1.wad
# under the hood:
SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy CHOCO_CSV=demo2.choco.csv \
  chocolate-doom/src/chocolate-doom -iwad doom1.wad -timedemo demo2 \
  -nosound -nomusic -config <tmp>/default.cfg -extraconfig <tmp>/extra.cfg
```

`-timedemo` runs the demo uncapped (fast) and quits; sim tics are identical to
`-playdemo`. The process exits non-zero (`I_Quit` after timedemo) — that is
normal, the CSV is fully written (fflush per row).

Row counts (must match doom-rs): DEMO1 **5026**, DEMO2 **3836**, DEMO3 **2134**.

## The instrumentation (`oracle-instrumentation.patch`)

Single-file patch to `src/doom/p_tick.c`. Adds `#include <stdio.h>/<stdlib.h>`,
`extern int prndindex;`, and an emit block at the **end of `P_Ticker()`**, right
after `leveltime++`. Opens `$CHOCO_CSV` once (env var; if unset, emits nothing —
zero behavioural change to normal play), writes the header, then one row per tic.

### CSV format & sampling point (matched to doom-rs exactly)

Header: `i,rndindex,px,py,pz,angle,health,kills,items,secrets,leveltime`

- **Sampling point**: end of `P_Ticker`, i.e. AFTER `P_PlayerThink` + `P_RunThinkers`
  + `P_UpdateSpecials` + `P_RespawnSpecials`, and AFTER `leveltime++`. This is the
  identical point doom-rs samples (it reads player-0 state right after
  `gs.tick(cmd)` returns). Verified: the first 3 rows of every demo are
  byte-identical between oracle and doom-rs → no phase offset.
- **`i`** = 0-based tic index, emitted as `leveltime - 1` (exactly one P_Ticker
  call per tic; `leveltime` starts at 0, so `leveltime == i+1`).
- **`px,py,pz`** = `players[consoleplayer].mo->x/y/z` — raw `fixed_t` (`%d`).
- **`angle`** = `players[consoleplayer].mo->angle` — raw `angle_t` (`%u`).
- **`rndindex`** = `prndindex` (the **P_Random** stream index), NOT the cosmetic
  `M_Random` `rndindex`. doom-rs uses a single unified RNG whose `index()` is the
  P_Random index, so emitting `prndindex` makes the column directly comparable.
  (DEMO_SYNC.md says "ignore the rndindex column" because in *vanilla* the column
  is normally the cosmetic M_Random counter; here it is deliberately prndindex,
  and it IS a valid RNG-consumption signal.)
- **`health`** = `players[consoleplayer].health` (`%d`, may go negative).
- **`kills/items/secrets`** = `players[consoleplayer].killcount/itemcount/secretcount`.
- **`leveltime`** = the global `leveltime`, post-increment.

## Diffing

`diff_csv.py <choco.csv> <doomrs.csv>` — first significant divergence (ignores
`rndindex`) with a 4-tic context window. `perfield.py` — first-divergence tic for
every field independently (expects `oracle/demoN.choco.csv` +
`demoN.doomrs.csv`). Both live in this dir.

## Results (trunk `0cf9e69` + M2a fix `1b26c6a`, measured on `m2a-demo2-sync`)

First-divergence tic per field (leveltime = tic + 1). DEMO1/DEMO3 are unchanged
by the M2a monster-line-crossing fix (guardrail-verified); the DEMO2 column
reflects the post-M2a state (see `docs/DEMO_SYNC.md`).

| field | DEMO1 (E1M5) | DEMO2 (E1M3) | DEMO3 (E1M7) |
|-------|-------------|-------------|-------------|
| rndindex (prndindex) | tic 1408 | **tic 1920** | none |
| px | tic 1652 | tic 2000 | **none** |
| py | tic 1652 | tic 2000 | **none** |
| pz | tic 1658 | **tic 1962** | **tic 735** |
| angle | tic 1916 | — | none |
| health | **tic 1408** | — | none |
| kills | tic 1682 | — | none |
| items | none | — | none |
| secrets | tic 1457 | — | none |
| total rows | 5026 | 3836 | 2134 |

(DEMO2 `angle/health/kills/items/secrets` not individually re-measured after M2a;
the first divergence overall is `rndindex` at 1920, first geometry is `pz` at 1962.)

- **DEMO1** first significant divergence: **health at tic 1408 (lt 1409)** — at
  that tic doom-rs draws **1 FEWER** P_Random (89→88) and health is 1 lower
  (86 vs 85): a combat/damage RNG-consumption divergence. Trajectory (px/py)
  holds until tic 1652.
- **DEMO2** (post-M2a): first divergence overall is an `rndindex`/painchance
  mismatch at **tic 1920** (doom-rs draws **1 FEWER** P_Random — a painchance
  roll in `P_DamageMobj`), and the first geometry divergence is **pz at tic 1962**
  (~51%); position (px/py) holds to tic 2000. The prior pre-M2a divergence was at
  tic 814 (2 EXTRA draws), fixed by the M2a monster box-straddle crossing rule.
- **DEMO3** is bit-exact end-to-end on every outcome field
  (rndindex,px,py,angle,health,kills,items,secrets). The **only** residual is a
  transient **pz +4.000-unit** offset (one platform step, 262144 fixed) during
  lift/platform rides — ~60 of 2134 tics, in bursts (first at tic 735), fully
  self-correcting (final rows byte-identical). This is the known-accepted residual.

## Files in this dir (committed)

- `oracle-instrumentation.patch` — the standalone instrumentation diff.
- `build.sh`, `run.sh` — reproduction scripts.
- `diff_csv.py`, `perfield.py` — analysis.
- `README.md` — this file.

Not committed (gitignored, see `.gitignore`): `chocolate-doom/` (the pinned,
patched, built clone) and the generated `demo{1,2,3}.choco.csv` traces.
