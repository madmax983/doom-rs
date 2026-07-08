# doom-rs → Chocolate Doom Parity Roadmap

> Goal: bring **doom-rs** to feature parity with **Chocolate Doom**, the reference vanilla-accurate Doom source port. This document is a current-state inventory, a gap list grouped by subsystem, and a prioritized milestone plan. It is analysis only — no features are implemented yet.
>
> _Last updated: 2026-07-08._

## 0. Executive summary

doom-rs is already a **remarkably complete** Doom engine: ~115k lines of Rust across a 10-crate workspace, ~2,800 unit tests, and near-zero stubs. WAD loading, the software BSP renderer, the full playsim, complete monster AI, all map specials, all 9 weapons, HUD/status bar, menus, automap, cheats, OPL music, and digital SFX are all **fully implemented and heavily tested**.

The parity gaps are therefore **not** "big missing features" — they are **vanilla-fidelity** gaps: the things Chocolate Doom exists to guarantee. The single most important framing point:

> **doom-rs is a terminal (TUI) port, not an SDL port.** It renders a vanilla-style 8-bit paletted software framebuffer, but presents it in the terminal (ratatui/crossterm: halfblocks/sixel/kitty/etc.). There is no SDL2/wgpu window. This means *pixel-for-pixel display fidelity* and *DOS input feel* — two of Chocolate Doom's stated goals — are out of scope by design. What **is** achievable and valuable is **simulation-level parity**: bit-exact playsim, demo sync, vanilla limits/overflow behavior, config/savegame/CLI compatibility. This roadmap targets simulation parity and treats presentation parity as explicitly non-goal.

**Top 5 gaps (highest severity first):**
1. **Vanilla limits & overflow behavior** — doom-rs uses dynamic `Vec`/`ArrayVec` clip buffers and verification-oriented caps, so it is effectively *limit-removing*. It will not reproduce visplane-overflow crashes, tutti-frutti, medusa, all-ghosts/intercepts overflow, or the "no more plats" fatal — all of which Chocolate reproduces on purpose.
2. **Vanilla demo sync (bit-exactness)** — the LMP format (v1.9 / version 109) is present, but tic-by-tic sync against real DOS/Chocolate `.lmp` demos is **unverified**. This is the hardest and most important acceptance test, gated on exact `M_Random` (`rndtable[256]`) call order.
3. **Config file handling** — **absent**. No `default.cfg` / `chocolate-doom.cfg` read or write; all settings are CLI-only.
4. **Game-mode / IWAD auto-detection** — **absent**. No automatic identification of shareware / registered / Ultimate / Doom II / Final Doom (Plutonia/TNT) / Chex / HACX from IWAD lumps.
5. **CLI arg parity** — several vanilla flags missing (`-nomonsters`, `-respawn`, `-fast`, `-turbo`, `-episode`, `-loadgame`, `-deathmatch`/`-altdeath`, `-file`, response files); doom-rs uses GNU `--long` conventions rather than vanilla single-dash.

**Proposed milestone order:** M1 Playsim correctness + demo sync → M2 Vanilla limits/overflow (opt-in compat mode) → M3 Game-mode auto-detection → M4 Config files → M5 CLI parity → M6 Sound completeness → M7 Vanilla savegame format → M8 Vanilla netplay → M9 Content polish (cast call, BEX). Rationale below.

## 1. Reference target: what "parity" means

Chocolate Doom's design philosophy is deliberately minimalist and historically accurate: 100% free/open-source, **accurate reproduction of the original DOS versions including bugs**, and **compatibility with DOS demo, config, and savegame files**. It refuses gameplay enhancements (no hi-res, mouselook, jumping) — anything that changes feel or breaks vanilla compat is out of scope. That strict bug-for-bug replica is the correct bar for a parity-focused engine.

Sources: [PHILOSOPHY.md](https://github.com/chocolate-doom/chocolate-doom/blob/master/PHILOSOPHY.md), [NOT-BUGS.md](https://github.com/chocolate-doom/chocolate-doom/blob/master/NOT-BUGS.md), [Doom Wiki: Chocolate Doom](https://doom.fandom.com/wiki/Chocolate_Doom).

### "Iron Doom" — the other named target

The user also named **Iron Doom** as a parity target. It is **real** and is _not_ a misremembering of Crispy Doom:

- **Repo:** [`Henrique194/iron-doom`](https://github.com/Henrique194/iron-doom) — "A modern and readable Doom source port written in Rust."
- **Basis:** a Rust rewrite **based on the Chocolate Doom codebase**, using **Bevy's ECS** to map C patterns into idiomatic Rust; GPL-3.0.
- **Status:** young/niche — first release **v0.1.0 on 2024-12-24**, single author, ~55 stars, ~12 commits at time of research. The author explicitly flags the **legacy-demo desync caveat** (readability refactors risk breaking bit-exact sync) — the exact tension this roadmap centers on.
- **Announcement:** [Doomworld: "Iron Doom 0.1.0 (Dec 24, 2024)"](https://www.doomworld.com/forum/topic/150233-iron-doom-010-dec-24-2024/).

**Implication:** Iron Doom is doom-rs's closest existing peer (Rust + Chocolate-based + same demo-sync tension). Because it targets the strict-Chocolate bar, "match Iron Doom" and "match Chocolate Doom" point at the **same simulation-fidelity bar** — so this roadmap serves both. Iron Doom's C→Rust mapping (Bevy ECS) and its readability-vs-bit-exactness notes are worth studying. _Flag for the user: confirm whether Iron Doom is meant as (a) the fidelity bar (same as Chocolate) or (b) a codebase to cross-reference — this roadmap assumes (a)._

**Crispy Doom** (in case it was the intended target — no evidence it is): Chocolate + optional limit-removal, hi-res (640×400+), widescreen, uncapped fps, mouselook/crosshair, full DeHackEd+BEX — all **off by default**, preserving demo/config/save/net compat. Distinction: **Chocolate = strict replica; Crispy = Chocolate + opt-in QoL; Iron Doom = Rust rewrite of the strict-Chocolate target.** If limit-removal/hi-res is ever wanted without breaking vanilla compat, Crispy is the model — see M2's opt-in framing.

Sources: [fabiangreffrath/crispy-doom](https://github.com/fabiangreffrath/crispy-doom), [Doom Wiki: Crispy Doom](https://doomwiki.org/wiki/Crispy_Doom).

## 2. Current-state inventory

Repo layout (lines approximate):

| Crate | Lines | Responsibility |
|---|---|---|
| `doom-types` | 2,766 | fixed-point, angles/BAM, bbox, limits, weapons, mobj kinds (`no_std`) |
| `doom-wad` | 1,895 | WAD parse + PWAD override stack |
| `doom-map` | 5,732 | BSP, classic + UDMF map parse, exporters |
| `doom-game` | 50,288 | all playsim: mobj, AI, weapons, specials, combat, savegame, cheats, dehacked, sound logic |
| `doom-renderer` | 30,998 | walls/flats/sprites/sky/masked, statusbar, automap, menus, intermission, wipe, fonts |
| `doom-audio` | 4,065 | OPL2 synth, MUS/MIDI, SFX mixer, spatial |
| `doom-net` | 2,916 | custom UDP relay + rollback netcode |
| `doom-demo` | 1,491 | LMP demo record/playback |
| `doom-tui` | 3,742 | terminal render/input |
| `doom-app` | 11,247 | main loop, CLI, orchestration, alt-renderer + AI-director extras |

### Subsystem status

| Subsystem | Status | Notes / evidence |
|---|---|---|
| WAD loading | **FULL** | `doom-wad/src/wad.rs` (`WadFile::parse`, IWAD/PWAD kind, `find_map_marker`), classic + UDMF lump groups; PWAD override stack (last-wins); PNAMES/TEXTURE1/2/flats/sprites consumed by renderer. |
| Renderer (software BSP) | **FULL** | `doom-renderer/src/render.rs` (`render_level*`); BSP + seg walls, visplanes (`visplane.rs`), spans, masked/transparent midtextures, sprite ordering, sky (map-specific), colormaps/lighting, palette flashes, screen melt (`wipe.rs`), animated flats/switches. _Presentation is terminal, not SDL._ |
| Playsim / thinkers | **FULL** | `Fixed16_16` FRACBITS=16 (`fixed.rs`), BAM angles; ticker/phase machine; `p_try_move`/`p_slide_move`, blockmap+intercepts (`trace.rs`, `sight.rs`); full mobj/state tables. |
| Monster AI | **FULL** | `doom-game/src/actions.rs` — all vanilla `A_*` functions incl. every monster attack and boss brain; targeting, infighting, sound propagation, ambush flags; unit-tested. |
| Map specials | **FULL** | `specials.rs` + `linedef_dispatch.rs` + `switch.rs` + `movers.rs`: doors, lifts/plats (incl. perpetual), floors/ceilings, crushers, stairs, donut, teleports, scrollers, all light effects, textured switches. |
| Player / weapons | **FULL** | health/armor/god/berserk; all 9 weapons, ammo types, switching, all `A_*` weapon actions; view bob; radius/LOS-gated damage. |
| Pickups | **FULL** | weapons/ammo/health/armor/keys/powerups/backpack; item-count stats. |
| HUD / status bar | **FULL** | `statusbar.rs` (`draw_status_bar`, digits, keys, mugshot/face, arms); messages; fullscreen + bar modes. |
| Menus | **FULL** | main/episode/skill, 6-slot load/save, options. |
| Automap | **FULL** | zoom/pan/follow, grid, thing markers, IDDT reveal. |
| Sound (SFX + music) | **PARTIAL** | Digital SFX (`cpal`) + spatial; from-scratch **OPL2 FM synth**, GENMIDI bank, MUS parser, WAV export; priority/dedup. **Missing: PC speaker (DP*/PCSND).** No native-MIDI / Timidity / GUS device options. |
| Savegames | **PARTIAL (non-vanilla)** | Custom binary format (`SAVE_VERSION=3`), works but **not byte-compatible** with Chocolate `.dsg`; a minimal `savegame_vanilla.rs` stub exists. |
| Demos | **PARTIAL** | Vanilla 13-byte LMP header (v1.9/version 109); record/playback/timedemo; deterministic capture repro. **Bit-exact sync vs real Chocolate/DOS `.lmp` is unverified.** |
| Intermission / finale | **PARTIAL** | WI stats screen + finale typed-text screen. **Missing: Doom II cast call, bunny/art end screens.** |
| Cheats | **FULL** | iddqd, idkfa, idfa, idclip, idspispopd, idbehold[vsiral], idclev, idmus, IDDT, choppers; prefix-conflict handling. |
| Multiplayer / netcode | **PARTIAL (non-vanilla)** | Custom UDP relay + rollback + CRC32 desync detect; **not** vanilla lockstep/Doomcom — will not interoperate with vanilla/Chocolate netgames. |
| Config files | **ABSENT** | No `default.cfg` / `.cfg` read or write anywhere; settings are CLI-only. |
| CLI args | **PARTIAL** | Has `--iwad`, `--pwad`×N, `--warp`, `--skill`, `--compat`, `--record`, `--playdemo`, `--timedemo`, `--deh`, net flags, exporters. **Missing: `-nomonsters`, `-respawn`, `-fast`, `-turbo`, `-episode`, `-loadgame`, `-deathmatch`/`-altdeath`, `-file`, response files.** GNU `--long` convention, not vanilla single-dash. |
| Game-mode detection | **PARTIAL (weak)** | `GameMode` enum is only SinglePlayer/Deathmatch; **no IWAD-based gamemode/gamemission auto-detection**; episode/map only from `--warp`. Doom II vs Doom 1 naming handled; Final Doom / shareware gating not auto-detected. |
| DeHackEd | **PARTIAL (strong core)** | `dehacked.rs` parses Thing/Frame/Weapon/Ammo/Misc/Sound/`[CODEPTR]`/`[STRINGS]`/Text; `--deh`. **BEX likely incomplete** (`[PARS]`, `[HELPER]`, `[SPRITES]`, BEX string mnemonics not evidenced). |
| Vanilla limits | **DIVERGENT** | `limits.rs` caps are verification bounds, often smaller than vanilla (e.g. `MAX_VISIBLE_THINGS=64` vs vanilla 128) and non-fatal; dynamic buffers make the renderer effectively limit-removing. **No overflow emulation.** |

**Beyond a plain port:** doom-rs also ships a "cogmind" ASCII/tile alt-renderer + FOV, an AI-director + GeoJSON telemetry, a map analyzer with chokepoint analysis, SVG/OBJ/GeoJSON/HTML/JSON exporters, and cargo-fuzz targets. These are orthogonal to parity.

## 3. Vanilla-fidelity reference data

Static limits Chocolate preserves (targets for M2's opt-in compat mode):

| Limit | Symbol | Vanilla value | Failure mode |
|---|---|---|---|
| Visplanes | `MAXVISPLANES` | 128 | "No more visplanes" overflow crash |
| Drawsegs | `MAXDRAWSEGS` | 256 | overflow / HOM |
| Vissprites | `MAXVISSPRITES` | 128 | sprites flicker in/out by sort order |
| Intercepts | `MAXINTERCEPTS` | 128 | overflow → UB; "all-ghosts" bug |
| Openings | `MAXOPENINGS` | 16384 | column-clip overflow |
| Active plats | `MAXPLATS` | 30 | fatal "P_AddActivePlat: no more plats!" |
| Savegame buffer | `SAVEGAMESIZE` | 180224 bytes (~180 KB) | "Savegame buffer overrun" crash |
| Demo buffer | — | ~128 KB | recording stops / exits |

Demo compatibility: Chocolate has no PrBoom `-complevel`; instead `-gameversion` selects a released DOS build (`1.666, 1.7, 1.8, 1.9, ultimate, final, final2, hacx, chex`); `-longtics` records hi-res "1.91" demos; `-shortticfix`, `-demoextend`, `-maxdemo`. Sound: OPL2/OPL3 emulation (default), MUS→MIDI, native MIDI / Timidity, GUS, PC speaker (Doom/Strife/Chex only), digital SFX with vanilla pitch shift + channel cutoff. Netplay: vanilla-compatible peer lockstep + dedicated `chocolate-server`, drone/spy modes. Config: DOS `default.cfg` + `chocolate-doom.cfg`; full vanilla arg set + `@file` response files.

Sources: [Static limits](https://doomwiki.org/wiki/Static_limits), [Compatibility](https://www.chocolate-doom.org/wiki/index.php/Compatibility), [Command line arguments](https://www.chocolate-doom.org/wiki/index.php/Command_line_arguments), [README.Music.md](https://github.com/chocolate-doom/chocolate-doom/blob/master/README.Music.md).

## 4. Gap list grouped by subsystem

**Simulation correctness (the parity core)**
- Verify `M_Random` uses the exact `rndtable[256]` values **and** exact call order/count; ensure UI/menu randomness never advances the game index.
- Prove tic-by-tic demo determinism against a corpus of real DOS/Chocolate `.lmp` demos.
- Implement `-gameversion`-style behavior selection for the subtle per-version playsim differences.

**Vanilla limits & overflow (opt-in "vanilla-compat" mode)**
- Reintroduce fixed-size buffers with vanilla caps and reproduce the overflow *consequences* (visplane crash, tutti-frutti, medusa, all-ghosts/intercepts, spechit, "no more plats", savegame-buffer overrun). Gate behind a compat toggle so the default limit-removing behavior stays available (the Crispy model).

**Compatibility surfaces**
- Config: read/write DOS `default.cfg` + a `chocolate-doom.cfg`-equivalent; map settings to CLI overrides.
- Savegames: optional vanilla `.dsg` read/write under the ~180 KB buffer semantics.
- Netcode: a vanilla lockstep mode + dedicated-server, distinct from the existing rollback relay.
- CLI: add the missing vanilla flags and a vanilla single-dash parsing mode + `@file` response files.
- Game-mode/mission auto-detection from IWAD lumps (shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom), with correct lump gating.

**Sound**
- PC speaker (DP* lumps) emulation; optional native-MIDI/Timidity/GUS music device selection.

**Content polish**
- Doom II cast call (`F_CastPrint`/castorder); finale art/bunny screens.
- DeHackEd BEX completeness (`[PARS]`, `[HELPER]`, `[SPRITES]`, BEX string mnemonics).

**Explicit non-goals (terminal port)**
- Pixel-identical SDL display output and DOS input-timing "feel" — precluded by the terminal presentation layer. If ever desired, they require an SDL/GPU presentation backend, tracked separately from parity.

## 5. Prioritized milestone plan

Effort is expressed as relative size — **S** (small/localized), **M** (moderate), **L** (large/cross-cutting), **XL** (very large / research-heavy) — not calendar time.

### M1 — Playsim correctness + demo playback _(XL)_ — foundation, do first
Everything else rides on deterministic simulation. Verify `rndtable[256]` and M_Random call order; build a demo-sync regression harness that plays a corpus of known DOS/Chocolate `.lmp`s under `-timedemo` and asserts identical end-state (tic count, checksums, player position/kills). Add `-gameversion`-equivalent behavior switches for per-version differences. **Acceptance:** a curated `.lmp` corpus (incl. Compet-N demos) plays to completion without desync. This is the single highest-value milestone and the hardest.

### M2 — Vanilla limits & overflow emulation _(L)_ — opt-in compat mode
Add a `vanilla-compat` toggle that swaps the dynamic clip/plane/sprite buffers for fixed vanilla-sized arrays and reproduces overflow consequences (see §3 table). Chocorenderlimits-style counters make good test oracles. Keep limit-removing as the default. **Acceptance:** known limit-breaking maps overflow at the same points as Chocolate.

### M3 — Game-mode / IWAD auto-detection _(M)_
Identify game/mission from IWAD lumps and set correct episode/level structure, sky, finale text, and lump gating (shareware/registered/Ultimate/Doom II/Plutonia/TNT/Chex/HACX/FreeDoom). Unblocks correct content selection for M1's demo corpus and M5's CLI. **Acceptance:** each supported IWAD boots into the right game mode with correct maps/skies/text.

### M4 — Config file handling _(M)_
Read/write DOS `default.cfg` + a `chocolate-doom.cfg`-equivalent; wire settings through to the engine with CLI overrides. **Acceptance:** a vanilla `default.cfg` is read and round-tripped; key bindings and sound/video settings apply.

### M5 — CLI arg parity _(M)_
Add `-nomonsters`, `-respawn`, `-fast`, `-turbo`, `-episode`, `-loadgame`, `-deathmatch`/`-altdeath`, `-file`, and `@file` response files; add a vanilla single-dash parsing mode alongside the existing GNU `--long` flags. **Acceptance:** the standard vanilla command lines launch equivalent sessions.

### M6 — Sound completeness _(M)_
PC speaker (DP* lump) emulation; optional native-MIDI/Timidity/GUS device selection to complement the existing OPL2 synth. **Acceptance:** PC-speaker mode produces the vanilla beeps; music device is selectable.

### M7 — Vanilla savegame format _(M)_
Optional `.dsg` read/write matching the vanilla layout and ~180 KB buffer semantics, alongside the existing custom format. **Acceptance:** a Chocolate-written `.dsg` loads in doom-rs and vice-versa.

### M8 — Vanilla netplay _(L)_
A vanilla-compatible peer-lockstep mode + dedicated-server, distinct from the existing rollback relay. Depends on M1's determinism. **Acceptance:** doom-rs syncs a netgame using the lockstep model (interop with Chocolate is a stretch goal).

### M9 — Content polish _(S–M)_
Doom II cast call, finale art/bunny screens, and DeHackEd BEX completeness (`[PARS]`, `[HELPER]`, `[SPRITES]`, string mnemonics). **Acceptance:** Doom II ending plays the cast call; BEX patches apply fully.

### Sequencing rationale
M1 is the bedrock — demo sync validates the entire playsim and is prerequisite for M8. M2 is next because limit/overflow behavior is the sharpest *observable* divergence and shares test infrastructure with M1. M3 unblocks correct content for demos and CLI. M4/M5 are the compatibility surface users touch first and are low-risk. M6/M7 are self-contained. M8 depends on M1. M9 is polish and can slot in opportunistically.

## 6. Open questions for the user
1. **Iron Doom intent** — is Iron Doom meant as the fidelity bar (same as Chocolate, this roadmap's assumption) or as a Rust codebase to cross-reference for the C→Rust mapping?
2. **Terminal vs SDL** — is simulation-level parity the accepted definition of "parity" here, or is a pixel-accurate SDL/GPU presentation backend also in scope (a large separate effort)?
3. **Limit-removing default** — keep doom-rs's limit-removing behavior as default with vanilla limits opt-in (Crispy model), or make strict-vanilla the default?
