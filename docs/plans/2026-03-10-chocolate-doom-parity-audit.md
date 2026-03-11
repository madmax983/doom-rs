# Chocolate Doom Parity Audit

Date: 2026-03-10

Goal: compare the current Rust port against Chocolate Doom subsystem by subsystem, turn each mismatch into a concrete regression, and fix root causes instead of accumulating folklore.

## Primary Sources

- Repo root: <https://github.com/chocolate-doom/chocolate-doom>
- Source tree: <https://github.com/chocolate-doom/chocolate-doom/tree/master/src>
- Gameplay use/specials: <https://github.com/chocolate-doom/chocolate-doom/blob/master/src/doom/p_spec.c#L366-L393>
- Gameplay use entry point: <https://github.com/chocolate-doom/chocolate-doom/blob/master/src/doom/p_spec.c#L622-L641>
- Monster AI: <https://raw.githubusercontent.com/chocolate-doom/chocolate-doom/master/src/doom/p_enemy.c>
- Map movement and openings: <https://github.com/chocolate-doom/chocolate-doom/blob/master/src/doom/p_map.c>
- Wall rendering and pegging: <https://github.com/chocolate-doom/chocolate-doom/blob/master/src/doom/r_segs.c#L623-L684>

## Audit Method

1. Map each Chocolate Doom source file to the local Rust modules that own the same behavior.
2. Write down known simplifications and observed regressions.
3. Add one regression per high-confidence mismatch.
4. Patch the root cause, then repeat with the next slice.

## Batch Strategy

- Batch A: Gameplay interactions
  Scope: player use, doors, linedef activation, blockers, movement-trigger interactions.
- Batch B: Monsters, combat, spawn
  Scope: chase logic, wakeup/sound propagation, attacks, damage, spawn placement.
- Batch C: Renderer
  Scope: wall pegging, BSP/seg traversal, visplanes, sky, sprite clipping.
- Batch D: Frontend and meta systems
  Scope: input semantics, tic cadence, audio/music transitions, accepted non-parity frontend choices.

## System Log Template

Each subsystem log should record:

- Chocolate Doom source files
- Local Rust files
- Confirmed parity wins
- Confirmed mismatches
- Likely user-visible symptoms
- Recommended regression tests
- Target fix batch

## Matrix

| Subsystem | Chocolate Doom | Local Rust Modules | Status | Notes |
| --- | --- | --- | --- | --- |
| Input and main tic loop | `d_event.c`, `d_loop.c`, `i_input.c`, `p_user.c` | `crates/doom-tui/src/input.rs`, `crates/doom-tui/src/event_loop.rs`, `crates/doom-game/src/tic.rs` | Partial | `Ctrl` attack is fixed. Desktop mouse should wait for a non-TUI frontend. Demo/input parity still unaudited. |
| Player use, doors, linedef specials | `p_spec.c`, `p_map.c` | `crates/doom-game/src/specials.rs`, `crates/doom-game/src/trace.rs`, `crates/doom-game/src/linedef_dispatch.rs` | Partial | Batch A is green: `USE` stops on ordinary blockers, front/back use side is respected, and blocked-use now queues the Doom-style fail SFX. Remaining work is broader parity on interaction sequencing around adjacent triggers and locked-door nuance. |
| Monster movement, sight, sound, door opening | `p_enemy.c`, `p_sight.c`, `p_map.c` | `crates/doom-game/src/actions.rs`, `crates/doom-game/src/sight.rs`, `crates/doom-game/src/sound.rs` | Partial | Recent fixes restored firing windows, sector ownership, and exact blocker-driven door opening. Remaining audit work is the larger `A_Chase`/sound propagation pass, not the old coarse blockmap door guess. |
| Spawn and thing placement | `p_mobj.c` | `crates/doom-game/src/spawn.rs`, `crates/doom-game/src/tic.rs` | Partial | Floor snap and respawn height bugs are fixed. Remaining audit item: verify every map thing spawn path against subsector floor lookup and spawn flags. |
| Weapons, hitscan, damage | `p_pspr.c`, `p_map.c`, `p_inter.c` | `crates/doom-game/src/weapon_fire.rs`, `crates/doom-game/src/combat.rs`, `crates/doom-game/src/tic.rs` | Partial | Broken fixed-point aim rays are fixed. Remaining audit item: pellet spread, damage tables, and pain/death edge cases. |
| BSP, seg traversal, wall rendering | `r_bsp.c`, `r_segs.c` | `crates/doom-renderer/src/seg.rs`, `crates/doom-renderer/src/render.rs` | Partial | Batch C1 is green: pegging now uses logical texture height and masked midtextures are deferred instead of being painted inline. Remaining renderer debt is deeper seg/visplane/sky projection parity. |
| Planes and sky | `r_plane.c`, `r_sky.c` | `crates/doom-renderer/src/visplane.rs`, `crates/doom-renderer/src/sky.rs`, `crates/doom-renderer/src/render.rs` | Partial | Disjoint sky-span bugs are fixed and map-specific sky selection now resolves from the level name. Remaining audit item: deeper visplane parity and sky vertical mapping. |
| Sprites and clipping | `r_things.c` | `crates/doom-renderer/src/sprite.rs`, `crates/doom-renderer/src/sprite_lookup.rs` | Partial | World-space sprite anchoring and portal clip ordering are much better, and masked midtextures now depth-sort with sprites. Remaining audit item: residual edge cases where clip state is borrowed from the wrong sector context. |
| Audio and music | `s_sound.c`, `i_sound.c`, `mus2mid` path | `crates/doom-app/src/audio_system.rs`, `crates/doom-app/src/main.rs` | Partial | Level music startup regression is fixed. Need a full parity pass on sound origin, channel stealing, and map-change transitions. |
| Savegames, demos, RNG parity | `p_saveg.c`, demo system, `m_random.c` | `crates/doom-game/src/savegame.rs`, demo code, RNG paths | Not started | Leave this until core gameplay/rendering behavior stops moving. |

## First Slice Findings

### Gameplay: `USE`, doors, and blockers

Chocolate Doom behavior from `P_UseLines` and `PTR_UseTraverse`:

- Build a 64-unit trace in front of the player.
- Traverse all crossed linedefs nearest-first.
- If the trace hits a special use-trigger line, try to activate it from the correct side.
- If the trace hits a non-special closed line, stop immediately.

Local state before Batch A:

- `crates/doom-game/src/specials.rs::p_use_lines()` only collected `special != 0` linedefs.
- `crates/doom-game/src/linedef_dispatch.rs::dispatch_linedef()` still ignored `from_side`.
- We had nearest-special ordering tests, but no regression for "ordinary blocker in front of special."
- Monster door opening guessed from a blockmap cell scan instead of the real failed-move blocker.

Action taken across the initial gameplay slice and Batch A:

- Added a regression for a closed non-special wall in front of a usable door.
- Changed `p_use_lines()` to traverse all crossed lines in distance order and stop on a closed blocker even when the blocker is not special.
- Added regressions for back-side use rejection, blocked-use feedback, and exact blocking-door activation for monsters.
- Passed actual `from_side` into `dispatch_linedef()` and rejected back-side use activation except for the Doom manual door specials.
- Queued blocked-use feedback through `GameState::sound_queue` and mapped it to `DSNOWAY` in `doom-app`.
- Threaded the exact blocking linedef out of movement failure and reused it for monster door opening.

Still open:

- Audit lock-specific feedback and any remaining interaction-sequencing differences against Chocolate Doom's full `P_UseLines` path.
- Add a regression for adjacent trigger interactions where movement crossing and use traces compete in the same local space.

### Renderer: Batch C1 user-facing parity

Chocolate Doom `r_segs.c` uses the texture's logical height when computing pegged `texturemid` values and defers masked midtextures until after the solid wall pass. Our renderer previously stored wall textures with `height` equal to the padded power-of-two cache height and reused that padded value in pegging math, painted masked midtextures inline, and hardcoded sky selection to `SKY1`.

Pre-Batch C1 risks:

- Non-power-of-two upper or masked wall textures could slide vertically on doors and windows even when column sampling itself was otherwise correct.
- Grates and other masked midtextures could sort wrong against sprites because they were drawn inline before the sprite pass.
- Maps that should resolve to `SKY2` or `SKY3` still rendered `SKY1`.

Action taken in Batch C1:

- Added a logical texture height field alongside padded cache height and switched pegging math to use the logical height where Chocolate Doom uses texture height.
- Switched sky selection to `sky_texture_name(level.name.as_str())` instead of hardcoding `SKY1`.
- Deferred masked midtexture columns out of the solid wall pass and interleaved them with sprites by depth in the post pass.
- Added focused regressions for map-specific sky selection and masked midtexture ordering, plus a pegging regression that keeps the logical-height path exercised.

Still open:

- Sky vertical mapping is still simplified compared to Doom's `skytexturemid` projection.
- Subsector/seg processing and visplane reuse still need the deeper C2 audit.
- Residual sprite clipping edge cases around sector context are still renderer debt, but no longer share the masked-midtexture root cause.

## System Logs

### System 1: Gameplay interactions

- Status: Batch A complete, broader parity audit still open
- Chocolate Doom sources: `p_spec.c`, `p_map.c`
- Local Rust files: `crates/doom-game/src/specials.rs`, `crates/doom-game/src/trace.rs`, `crates/doom-game/src/linedef_dispatch.rs`, `crates/doom-game/src/movement.rs`
- Confirmed parity wins:
  - `p_use_lines()` now stops on a closed ordinary blocker instead of tunneling to a special behind it.
  - Back-side player use now fails for front-only use specials while manual door specials still work from the back side like Doom.
  - Pressing use into a closed ordinary blocker now queues the blocked-use fail sound request.
  - Monster movement reuses the exact failed-move blocking linedef when trying to open a door.
- Confirmed mismatches:
  - `p_use_lines()` now sees all crossed lines, but the broader player and monster interaction path still needs a full parity pass against Chocolate Doom's intercept traversal and movement-trigger sequencing.
  - Lock/key feedback nuance and any map-order edge cases around adjacent trigger lines remain unaudited.
- Recommended regressions:
  - Movement-trigger ordering test: crossing and use interactions near adjacent trigger lines should follow intercept distance order rather than map-order accidents.
  - Locked-door feedback test: keyed doors should match Chocolate Doom's exact fail semantics and sound/message behavior.
- Recommended fix batch: Batch A

### System 2: Monsters, combat, and spawn

- Status: findings integrated, not yet patched
- Chocolate Doom sources: `p_enemy.c`, `p_sight.c`, `p_mobj.c`, `p_inter.c`, `p_pspr.c`
- Local Rust files: `crates/doom-game/src/actions.rs`, `crates/doom-game/src/sight.rs`, `crates/doom-game/src/sound.rs`, `crates/doom-game/src/spawn.rs`, `crates/doom-game/src/combat.rs`, `crates/doom-game/src/weapon_fire.rs`, `crates/doom-game/src/tic.rs`
- Confirmed mismatches:
  - `A_Chase` is still much simpler than Doom's `P_CheckMissileRange` path and does not capture just-hit retaliation, threshold behavior, or Doom-like missile gating.
  - Visual acquisition is too eager because `A_Look` is not really using Doom's `P_LookForPlayers` behavior, including behind-the-back restrictions.
  - Sound propagation still crosses two `ML_SOUNDBLOCK` barriers instead of one.
  - Spawn initialization still misses several Doom semantics: ceiling spawns, initial tic randomization, and blocked Nightmare respawn handling.
  - Hitscan and weapon behavior still diverge from Doom's bullet-slope and refire logic, especially for elevated targets and accurate first shots.
- Recommended regressions:
  - Seeded `A_Chase` retaliation and threshold test after damaging a monster mid-chase.
  - Monster-facing-away visual acquisition test where vanilla Doom would keep it idle.
  - Two-soundblock wakeup test: noise should cross one blocker but not two.
  - Ceiling-spawn, initial-tics, and blocked Nightmare respawn tests.
  - Elevated-target pistol autoaim, accurate-first-shot pistol, and melee snap-to-target tests.
- Recommended fix batch: Batch B

## Proposed Fix Order

1. Batch A: Gameplay interactions
   Reason: it affects doors, use semantics, and blocking behavior directly under the player's hands.
2. Batch C1: Renderer user-facing lies
   Status: complete on 2026-03-11.
   Scope landed: masked midtextures, logical texture height pegging, and map-specific sky selection.
3. Batch B: Monsters, combat, and spawn
   Reason: this is the main source of "the boys are weird again" behavior once doors and visuals stop lying.
4. Batch D: Frontend and audio
   Reason: held-fire semantics and fire-SFX need to move in lockstep, and music transitions should be fixed before demo/title work.
5. Batch C2 and System 5
   Reason: deeper renderer structure and save/demo/RNG parity should wait until the louder regressions stop moving.

### System 3: Renderer

- Status: Batch C1 complete, Batch C2 still open
- Chocolate Doom sources: `r_bsp.c`, `r_segs.c`, `r_plane.c`, `r_sky.c`, `r_things.c`
- Local Rust files: `crates/doom-renderer/src/render.rs`, `crates/doom-renderer/src/seg.rs`, `crates/doom-renderer/src/sky.rs`, `crates/doom-renderer/src/sprite.rs`, `crates/doom-renderer/src/sprite_lookup.rs`, `crates/doom-renderer/src/texture.rs`, `crates/doom-renderer/src/visplane.rs`
- Confirmed parity wins:
  - Masked midtextures no longer draw inline with solid walls; they are deferred and depth-sorted against sprites in a separate masked pass.
  - Pegging math now distinguishes logical texture height from padded cache height for upper, masked, and one-sided wall paths.
  - Sky selection now resolves from the map name instead of rendering `SKY1` everywhere.
- Confirmed mismatches:
  - Sky vertical mapping is still simplified and not driven by Doom's `skytexturemid` projection.
  - Subsector segs are still explicitly resorted nearest-first, which diverges from Doom's subsector processing order.
  - `r_check_plane` appears more eager to split visplanes than Chocolate Doom's `R_CheckPlane`.
  - Renderer-side sector ownership still needs scrutiny in synthetic or mixed-subsector edge cases, even though recent hardening fixed a failing regression.
- Regressions landed in Batch C1:
  - Midtexture transparency plus sprite-behind-mask ordering test.
  - Non-power-of-two pegging regression for upper and masked textures.
  - Map-specific sky selection regression using distinct `SKY1` and `SKY2`.
- Recommended regressions for Batch C2:
  - Sky row-sampling regression for vertical mapping.
  - Subsector seg-order regression across multiple view angles.
  - Visplane reuse regression where overlapping ranges are still empty.
- Batch priority:
  - Batch C1: masked midtextures, logical texture height for pegging, map-specific sky selection.
  - Batch C2: sky projection, subsector order, visplane reuse.
- Recommended fix batch: Batch C

### System 4: Frontend, tic loop, and audio

- Status: findings integrated, not yet patched
- Chocolate Doom sources: `d_event.c`, `d_loop.c`, `i_input.c`, `p_user.c`, `s_sound.c`
- Local Rust files: `crates/doom-tui/src/input.rs`, `crates/doom-tui/src/event_loop.rs`, `crates/doom-game/src/tic.rs`, `crates/doom-app/src/audio_system.rs`, `crates/doom-app/src/main.rs`
- Confirmed parity wins:
  - `Ctrl` attack works again.
  - map music startup was restored.
- Confirmed mismatches:
  - Held-fire semantics are still non-vanilla for several weapons; only a subset currently refire while the button is held.
  - Weapon SFX are keyed off attack-button edge instead of actual weapon fire events.
  - Map music is still too tied to startup state and not fully re-resolved on map or phase transitions.
  - Sound origin and channel behavior are much simpler than Doom's source-aware `S_StartSound` behavior.
  - The title/demo attract loop advances state, but there is no real demo playback path behind it.
- Recommended regressions:
  - Held pistol and shotgun should continue firing over multiple tics without releasing `BT_ATTACK`.
  - Held chaingun or plasma should emit repeated fire-SFX requests, not a single startup sound.
  - Map change or quick-load to a different map should request different music.
  - Repeated sound events from the same origin should reuse/update channels in a Doom-shaped way.
  - Title mode should either play demos or stay out of demo phases until playback exists.
- Known intentional divergence:
  - TUI frontend constraints mean terminal mouse is not a Doom-parity target.
- Accepted non-parity:
  - Modernized TUI bindings such as `WASD`, `E`/Space for use, and `Q` to quit.
- Recommended fix batch: Batch D

### System 5: Savegames, demos, and RNG

- Status: not started
- Chocolate Doom sources: `p_saveg.c`, demo system, `m_random.c`
- Local Rust files: `crates/doom-game/src/savegame.rs`, demo paths, RNG paths
- Notes:
  - This slice should stay behind the gameplay and renderer batches until core behavior stops moving.
  - Once we audit it, we should separate "behavioral parity required" from "modern convenience accepted."
- Recommended fix batch: later, after Batches A-C stabilize
