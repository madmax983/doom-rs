# Unofficial Doom Specs Parity Matrix

Date: 2026-04-06

Goal: compare the current `doom-rs` implementation against Matthew Fell's Unofficial Doom Specs v1.666, identify confirmed drift, and separate true incompatibilities from deliberate extensions.

Parity claims in this matrix refer to behavior reachable under
`CompatibilityProfile::VanillaStrict` unless noted otherwise. The default
`Extended` profile deliberately preserves source-port-style conveniences where
that does not interfere with having a strict path.

## Primary Sources

- Unofficial Doom Specs v1.666: <https://www.gamers.org/dhs/helpdocs/dmsp1666.html>
- Alternate mirror: <https://www.gamers.org/docs/FAQ/DOOM.FAQ.Specs.html>
- DoomWiki demo reference: <https://doomwiki.org/wiki/Demo>
- DoomWiki tic reference: <https://doomwiki.org/wiki/Tic>
- Existing local parity notes:
  - `docs/plans/2026-03-10-chocolate-doom-parity-audit.md`
  - `docs/plans/2026-03-13-parity-closeout-and-remaining-work.md`
  - `docs/plans/vantage-spec-demo-parity.md`
  - `docs/plans/vantage-spec-savegame-parity.md`
  - `docs/plans/vantage-spec-rng-parity.md`

## Scope

This matrix is strongest where UDS is strongest:

- WAD header and directory layout
- classic map lump layout and field sizes
- picture, flat, texture, palette, and colormap resource formats
- sound and music lump formats
- demo file structure

This matrix is intentionally weaker where UDS is incomplete or not the best authority:

- savegame internals
- gameplay logic parity
- BSP traversal and visplane behavior
- full demo determinism beyond the file format itself

For those areas, Chocolate Doom or original source behavior remains the better benchmark.

## Status Legend

- `Aligned`: matches UDS closely enough to treat as spec-conformant
- `Partial`: core format matches, but some behavior or edge cases drift
- `Drifted`: incompatible with vanilla/UDS behavior in a user-visible way
- `Extended`: deliberate feature beyond UDS/vanilla scope
- `Unknown`: not enough evidence yet

## Matrix

| Area | UDS Section | Local Modules | Status | Notes |
| --- | --- | --- | --- | --- |
| WAD header and directory | [2], [A-1] | `crates/doom-wad/src/wad.rs` | Aligned | Parses `IWAD`/`PWAD`, validates `numlumps`, directory offset, and lump bounds. |
| Duplicate lump lookup within a WAD | [2-1] | `crates/doom-wad/src/wad.rs` | Aligned | Reverse search gives last-defined-wins behavior, matching Doom's practical lookup rules. |
| PWAD stack precedence | [2-1] | `crates/doom-wad/src/stack.rs` | Aligned | `VanillaStrict` preserves vanilla-era sprite/flat loading boundaries; `Extended` keeps last-loaded-wins convenience by design. |
| Classic binary map grouping | [4] | `crates/doom-wad/src/wad.rs` | Aligned | Requires marker plus the classic 10 lump sequence in spec order. |
| THINGS/LINEDEFS/SIDEDEFS/VERTEXES/SEGS/SSECTORS/NODES/SECTORS/REJECT/BLOCKMAP parsing | [4-2]..[4-11] | `crates/doom-map/src/lumps.rs`, `crates/doom-map/src/level.rs` | Aligned | Field widths and record sizes line up with UDS; local validation is stricter than vanilla in useful ways. |
| Picture and patch format | [5-1] | `crates/doom-renderer/src/texture.rs`, `crates/doom-renderer/src/texture_compose.rs`, `crates/doom-renderer/src/sprite.rs` | Aligned | Parses width, height, offsets, column offsets, and post streams in the classic Doom picture format. |
| Flats between markers | [6], [2-1] | `crates/doom-renderer/src/flat_cache.rs` | Aligned | `VanillaStrict` ignores stacked `FF_START`/`FF_END` overrides; `Extended` keeps the modern stacked path. |
| Sprites between markers | [3], [5], [2-1] | `crates/doom-renderer/src/sprite.rs` | Aligned | `VanillaStrict` ignores stacked `SS_START`/`SS_END` overrides; `Extended` keeps the modern stacked path. |
| PLAYPAL | [8-1] | `crates/doom-renderer/src/palette.rs` | Aligned | Enforces the canonical 14 x 256 x 3 = 10752 byte layout. |
| COLORMAP normal lighting rows | [8-2] | `crates/doom-renderer/src/colormap.rs` | Aligned | The lump size and 34-row shape are correct, and the strict special-row path no longer pollutes normal lighting access. |
| COLORMAP special rows | [8-2] | `crates/doom-renderer/src/colormap.rs` | Aligned | `VanillaStrict` uses row 32 directly for invulnerability; `Extended` intentionally retains the synthetic grayscale fallback. |
| Soundcard SFX lumps | [7-2] | `crates/doom-audio/src/mixer.rs`, `crates/doom-audio/src/sfx.rs` | Aligned | The classic 8-byte header model and sample loading path match the spec shape. |
| MUS music and GENMIDI | [7-3], [7-4] | `crates/doom-audio/src/mus.rs`, `crates/doom-audio/src/midi.rs` | Aligned | Format support is spec-shaped and adequate for vanilla data loading. |
| Demo header and tic stream | [8-6] | `crates/doom-demo/src/header.rs`, `crates/doom-demo/src/ticcmd.rs`, `crates/doom-demo/src/player.rs`, `crates/doom-app/src/demo_mode.rs` | Aligned | The strict path now uses vanilla four-byte tic payloads and preserves action buttons end-to-end in app playback tests. |
| Savegame files | [9] | `crates/doom-game/src/savegame.rs` | Partial | `DoomRs` remains the only fully implemented payload format, but the engine now distinguishes `DoomRs` from plausible vanilla DSG headers and fails explicitly instead of pretending custom saves are vanilla-compatible. |
| Core RNG table | spec-adjacent | `crates/doom-game/src/state.rs`, `crates/doom-game/src/random.rs` | Aligned | The 256-byte lookup table and modulo-256 index stepping match Doom's known RNG core. |
| Full gameplay RNG consumption order | source-behavior, not UDS | gameplay modules across `doom-game` | Unknown | Core RNG is right, but this matrix does not prove every call site consumes values in source-faithful order. |
| UDMF `TEXTMAP` support | outside UDS | `crates/doom-wad/src/wad.rs`, `crates/doom-map/src/udmf.rs`, `crates/doom-map/src/level.rs` | Extended | Intentional modern extension; should be treated as non-vanilla capability, not parity debt. |

## Remaining High-Confidence Drift

### 1. Savegames are still not vanilla payload-compatible

`crates/doom-game/src/savegame.rs` still treats `DoomRs` as the only fully
implemented payload format. The new `SaveFormat::VanillaDsg` path is honest
about the boundary, but it is still a boundary:

- vanilla header detection exists
- strict mode no longer silently writes or loads `DRS1`
- full vanilla `.dsg` payload read/write is still unimplemented

That is progress, but not full parity.

## Confirmed Alignment

These areas look genuinely good against UDS:

- WAD header parsing and directory bounds validation
- classic 10-lump map grouping
- raw binary map lump field parsing
- picture/patch parsing
- PLAYPAL sizing and palette count
- normal and special COLORMAP row handling in `VanillaStrict`
- MUS and GENMIDI support
- strict sprite/flat loading semantics
- vanilla demo tic byte semantics and action-bit preservation
- the canonical RNG table and table-walk logic

## Out Of Scope For This Matrix

These are important parity areas, but UDS is not the right primary source:

- monster AI and combat sequencing
- movement, specials, and trigger order
- BSP traversal, visplanes, and clipping behavior
- timing and long-run demo determinism

Those should continue to be audited against Chocolate Doom and original-source behavior rather than UDS alone.

## Verification Notes

Commands run during this audit:

- `cargo test -p doom-wad --lib`
- `cargo test -p doom-demo`
- `cargo test -p doom-renderer --lib`
- `cargo test -p doom-game savegame --lib`
- `cargo test -p doom-app`

Observed results:

- all of the above passed on this branch after the remediation tasks landed
- the WAD crate test harness is no longer blocked

## Recommended Next Actions

Implementation plan: `docs/plans/2026-04-06-uds-drift-remediation-plan.md`

1. Finish real vanilla DSG payload read/write if strict savegame interoperability still matters.
2. Prove long-run gameplay determinism against Chocolate Doom demos instead of stopping at byte-format parity.
3. Keep UDMF support explicitly documented as an extension so it does not get mistaken for parity debt.
