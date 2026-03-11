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
| Input and main tic loop | `d_event.c`, `d_loop.c`, `i_input.c`, `p_user.c` | `crates/doom-tui/src/input.rs`, `crates/doom-tui/src/event_loop.rs`, `crates/doom-game/src/tic.rs` | Partial | Batch D landed deterministic held-fire cadence and kept rocket/BFG release-gated. Desktop mouse should wait for a non-TUI frontend. Deeper demo/input parity is still unaudited. |
| Player use, doors, linedef specials | `p_spec.c`, `p_map.c` | `crates/doom-game/src/specials.rs`, `crates/doom-game/src/trace.rs`, `crates/doom-game/src/linedef_dispatch.rs`, `crates/doom-game/src/tic.rs` | Partial | Batch A2 is green: locked doors now emit player-only keyed feedback and walk-trigger processing moved back into the game tick with deterministic reverse-crossing order. Remaining debt is the exact pathological `spechit` encounter order, not the old app-wrapper or lump-order behavior. |
| Monster movement, sight, sound, door opening | `p_enemy.c`, `p_sight.c`, `p_map.c` | `crates/doom-game/src/actions.rs`, `crates/doom-game/src/sight.rs`, `crates/doom-game/src/sound.rs` | Partial | Batch B restored behind-the-back wakeup rules, one-soundblock propagation, retaliation, and Doom-shaped missile gating. B2 started by fixing the vanilla Arch-Vile and Revenant missile-range edge cases; elevated-target hitscan parity is still open. |
| Spawn and thing placement | `p_mobj.c` | `crates/doom-game/src/spawn.rs`, `crates/doom-game/src/tic.rs` | Partial | Batch B now honors `MF_SPAWNCEILING`, randomizes positive spawn tics, and blocks invalid Nightmare respawns. Remaining audit item: broader map-thing spawn parity and any remaining flag-specific edge cases. |
| Weapons, hitscan, damage | `p_pspr.c`, `p_map.c`, `p_inter.c` | `crates/doom-game/src/weapon_fire.rs`, `crates/doom-game/src/combat.rs`, `crates/doom-game/src/tic.rs` | Partial | Broken fixed-point aim rays are fixed, first pistol/chaingun shots are now accurate, fist snap-to-target matches chainsaw behavior, and Batch D landed held-fire/refire cadence in the tic loop. Remaining audit item: elevated-target autoaim and deeper damage-table parity. |
| BSP, seg traversal, wall rendering | `r_bsp.c`, `r_segs.c` | `crates/doom-renderer/src/seg.rs`, `crates/doom-renderer/src/render.rs` | Partial | Batch C1 is green: pegging now uses logical texture height and masked midtextures are deferred instead of being painted inline. Remaining renderer debt is deeper seg/visplane/sky projection parity. |
| Planes and sky | `r_plane.c`, `r_sky.c` | `crates/doom-renderer/src/visplane.rs`, `crates/doom-renderer/src/sky.rs`, `crates/doom-renderer/src/render.rs` | Partial | Disjoint sky-span bugs are fixed and map-specific sky selection now resolves from the level name. Remaining audit item: deeper visplane parity and sky vertical mapping. |
| Sprites and clipping | `r_things.c` | `crates/doom-renderer/src/sprite.rs`, `crates/doom-renderer/src/sprite_lookup.rs` | Partial | World-space sprite anchoring and portal clip ordering are much better, and masked midtextures now depth-sort with sprites. Remaining audit item: residual edge cases where clip state is borrowed from the wrong sector context. |
| Audio and music | `s_sound.c`, `i_sound.c`, `mus2mid` path | `crates/doom-app/src/audio_system.rs`, `crates/doom-app/src/main.rs` | Partial | Batch D moved player weapon SFX onto actual fire events, re-resolves map music from current level state, and removed the fake timer-driven demo loop. Remaining audit work is deeper sound-origin/channel parity and true attract-mode demo playback. |
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

- `p_use_lines()` itself is in good shape now, but pathological same-block multi-trigger movement still uses a reverse-crossing approximation of vanilla `spechit` processing rather than the original blockmap encounter order.
- If a real map surfaces a remaining discrepancy here, the next escalation is to collect crossed special lines during `p_try_move()` itself instead of reconstructing them after the move.

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

- Status: Batch A and Batch A2 complete, one narrow `spechit` caveat still open
- Chocolate Doom sources: `p_spec.c`, `p_map.c`
- Local Rust files: `crates/doom-game/src/specials.rs`, `crates/doom-game/src/trace.rs`, `crates/doom-game/src/linedef_dispatch.rs`, `crates/doom-game/src/movement.rs`, `crates/doom-game/src/tic.rs`, `crates/doom-app/src/main.rs`
- Confirmed parity wins:
  - `p_use_lines()` now stops on a closed ordinary blocker instead of tunneling to a special behind it.
  - Back-side player use now fails for front-only use specials while manual door specials still work from the back side like Doom.
  - Pressing use into a closed ordinary blocker now queues the blocked-use fail sound request.
  - Monster movement reuses the exact failed-move blocking linedef when trying to open a door.
  - Locked doors now emit player-only `sfx_oof` keyed-door feedback plus the classic "You need a <color> key to open this door" message, matching `EV_DoLockedDoor`.
  - Player crossed-line specials now fire during the game tick instead of from the app wrapper after `tick()`.
  - Crossed walk specials now resolve in reverse crossing order instead of raw linedef order, which is a closer match to vanilla `spechit` processing.
- Confirmed mismatches:
  - Exact vanilla `spechit` order still depends on blockmap encounter order, while the current port uses a deterministic reverse-crossing approximation after a successful move.
- Recommended regressions:
  - Same-block multi-trigger torture case once we are ready to thread actual `spechit` collection through `p_try_move()`.
- Recommended fix batch: Batch A

### System 2: Monsters, combat, and spawn

- Status: Batch B complete for the planned scope, deeper combat parity still open
- Chocolate Doom sources: `p_enemy.c`, `p_sight.c`, `p_mobj.c`, `p_inter.c`, `p_pspr.c`
- Local Rust files: `crates/doom-game/src/actions.rs`, `crates/doom-game/src/sight.rs`, `crates/doom-game/src/sound.rs`, `crates/doom-game/src/spawn.rs`, `crates/doom-game/src/combat.rs`, `crates/doom-game/src/weapon_fire.rs`, `crates/doom-game/src/tic.rs`
- Confirmed parity wins:
  - `p_look_for_players()` now respects Doom's behind-the-back restriction outside melee range.
  - `p_noise_alert()` now crosses one `ML_SOUNDBLOCK`, not two, and the mixed-topology regression matches that rule.
  - Monster retaliation now sets `MF_JUSTHIT` and threshold state so damaged monsters can immediately choose the missile path Doom allows.
  - `A_Chase` now routes missile decisions through a Doom-shaped range gate instead of the old pure `movecount <= 0 && LOS` shortcut.
  - `P_CheckMissileRange` now respects the vanilla Arch-Vile maximum range and Revenant minimum missile range instead of using the old flattened monster grouping.
  - Spawn sync honors `MF_SPAWNCEILING`, non-Nightmare map-thing spawns randomize positive tics, and blocked Nightmare respawns now fail cleanly.
  - Pistol and chaingun now get accurate first shots after release, and fist hits snap the player toward the struck target like chainsaw hits.
- Confirmed mismatches:
  - Hitscan is still effectively 2D, so elevated-target autoaim and bullet-slope behavior remain open.
  - The remaining `P_CheckMissileRange` debt is the broader source-to-source cleanup pass, not the obvious monster-specific edge cases.
- Recommended regressions:
  - Seeded `A_Chase` retaliation and threshold test after damaging a monster mid-chase.
  - Monster-facing-away visual acquisition test where vanilla Doom would keep it idle.
  - Two-soundblock wakeup test: noise should cross one blocker but not two.
  - Ceiling-spawn, initial-tics, and blocked Nightmare respawn tests.
  - Elevated-target pistol autoaim and broader held-fire/refire tests.
- Recommended fix batch: Batch B

## Proposed Fix Order

1. Batch A: Gameplay interactions
   Reason: it affects doors, use semantics, and blocking behavior directly under the player's hands.
2. Batch C1: Renderer user-facing lies
   Status: complete on 2026-03-11.
   Scope landed: masked midtextures, logical texture height pegging, and map-specific sky selection.
3. Batch B: Monsters, combat, and spawn
   Status: complete on 2026-03-11 for the planned scope.
   Scope landed: wakeup and sound parity, retaliation and missile gating, spawn semantics, first-shot accuracy, and fist snap-to-target.
4. Batch D: Frontend and audio
   Status: complete on 2026-03-11 for the planned scope.
   Scope landed: held-fire cadence, actual fire-driven player weapon SFX, map-music re-resolution, and title-loop demo sanity.
5. Batch C2 and System 5
   Status: Batch C2 landed on 2026-03-11 for sky projection, visplane reuse, and renderer-side sector ownership hardening.
   Remaining debt: raw subsector-order parity still needs deeper renderer work before the hardening sort in `seg.rs` can be removed safely.
   Reason: save/demo/RNG parity still waits until the louder renderer and gameplay regressions stop moving.

### System 3: Renderer

- Status: Batch C1 complete, Batch C2 mostly complete with one explicit order-parity item still open
- Chocolate Doom sources: `r_bsp.c`, `r_segs.c`, `r_plane.c`, `r_sky.c`, `r_things.c`
- Local Rust files: `crates/doom-renderer/src/render.rs`, `crates/doom-renderer/src/seg.rs`, `crates/doom-renderer/src/sky.rs`, `crates/doom-renderer/src/sprite.rs`, `crates/doom-renderer/src/sprite_lookup.rs`, `crates/doom-renderer/src/texture.rs`, `crates/doom-renderer/src/visplane.rs`
- Confirmed parity wins:
  - Masked midtextures no longer draw inline with solid walls; they are deferred and depth-sorted against sprites in a separate masked pass.
  - Pegging math now distinguishes logical texture height from padded cache height for upper, masked, and one-sided wall paths.
  - Sky selection now resolves from the map name instead of rendering `SKY1` everywhere.
  - Sky vertical mapping now follows Doom-style `skytexturemid` behavior instead of stretching the upper half of the screen and clamping the lower half.
  - `r_check_plane` now behaves like Doom's current-plane allocator instead of reusing arbitrary same-key sibling planes after a conflict.
  - Renderer-side player sector lookup now prefers map-owned subsector sector resolution and only falls back to the older heuristic for malformed or synthetic no-BSP scenes.
- Confirmed mismatches:
  - Subsector segs are still explicitly resorted nearest-first, which diverges from Doom's subsector processing order.
  - Synthetic same-subsector portal torture scenes still rely on that hardening sort; removing it currently reintroduces over-clip or leak regressions.
- Regressions landed in Batch C1:
  - Midtexture transparency plus sprite-behind-mask ordering test.
  - Non-power-of-two pegging regression for upper and masked textures.
  - Map-specific sky selection regression using distinct `SKY1` and `SKY2`.
- Regressions landed in Batch C2:
  - Sky row-sampling regressions for full-screen horizon-relative vertical mapping.
  - Visplane allocator regressions for overlap reuse and fresh-plane allocation after conflicts.
  - Player-sector regression where map-owned subsector sector beats the older nearest-seg heuristic.
  - Hardening regressions for swapped same-subsector far-wall and far-portal portal-window cases.
- Recommended regressions for Batch C2:
  - Subsector seg-order regression across multiple view angles once the hardening sort can actually be removed without reopening clip bugs.
- Batch priority:
  - Batch C1: masked midtextures, logical texture height for pegging, map-specific sky selection.
  - Batch C2: sky projection, visplane reuse, and player-sector ownership hardening.
  - Remaining renderer debt after Batch C2: subsector raw-order parity.
- Recommended fix batch: Batch C

### System 4: Frontend, tic loop, and audio

- Status: Batch D complete for the planned scope, deeper mixer/demo parity still open
- Chocolate Doom sources: `d_event.c`, `d_loop.c`, `i_input.c`, `p_user.c`, `s_sound.c`
- Local Rust files: `crates/doom-tui/src/input.rs`, `crates/doom-tui/src/event_loop.rs`, `crates/doom-game/src/tic.rs`, `crates/doom-app/src/audio_system.rs`, `crates/doom-app/src/main.rs`
- Confirmed parity wins:
  - `Ctrl` attack works again.
  - Held pistol and shotgun now refire on hold after deterministic cooldowns, while rocket launcher and BFG stay release-gated.
  - Player weapon SFX now come from actual weapon fire events instead of raw attack-button edges.
  - Map music now re-resolves from `GameState.level_name` and restarts at gameplay entry/load points.
  - The timer-driven title loop no longer walks into blank `Demo(_)` phases when no playback path exists.
- Confirmed mismatches:
  - Sound origin and channel behavior are much simpler than Doom's source-aware `S_StartSound` behavior.
  - True attract-mode demo playback still does not exist; we now stay out of fake demo phases instead of pretending otherwise.
  - Weapon cadence is deterministic and much closer, but it is still a simplified cooldown model rather than a full psprite state machine.
- Recommended regressions:
  - Held chaingun and plasma cadence should be tightened against Chocolate Doom's psprite timing rather than the current simplified cooldown table.
  - Repeated sound events from the same origin should reuse/update channels in a Doom-shaped way.
  - Real title demo playback should replace the current `Title <-> Credits` fallback loop.
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
