//! Core simulation tick — `GameState::tick()`.
//!
//! Called exactly once per tic (35x/sec by the event loop).
//! Applies player input via `P_MovePlayer`, then runs the thinker loop
//! to advance all actor state machines by one step.
//!
//! # Unified thinker/ticker loop
//!
//! `tick_world` is the master per-tic function that processes all game objects:
//! 1. `tick_all_mobjs` — advance every actor's state machine
//! 2. Sector movers (doors, ceilings, floors, lifts, platforms)
//! 3. Light effects, scrollers, conveyors
//! 4. Projectile movement
//!
//! `p_set_mobj_state` is the canonical state transition function: it sets the
//! new state, loads tics from the STATES table, and fires the entry action.
//!
//! # Player movement (P_MovePlayer / P_Thrust)
//!
//! The original Doom movement math:
//! ```text
//! mo->angle += cmd->angleturn << 16;          // 16-bit -> 32-bit BAM
//! if (cmd->forwardmove)
//!     P_Thrust(player, mo->angle, cmd->forwardmove * 2048);
//! if (cmd->sidemove)
//!     P_Thrust(player, mo->angle - ANG90, cmd->sidemove * 2048);
//! // P_Thrust:
//!     mo->momx += FixedMul(move, finecosine[angle >> ANGLETOFINESHIFT]);
//!     mo->momy += FixedMul(move, finesine[angle >> ANGLETOFINESHIFT]);
//! ```

use doom_map::Level;
use doom_types::mobj_kind::MobjKind;
use doom_types::{ANG90, Bam, Fixed16_16, TicCmd, bt};

use crate::mobj::{MobjHandle, StateNum, flags};
use crate::state::GameState;

// ---------------------------------------------------------------------------
// Movement constants
// ---------------------------------------------------------------------------

/// Speed multiplier applied to `TicCmd::forward_move` and `side_move`.
///
/// Doom original: `cmd->forwardmove * 2048` (in fixed-point).
pub const PLAYER_SPEED_SCALE: i32 = 2048;

/// Ground friction coefficient (approx 0.90625 in fixed-point: 59392/65536).
///
/// Doom original: `0xE800` stored as a fixed-point fraction.
pub const FRICTION: Fixed16_16 = Fixed16_16(0x0000_E800);

/// Maximum horizontal velocity per axis (30 map units).
///
/// Doom original: `p_local.h` `MAXMOVE (30*FRACUNIT)`.
pub const MAXMOVE: Fixed16_16 = Fixed16_16(30 << 16);

/// Downward acceleration applied per tic to airborne, gravity-affected actors.
///
/// Doom original: `p_local.h` `GRAVITY (FRACUNIT)`.
pub const GRAVITY: Fixed16_16 = Fixed16_16(1 << 16);

// ---------------------------------------------------------------------------
// p_set_mobj_state — canonical state transition (port of P_SetMobjState)
// ---------------------------------------------------------------------------

/// Transition an actor to a new state.
///
/// Port of Doom's `P_SetMobjState`:
/// - Sets `mobj.state = new_state`
/// - Sets `mobj.tics` from `STATES[new_state].tics`
/// - Fires the action function on state *entry* (if `action != Action::NoAction`)
/// - Returns `false` if `new_state` is `S_NULL` (mobj should be removed)
///
/// The caller is responsible for removing the mobj from the slab when this
/// returns `false`.
pub fn p_set_mobj_state(
    gs: &mut GameState,
    handle: MobjHandle,
    new_state: StateNum,
    level: Option<&Level>,
) -> bool {
    // Port of vanilla `P_SetMobjState`'s `do { ... } while (!mobj->tics)` loop:
    // a state with `tics == 0` chains immediately to its `next_state` within the
    // same call, firing every intermediate action (and consuming their RNG).
    //
    // `guard` mirrors nothing in vanilla 1.9 (which has no cycle limit) but
    // protects against a malformed all-zero-tic state cycle hanging the sim.
    let mut state = new_state;
    let mut guard = 0u32;
    loop {
        // S_NULL means the actor is done — caller should remove it.
        if state == StateNum::NULL {
            return false;
        }

        let entry = match crate::states::STATES.get(state.0 as usize) {
            Some(entry) => *entry,
            None => return false,
        };

        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.state = state;
            mo.tics = entry.tics;
        }

        // Fire action on state entry (needs &mut self — borrow released above).
        if entry.action != crate::actions::Action::NoAction as u8 {
            crate::actions::dispatch_action(gs, handle, entry.action, level);
        }

        // Re-read tics: the action may have changed state/tics (e.g. via a
        // nested transition). Vanilla loops while the current tics are 0.
        let tics = match gs.mobjslab.get(handle) {
            Some(mo) => mo.tics,
            // Mobj vanished during the action — nothing more to chain.
            None => return true,
        };
        if tics != 0 {
            break;
        }

        state = entry.next_state;
        guard += 1;
        if guard > 1024 {
            break;
        }
    }

    true
}

// ---------------------------------------------------------------------------
// tick_mobj — process one actor for one tic
// ---------------------------------------------------------------------------

/// Result of processing one actor for one tic.
enum TickMobjResult {
    /// Actor is still alive and active.
    Alive,
    /// Actor should be removed from the slab (transitioned to S_NULL).
    Remove,
}

/// Process one actor for one tic.
///
/// - Decrements `tics` by 1.
/// - If `tics` reaches 0: transitions to `STATES[state].next_state` via
///   `p_set_mobj_state`. If the next state is `S_NULL`, returns `Remove`.
/// - Handles `MF_MISSILE` flag: projectile momentum-based position update.
/// - Skips dead `MF_COUNTKILL` actors (corpses don't need AI processing,
///   but still need state machine advancement for death animations).
fn tick_mobj(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) -> TickMobjResult {
    // --- Phase 1: Missile momentum + collision (vanilla P_XYMovement / P_ZMovement) ---
    // In `P_MobjThinker`, a missile advances by its momentum EVERY tic, BEFORE
    // its state machine, through the same swept `P_XYMovement` path monsters
    // use: it steps via `P_TryMove` (whose `PIT_CheckThing` missile branch
    // damages a shootable actor it overlaps, drawing `(P_Random()%8+1)*damage`)
    // and calls `P_ExplodeMissile` on a blocked step, then `P_ZMovement`
    // explodes it on the floor/ceiling. This replaces the old naive
    // `mo.x += momx` integration plus the separate, non-swept
    // `p_move_projectiles` stepper — which moved missiles twice per tic, ignored
    // z, and used an unswept AABB test, so a fireball reached and struck its
    // target several tics early (DEMO3/E1M7 hit the player at lt192 instead of
    // vanilla's lt206).
    {
        let is_missile = gs
            .mobjslab
            .get(handle)
            .map(|m| m.flags & flags::MF_MISSILE != 0)
            .unwrap_or(false);
        if is_missile {
            // momentum movement (vanilla gate: momx || momy; skulls only for AI)
            let has_momentum = gs
                .mobjslab
                .get(handle)
                .map(|m| m.momx != Fixed16_16::ZERO || m.momy != Fixed16_16::ZERO)
                .unwrap_or(false);
            if has_momentum {
                p_xy_movement_missile(gs, handle, level);
            }
            // z movement — only while still a missile (P_XYMovement may have just
            // exploded it, clearing MF_MISSILE).
            let still_missile = gs
                .mobjslab
                .get(handle)
                .map(|m| m.flags & flags::MF_MISSILE != 0)
                .unwrap_or(false);
            if still_missile {
                p_z_movement_missile(gs, handle, level);
            }
        }
    }

    // --- Phase 1b: Monster / thing momentum integration (vanilla P_XYMovement) ---
    // In `P_MobjThinker` (p_mobj.c), ANY mobj carrying momentum runs
    // `P_XYMovement` BEFORE its state machine (the AI) advances. Missiles are
    // handled by Phase 1 above and the player by
    // `p_move_player`; this covers monsters and anything holding residual
    // thrust momentum (from `P_DamageMobj`) or a Lost Soul skull-fly charge.
    // Vanilla `P_Move` does NOT create walk momentum, so an idle monster has
    // zero momentum here and this is a cheap no-op; a shot monster's thrust
    // momentum is integrated into position and decayed by friction each tic,
    // matching vanilla's per-tic drift exactly.
    {
        let Some(mo) = gs.mobjslab.get(handle) else {
            return TickMobjResult::Remove;
        };
        if mo.flags & flags::MF_MISSILE == 0 {
            p_xy_movement_mobj(gs, handle, level);
        }
    }

    // --- Phase 2: State machine countdown ---
    let cur_state = {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return TickMobjResult::Remove;
        };

        // Infinite-duration states (tics < 0) hold until externally changed.
        if mo.tics < 0 {
            return TickMobjResult::Alive;
        }

        mo.tics -= 1;
        if mo.tics > 0 {
            return TickMobjResult::Alive;
        }

        // tics reached 0 -- time to transition.
        mo.state // Copy: StateNum is Copy
    };

    // --- Phase 3: State transition ---
    // Look up next_state from the STATES table.
    let next_sn = crate::states::STATES
        .get(cur_state.0 as usize)
        .map(|e| e.next_state)
        .unwrap_or(StateNum::NULL);

    // Transition to next state via p_set_mobj_state.
    if !p_set_mobj_state(gs, handle, next_sn, level) {
        // S_NULL -- actor is done, remove it.
        return TickMobjResult::Remove;
    }

    TickMobjResult::Alive
}

// ---------------------------------------------------------------------------
// tick_all_mobjs — iterate all active mobjs
// ---------------------------------------------------------------------------

/// Iterate all active mobjs and advance their state machines by one tic.
///
/// Collects all valid handles first to avoid borrow conflicts during
/// iteration. Skips and gracefully handles removed/freed mobjs.
/// Actors that transition to S_NULL are removed from the slab.
///
/// The player mobj is skipped (its state machine is managed separately
/// by `tick_player`).
/// Snapshot every live actor handle ordered by ascending `generation`.
///
/// Vanilla `P_RunThinkers` walks the thinker list in INSERTION (creation)
/// order — `P_AddThinker` appends each new thinker to the tail. Our slab's
/// monotonic `generation` counter reproduces that order exactly: an actor's
/// generation is its global creation index. Iterating by raw slot index
/// instead would tick actors in free-list-reuse order, so a missile that
/// occupies a recycled low slot would think BEFORE the monsters it was created
/// after — shifting which `P_Random` byte its damage roll consumes (e.g. an imp
/// fireball rolling a different `(P_Random()%8+1)*3`) and desyncing from vanilla
/// even though the total per-tic draw count matches.
fn actors_by_generation(slab: &crate::mobj::MobjSlab) -> Vec<MobjHandle> {
    let mut handles: Vec<MobjHandle> = (0..slab.slot_count())
        .filter_map(|i| slab.handle_at(i))
        .collect();
    // Generations are globally unique, so the ordering is total and stable.
    handles.sort_unstable_by_key(|h| h.generation);
    handles
}

pub fn tick_all_mobjs(gs: &mut GameState, level: Option<&Level>) {
    // Process every actor that existed at the start of the tic, then the
    // this-tic-spawned missiles, all in creation (generation) order.
    let initial_generation = gs.mobjslab.next_generation();
    tick_existing_mobjs_in_gen_range(gs, level, 0, initial_generation, initial_generation);
    tick_same_tic_missiles(gs, level, initial_generation);
}

/// Tick every actor that existed at tic start (`generation < initial_generation`)
/// whose `generation` falls in `[gen_lo, gen_hi)`, in ascending generation order.
///
/// Splitting the tic-start actors by generation lets `tick_world` interleave the
/// sector-light pass at the vanilla thinker-list position (after setup monsters,
/// before gameplay-spawned actors). `[0, initial_generation)` ticks them all.
fn tick_existing_mobjs_in_gen_range(
    gs: &mut GameState,
    level: Option<&Level>,
    gen_lo: u32,
    gen_hi: u32,
    initial_generation: u32,
) {
    let player_handle = gs.player.handle;
    let is_nightmare = gs.skill == crate::spawn::Skill::Nightmare;

    // Snapshot the tic-start actors and tick them in creation (generation)
    // order to match vanilla thinker order (see `actors_by_generation`).
    let tick_order: Vec<MobjHandle> = actors_by_generation(&gs.mobjslab)
        .into_iter()
        // Only actors that existed at tic start; skip the player (tick_player).
        .filter(|h| {
            h.generation < initial_generation
                && h.generation >= gen_lo
                && h.generation < gen_hi
                && *h != player_handle
        })
        .collect();

    for handle in tick_order {
        // Skip freed mobjs (may have been removed by an earlier iteration).
        if gs.mobjslab.get(handle).is_none() {
            continue;
        }

        // --- Nightmare respawn check ---
        // On Nightmare, dead monsters (MF_COUNTKILL corpses) use movecount
        // as a respawn timer.  After 420 tics (12 seconds) the corpse is
        // replaced by a fresh monster at the original spawn point.
        if is_nightmare {
            let is_dead_monster = gs
                .mobjslab
                .get(handle)
                .map(|mo| {
                    mo.health <= 0 && mo.flags & flags::MF_COUNTKILL != 0 && mo.spawn_type != 0
                })
                .unwrap_or(false);

            if is_dead_monster {
                // p_nightmare_respawn handles timer increment and respawn.
                // If it returns true, the corpse has been freed — skip to next.
                if crate::spawn::p_nightmare_respawn(gs, level, handle) {
                    continue;
                }
            }
        }

        let result = tick_mobj(gs, handle, level);
        if matches!(result, TickMobjResult::Remove) {
            gs.mobjslab.free(handle);
        }
    }
}

fn tick_same_tic_missiles(gs: &mut GameState, level: Option<&Level>, initial_generation: u32) {
    let player_handle = gs.player.handle;
    // --- Same-tic missile spawn step (vanilla P_RunThinkers ordering) ---
    // Vanilla appends a newly spawned thinker to the END of the list, so a
    // missile a monster launches from its `A_*Attack` (fired DURING the loop
    // above) is reached and thinker'd on the SAME tic — taking its first
    // `P_XYMovement` step immediately. The loop above only visited mobjs that
    // existed at tic start (`generation < initial_generation`), so those
    // monster-fired missiles would otherwise sit still for a tic and reach
    // their target one step late (a fireball would strike the player a tic off
    // vanilla). Give every missile spawned this tic its first move now.
    // Player-fired missiles are spawned in `tick_player` (before this function)
    // so they already have `generation < initial_generation` and moved above;
    // missiles never spawn further missiles, so one follow-up pass is exact.
    // Tick this-tic-spawned missiles in generation (creation) order too, so a
    // monster that fires several missiles in one tic advances them in the same
    // order vanilla's tail-appended thinker list would.
    let missile_order: Vec<MobjHandle> = actors_by_generation(&gs.mobjslab)
        .into_iter()
        .filter(|h| h.generation >= initial_generation && *h != player_handle)
        .collect();
    for handle in missile_order {
        let is_missile = gs
            .mobjslab
            .get(handle)
            .map(|m| m.flags & flags::MF_MISSILE != 0)
            .unwrap_or(false);
        if !is_missile {
            continue;
        }
        let result = tick_mobj(gs, handle, level);
        if matches!(result, TickMobjResult::Remove) {
            gs.mobjslab.free(handle);
        }
    }
}

// ---------------------------------------------------------------------------
// tick_world — the master per-tic function
// ---------------------------------------------------------------------------

/// The master per-tic world simulation function.
///
/// Processes all game objects in the correct order, matching Doom's
/// `P_Ticker` / `P_RunThinkers`:
///
/// 1. `tick_all_mobjs` — process all actor state machines
/// 2. `tick_doors` — door movement
/// 3. `tick_ceilings` — crusher/ceiling movement
/// 4. `tick_floors` — floor movement
/// 5. `tick_lifts` — lift movement
/// 6. `tick_platforms` — perpetual platforms
/// 7. `tick_lights` + `tick_sector_lights` — sector light effects
/// 8. `tick_scrollers` — scrolling wall textures
/// 9. `tick_conveyors` — conveyor belt forces
/// 10. (Missiles are advanced per-actor inside `tick_all_mobjs`, step 1.)
/// 11. Increment `gs.stats.level_time`
///
/// This does NOT process player input — call `tick_player` before this.
pub fn tick_world(gs: &mut GameState, mut level: Option<&mut Level>) {
    let initial_generation = gs.mobjslab.next_generation();

    // Vanilla's single thinker list runs in creation order:
    //   [setup map things]  [setup sector specials]  [gameplay spawns]
    // Sector-light thinkers (which draw P_Random) were created during
    // P_SpawnSpecials — after every map monster but before any actor spawned
    // during play. When the setup boundary has been frozen we reproduce that
    // ordering: tick setup monsters, then the sector-light pass, then
    // gameplay-spawned actors. Otherwise fall back to the legacy "all mobjs,
    // then lights" order (unit tests and callers that never freeze it).
    let boundary = gs.thinker_setup_boundary;
    if boundary != u32::MAX {
        // 1a. Setup-era actors (created by P_SetupLevel).
        tick_existing_mobjs_in_gen_range(gs, level.as_deref(), 0, boundary, initial_generation);

        // 1b. Setup-time sector-light thinkers tick here, at their vanilla
        //     thinker-list position (before any gameplay-spawned actor).
        if let Some(lv) = level.as_deref_mut() {
            crate::specials::tick_lights(gs, lv);
            crate::specials::tick_sector_lights(gs, lv);
        }

        // 1c. Gameplay-era actors (missiles, etc.) and this-tic-spawned
        //     missiles, all after the setup-time lights.
        tick_existing_mobjs_in_gen_range(
            gs,
            level.as_deref(),
            boundary,
            initial_generation,
            initial_generation,
        );
        tick_same_tic_missiles(gs, level.as_deref(), initial_generation);

        // 1d. Walkover specials monsters crossed during the actor pass (vanilla
        //     `P_CrossSpecialLine`), dispatched before the sector movers so an
        //     activated lift steps on the same tic.
        if let Some(lv) = level.as_deref_mut() {
            crate::linedef_dispatch::dispatch_pending_monster_crossings(gs, lv);
        }

        // 2-6. Sector movers (doors, ceilings, floors, lifts, platforms).
        //      RNG-neutral; kept after the actor passes as before.
        if let Some(lv) = level.as_deref_mut() {
            crate::specials::tick_doors(gs, lv);
            crate::specials::tick_ceilings(gs, lv);
            crate::specials::tick_floors(gs, lv);
            crate::specials::tick_lifts(gs, lv);
            crate::specials::tick_platforms(gs, lv);
        }
    } else {
        // 1. Advance all actor state machines.
        tick_all_mobjs(gs, level.as_deref());

        // 1d. Walkover specials monsters crossed during the actor pass.
        if let Some(lv) = level.as_deref_mut() {
            crate::linedef_dispatch::dispatch_pending_monster_crossings(gs, lv);
        }

        // 2-6. Sector movers (doors, ceilings, floors, lifts, platforms).
        if let Some(lv) = level.as_deref_mut() {
            crate::specials::tick_doors(gs, lv);
            crate::specials::tick_ceilings(gs, lv);
            crate::specials::tick_floors(gs, lv);
            crate::specials::tick_lifts(gs, lv);
            crate::specials::tick_platforms(gs, lv);

            // 7. Light effects.
            crate::specials::tick_lights(gs, lv);
            crate::specials::tick_sector_lights(gs, lv);
        }
    }

    // 8. Scrolling walls (no level mutation needed).
    crate::specials::tick_scrollers(gs);

    // 9. Conveyor belts: push actors standing in conveyor sectors.
    crate::specials::tick_conveyors(gs, level.as_deref());

    // 10. Missiles are advanced inside `tick_all_mobjs` (vanilla `P_MobjThinker`
    //     runs `P_XYMovement`/`P_ZMovement` per missile in thinker order), so
    //     there is no longer a separate projectile-stepping pass here.

    // 11. Increment level time.
    gs.stats.level_time = gs.stats.level_time.wrapping_add(1);
}

// ---------------------------------------------------------------------------
// tick_player — process player input for one tic
// ---------------------------------------------------------------------------

/// Process player input for one tic.
///
/// Applies the `TicCmd` to the player's mobj:
/// - Applies `angle_turn` to player angle
/// - Applies `forward_move` and `side_move` as thrust
/// - Processes position update with collision
/// - Applies friction and velocity clamping
/// - Processes buttons: `BT_ATTACK` (fire weapon), `BT_USE` (activate
///   linedef), `BT_CHANGE` (weapon switch)
/// - Checks for item pickups
/// - Checks for secret sector discovery
pub fn tick_player(gs: &mut GameState, cmd: TicCmd, mut level: Option<&mut Level>) {
    if gs.player.is_dead() {
        gs.player.attack_down = false;
        gs.player.extra_light = 0;
        gs.player.use_down = false;
        return;
    }

    // Sector specials — vanilla `P_PlayerThink` runs `P_PlayerInSpecialSector`
    // BEFORE the player mobj's position/z is integrated for this tic (position
    // integration happens later in `P_MobjThinker`, after `P_PlayerThink`).
    // In doom-rs `p_move_player` integrates the player's z inline, so the check
    // must run FIRST — sampling the position/z left over from the previous tic,
    // exactly as vanilla does. Sampling after `p_move_player` would see the z
    // one tic early (e.g. reaching a nukage floor mid-descent) and apply
    // spurious floor damage vanilla does not. `leveltime` is unchanged by this
    // ordering (it is incremented later, in `tick_world`).
    if let Some(lv) = level.as_deref_mut() {
        crate::specials::p_player_in_special_sector(gs, lv);
    }

    // Capture the player's position BEFORE this tic's movement is integrated.
    //
    // Vanilla `P_MovePlayer` only sets the player's momentum (`P_Thrust`); the
    // position itself is integrated later in `P_XYMovement`, which runs in
    // `P_RunThinkers` — AFTER `P_MovePsprites`. doom-rs instead integrates the
    // player's position inline here in `p_move_player`. So when the weapon
    // psprite action functions run (`A_FirePistol` etc., in `tick_psprites`
    // below), vanilla samples the player origin from the END of the PREVIOUS
    // tic (this tic's angle is already applied, but the position is not yet
    // integrated), whereas a naive doom-rs would sample this tic's post-move
    // position — one movement step ahead. That one-tic-late origin skews every
    // hitscan's `P_LineAttack` / knockback `R_PointToAngle2(player, target)`.
    //
    // Preserve vanilla timing by running `tick_psprites` against the pre-move
    // position (with this tic's freshly-applied angle) and restoring the real
    // post-move position afterward. See the `tick_psprites` call below.
    let pre_move_pos = gs
        .mobjslab
        .get(gs.player.handle)
        .map(|mo| (mo.x, mo.y, mo.z, mo.subsector));

    // Movement + attack (immutable level borrow).
    p_move_player(gs, cmd, level.as_deref_mut());

    // Pickup check: scan MF_SPECIAL actors.
    //
    // Vanilla collects items in `P_TouchSpecialThing`, called from
    // `PIT_CheckThing` during `P_XYMovement` — i.e. AFTER this tic's XY move
    // reaches its destination, but BEFORE `P_ZMovement` integrates the player's
    // z. `P_TouchSpecialThing`'s opening Z-reach gate therefore samples the
    // player's START-of-tic z (`toucher->z`), not the post-`P_ZMovement` z.
    // doom-rs integrates z inline in `p_move_player`, so restore the pre-move z
    // (keeping the post-move x,y that the AABB overlap uses) for the pickup scan,
    // then put the real post-move z back. Without this, a player descending
    // stairs onto a low item reaches Z-reach one tic early (DEMO3/E1M7 health
    // bonus at z=24 collected at leveltime 530, matching vanilla, not 529).
    if !gs.player.is_dead() {
        let restore_z = pre_move_pos.and_then(|(_, _, pre_z, _)| {
            gs.mobjslab.get_mut(gs.player.handle).map(|mo| {
                let post_z = mo.z;
                mo.z = pre_z;
                post_z
            })
        });
        crate::pickups::p_check_pickups(gs);
        if let Some(post_z) = restore_z
            && let Some(mo) = gs.mobjslab.get_mut(gs.player.handle)
        {
            mo.z = post_z;
        }
    }

    // BT_USE: activate linedef ahead of player on the leading edge only.
    let use_held = cmd.buttons & bt::BT_USE != 0;
    if use_held && !gs.player.use_down {
        if let Some(lv) = level.as_deref_mut() {
            let handle = gs.player.handle;
            crate::specials::p_use_lines(gs, lv, handle);
        }
    }
    gs.player.use_down = use_held;

    #[cfg(feature = "telemetry")]
    {
        if let Some(mo) = gs.mobjslab.get(gs.player.handle) {
            gs.telemetry.record(
                gs.tic_num,
                mo.x.to_int(),
                mo.y.to_int(),
                crate::telemetry::TelemetryKind::Position,
            );
        }
    }

    // Weapon psprites (vanilla `P_MovePsprites`). Vanilla runs this against the
    // player's pre-integration position (see `pre_move_pos` above), so restore
    // that position for the duration of the psprite tick — keeping this tic's
    // already-applied angle and momentum — then put the real post-move position
    // back so `tick_world` and the per-tic snapshot observe the current origin.
    let post_move_pos = gs
        .mobjslab
        .get(gs.player.handle)
        .map(|mo| (mo.x, mo.y, mo.z, mo.subsector));
    if let Some((x, y, z, subsector)) = pre_move_pos
        && let Some(mo) = gs.mobjslab.get_mut(gs.player.handle)
    {
        mo.x = x;
        mo.y = y;
        mo.z = z;
        mo.subsector = subsector;
    }

    crate::weapons::tick_psprites(gs, cmd, level.as_deref());

    if let Some((x, y, z, subsector)) = post_move_pos
        && let Some(mo) = gs.mobjslab.get_mut(gs.player.handle)
    {
        mo.x = x;
        mo.y = y;
        mo.z = z;
        mo.subsector = subsector;
    }
}

// ---------------------------------------------------------------------------
// GameState::tick (backward-compatible entry point)
// ---------------------------------------------------------------------------

impl GameState {
    /// Advance the simulation by one tic.
    ///
    /// 1. Increment `tic_num`.
    /// 2. Apply `cmd` to the player Mobj (`tick_player`), including weapon
    ///    firing (`BT_ATTACK`) and use-key activation (`BT_USE`).
    /// 3. Run the world simulation (`tick_world`): sector specials,
    ///    thinker loop, movers, projectiles.
    ///
    /// `level` is `None` in unit tests (skips blockmap collision, BT_USE,
    /// and sector specials) and `Some(&mut level)` in real gameplay.
    pub fn tick(&mut self, cmd: TicCmd, mut level: Option<&mut Level>) {
        self.tic_num = self.tic_num.wrapping_add(1);

        // Clear transient per-tic state.
        self.exit_request = None;
        self.sound.sound_queue.clear();

        // Player input processing.
        tick_player(self, cmd, level.as_deref_mut());

        // World simulation: mobjs, movers, projectiles, level_time.
        tick_world(self, level);
        #[cfg(feature = "style_meter")]
        self.style.tick(self.tic_num);
    }

    // -----------------------------------------------------------------------
    // Legacy thinker loop — kept for backward compatibility
    // -----------------------------------------------------------------------

    /// Run the legacy thinker loop (advance all actor state machines).
    ///
    /// Prefer `tick_all_mobjs` for new code. This method is retained for
    /// backward compatibility with existing call sites.
    #[doc(hidden)]
    pub fn run_thinkers(&mut self, level: Option<&Level>) {
        // Collect iteration boundaries to avoid borrow conflicts and guarantee determinism.
        let initial_slot_count = self.mobjslab.slot_count();
        let initial_generation = self.mobjslab.next_generation();

        for i in 0..initial_slot_count {
            if let Some(handle) = self.mobjslab.handle_at(i) {
                // Skip mobjs spawned during this iteration.
                if handle.generation >= initial_generation {
                    continue;
                }
                self.advance_mobj_state(handle, level);
            }
        }
    }

    /// Advance `handle`'s state machine by one tic (legacy implementation).
    ///
    /// When `tics` reaches zero the actor transitions to `next_state` and the
    /// new state's action function fires immediately (matching Doom's
    /// `P_SetMobjState` behaviour).
    ///
    /// Prefer `tick_mobj` + `p_set_mobj_state` for new code.
    fn advance_mobj_state(&mut self, handle: MobjHandle, level: Option<&Level>) {
        // Step 1: countdown.  Copy out the current StateNum on transition,
        // then release the borrow so dispatch_action can take &mut self.
        let cur_state: StateNum = {
            let Some(mo) = self.mobjslab.get_mut(handle) else {
                return;
            };
            // Infinite-duration states (tics < 0) hold until externally changed.
            if mo.tics < 0 {
                return;
            }
            mo.tics -= 1;
            if mo.tics > 0 {
                return;
            }
            mo.state // Copy: StateNum is Copy
        };

        // Step 2: look up next_state.
        let next_sn = crate::states::STATES
            .get(cur_state.0 as usize)
            .map(|e| e.next_state)
            .unwrap_or(StateNum::NULL);

        if next_sn == StateNum::NULL {
            // Terminal state: hold forever.
            if let Some(mo) = self.mobjslab.get_mut(handle) {
                mo.tics = -1;
            }
            return;
        }

        // Step 3: transition via p_set_mobj_state.
        p_set_mobj_state(self, handle, next_sn, level);
    }
}

// ---------------------------------------------------------------------------
// P_MovePlayer — port of Doom's p_user.c: P_MovePlayer
// ---------------------------------------------------------------------------

/// Apply player movement from `cmd` to the player's mobj.
///
/// Port of Doom's `P_MovePlayer`:
/// 1. Apply angle_turn to mobj angle
/// 2. Apply forward_move thrust
/// 3. Apply side_move thrust (perpendicular)
/// 4. Collision check via P_TryMove
/// 5. Apply friction and velocity clamping
fn p_move_player(gs: &mut GameState, cmd: TicCmd, level: Option<&mut Level>) {
    let handle = gs.player.handle;

    // Vanilla `onground` (P_MovePlayer): the player may only accelerate while
    // resting on the floor. Computed from the floor under the *current*
    // position (the previous tic's `floorz`), before any movement this tic.
    // With no level (unit tests) we treat the player as grounded, preserving
    // the historical always-thrust behaviour those tests assert.
    let onground = match level.as_deref() {
        Some(lv) => gs
            .mobjslab
            .get(handle)
            .and_then(|mo| {
                let (z, x, y) = (mo.z, mo.x, mo.y);
                crate::movement::support_state_at(&gs.mobjslab, handle, x, y, lv)
                    .map(|(floorz, _)| z <= floorz)
            })
            .unwrap_or(true),
        None => true,
    };

    // Thrust block: apply turn + acceleration, then release borrow.
    {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };

        // 1. Turn: cmd.angle_turn is 16-bit BAM; shift to 32-bit BAM space.
        mo.angle = mo.angle + Bam((cmd.angle_turn as i32 as u32).wrapping_shl(16));

        // 2. Forward / backward thrust — only while on the ground.
        if cmd.forward_move != 0 && onground {
            let angle = mo.angle;
            p_thrust(mo, angle, cmd.forward_move);
        }

        // 3. Strafe: thrust perpendicular (90 degrees left of facing direction).
        if cmd.side_move != 0 && onground {
            let strafe_angle = mo.angle - ANG90;
            p_thrust(mo, strafe_angle, cmd.side_move);
        }
    }

    // Vanilla `P_CalcHeight` (called from `P_PlayerThink` immediately after
    // `P_MovePlayer`, before `P_MovePsprites`): recompute `player->bob` from the
    // post-thrust, pre-friction, pre-clamp momentum. `A_WeaponReady` reads this
    // to sway the weapon psprite; the resting `sy` it leaves behind sets the
    // exact lower/raise tic count and therefore the fire cadence. Vanilla uses
    // the momentum here — before `P_XYMovement`'s MAXMOVE clamp and friction —
    // so it must be sampled at this point, matching that ordering.
    {
        let Some(mo) = gs.mobjslab.get(handle) else {
            return;
        };
        let momx = mo.momx.raw();
        let momy = mo.momy.raw();
        // MAXBOB = 0x100000 (16 pixels).
        const MAXBOB: i32 = 0x0010_0000;
        let mut bob =
            crate::geom::fixed_mul(momx, momx).wrapping_add(crate::geom::fixed_mul(momy, momy));
        bob >>= 2;
        if bob > MAXBOB {
            bob = MAXBOB;
        }
        gs.player.bob = bob;
    }

    // 4. P_XYMovement: clamp momentum to MAXMOVE, then step-and-collide.
    if gs.mobjslab.get(handle).is_none() {
        return;
    };

    {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };
        mo.momx = mo.momx.clamp(-MAXMOVE, MAXMOVE);
        mo.momy = mo.momy.clamp(-MAXMOVE, MAXMOVE);
    }

    // Walkover line-crossings accumulated across this tic's P_TryMove steps, in
    // vanilla `while(numspechit--)` dispatch order. Dispatched after the move
    // (below), where the level can be borrowed mutably.
    let mut player_crossings: Vec<usize> = Vec::new();

    // Vanilla P_XYMovement move-stepping: split moves whose magnitude exceeds
    // MAXMOVE/2 into halves (P_TryMove per step), sliding along walls on a
    // blocked step. With no level (unit tests) apply the momentum directly.
    match level.as_deref() {
        Some(lv) => {
            let (mut xmove, mut ymove) = {
                let mo = gs.mobjslab.get(handle).expect("player exists");
                (mo.momx.raw(), mo.momy.raw())
            };
            let half = (MAXMOVE.raw()) / 2;
            loop {
                let (ptryx, ptryy);
                // Vanilla checks only the positive overflow bound here.
                if xmove > half || ymove > half {
                    let mo = gs.mobjslab.get(handle).expect("player exists");
                    ptryx = mo.x + Fixed16_16::from_raw(xmove / 2);
                    ptryy = mo.y + Fixed16_16::from_raw(ymove / 2);
                    xmove >>= 1;
                    ymove >>= 1;
                } else {
                    let mo = gs.mobjslab.get(handle).expect("player exists");
                    ptryx = mo.x + Fixed16_16::from_raw(xmove);
                    ptryy = mo.y + Fixed16_16::from_raw(ymove);
                    xmove = 0;
                    ymove = 0;
                }

                // Vanilla `P_TryMove` fires `P_CrossSpecialLine` for every special
                // line the box straddled whose *infinite-line* side the centre
                // crossed on this step (see `record_player_crossings`); the
                // split-move halves and each `P_SlideMove` sub-step accumulate
                // their crossings in vanilla order for dispatch below.
                if !crate::movement::p_try_move_commit_tracked(
                    &mut gs.mobjslab,
                    handle,
                    ptryx,
                    ptryy,
                    lv,
                    &mut player_crossings,
                ) {
                    // Blocked: player slides along the wall.
                    crate::movement::p_slide_move_vanilla(
                        &mut gs.mobjslab,
                        handle,
                        lv,
                        &mut player_crossings,
                    );
                }

                if xmove == 0 && ymove == 0 {
                    break;
                }
            }
        }
        None => {
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.x += mo.momx;
                mo.y += mo.momy;
            }
        }
    }

    let (final_x, final_y) = {
        let mo = gs.mobjslab.get(handle).expect("player exists");
        (mo.x, mo.y)
    };

    // Floor height under the (new) position, needed both for vanilla's
    // "no friction while airborne" rule and for P_ZMovement below.
    let support = level.as_deref().and_then(|lv| {
        crate::movement::support_state_at(&gs.mobjslab, handle, final_x, final_y, lv)
    });

    // 5. Friction (P_XYMovement) + clamp. Momentum is already the post-slide
    //    velocity (P_SlideMove updated it); do NOT recompute it from the net
    //    displacement, which would discard the tangential slide component.
    {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };

        // Vanilla P_XYMovement applies friction only when the actor is resting
        // on the floor (`mo->z <= mo->floorz`); an airborne actor keeps its full
        // horizontal momentum. `mo.z` here is still this tic's starting z (the
        // Z step runs afterwards), matching the p_mobj.c ordering. When we have
        // no level (unit tests) fall back to the historical always-friction path.
        let on_ground = match support {
            Some((floorz, _)) => mo.z <= floorz,
            None => true,
        };
        if on_ground {
            // STOPSPEED zeroing: with no player input and sub-STOPSPEED speed,
            // vanilla snaps momentum to zero instead of applying friction.
            let stopspeed = Fixed16_16(0x1000);
            let below_stop = mo.momx > -stopspeed
                && mo.momx < stopspeed
                && mo.momy > -stopspeed
                && mo.momy < stopspeed;
            if below_stop && cmd.forward_move == 0 && cmd.side_move == 0 {
                mo.momx = Fixed16_16::ZERO;
                mo.momy = Fixed16_16::ZERO;
            } else {
                mo.momx = mo.momx.fixed_mul(FRICTION);
                mo.momy = mo.momy.fixed_mul(FRICTION);
            }
        }
        mo.momx = mo.momx.clamp(-MAXMOVE, MAXMOVE);
        mo.momy = mo.momy.clamp(-MAXMOVE, MAXMOVE);
    }

    if let Some((support_floor, subsector)) = support
        && let Some(mo) = gs.mobjslab.get_mut(handle)
    {
        if let Some(subsector) = subsector {
            mo.subsector = subsector as u32;
        }
        // Vanilla P_ZMovement (floor half): advance z by momz, then either land
        // on the floor (clamp + kill downward momentum) or, when airborne,
        // accelerate downward under gravity. The first tic of a fall uses
        // `-2*GRAVITY` (`momz == 0` case) exactly as in p_mobj.c, so stepping
        // off a ledge descends gradually instead of snapping to the low floor.
        mo.z += mo.momz;
        if mo.z <= support_floor {
            if mo.momz < Fixed16_16::ZERO {
                mo.momz = Fixed16_16::ZERO;
            }
            mo.z = support_floor;
        } else if mo.momz == Fixed16_16::ZERO {
            mo.momz = -(GRAVITY + GRAVITY);
        } else {
            mo.momz -= GRAVITY;
        }
    }

    // Fire the walkover crossings accumulated during the move (vanilla
    // `P_CrossSpecialLine`, in `while(numspechit--)` order). Detection is the
    // exact vanilla box-straddle + `P_PointOnLineSide` centre side-change test
    // (see `record_player_crossings`), not the old truncated-integer segment
    // intersection.
    if !player_crossings.is_empty()
        && let Some(lv) = level
    {
        crate::linedef_dispatch::dispatch_player_crossings(gs, lv, handle, &player_crossings);
    }
}

// ---------------------------------------------------------------------------
// P_Thrust helper
// ---------------------------------------------------------------------------

/// Apply a thrust of `move_units` map-units/tic in direction `angle`.
///
/// Port of Doom's `P_Thrust`:
/// ```c
/// mo->momx += FixedMul(move, finecosine[angle >> ANGLETOFINESHIFT]);
/// mo->momy += FixedMul(move, finesine  [angle >> ANGLETOFINESHIFT]);
/// ```
///
/// Requires `Bam::init_trig_tables()` to have been called.
/// Returns zero thrust if tables have not been initialized
/// (safe startup behavior -- trig returns 0 before init).
fn p_thrust(mo: &mut crate::mobj::Mobj, angle: Bam, move_units: i8) {
    let thrust = Fixed16_16::from_raw(move_units as i32 * PLAYER_SPEED_SCALE);
    mo.momx += angle.cos().fixed_mul(thrust);
    mo.momy += angle.sin().fixed_mul(thrust);
}

// ---------------------------------------------------------------------------
// P_XYMovement — non-player mobj momentum integration
// ---------------------------------------------------------------------------

/// Vanilla `P_XYMovement` MF_CORPSE friction clause (`p_mobj.c`).
///
/// A corpse that is still sliding with some momentum (`|momx| > FRACUNIT/4` or
/// `|momy| > FRACUNIT/4`) and is "halfway off a step" — its bbox-support floor
/// (`mo->floorz`) differs from its center-point sector floor
/// (`mo->subsector->sector->floorheight`) — must NOT have friction applied that
/// tic ("do not stop sliding"). Returns `true` when friction should be skipped.
///
/// Getting this wrong makes a shot corpse decelerate one tic too early, leaving
/// it a couple of map units short of its vanilla resting position — enough to
/// flip a razor-thin `PIT_CheckThing` overlap and stall a passing player (the
/// DEMO3/E1M7 leveltime-409 one-tic `py` stall this pins).
fn corpse_skips_friction(
    mo_flags: u32,
    momx: Fixed16_16,
    momy: Fixed16_16,
    support_floor: Fixed16_16,
    center_floor: Fixed16_16,
) -> bool {
    if mo_flags & flags::MF_CORPSE == 0 {
        return false;
    }
    let quarter = Fixed16_16(0x4000); // FRACUNIT/4
    let has_momentum = momx > quarter || momx < -quarter || momy > quarter || momy < -quarter;
    has_momentum && support_floor != center_floor
}

/// Port of vanilla `P_XYMovement` (`p_mobj.c`) for NON-PLAYER, NON-MISSILE
/// mobjs — monsters and any thing carrying momentum.
///
/// The player runs its own inline copy in `p_move_player` (which additionally
/// wall-slides and gates the STOPSPEED zeroing on player input); missiles run
/// `p_xy_movement_missile`. This handles the remaining actors: it
/// integrates `momx/momy` into position (with `P_TryMove` collision, zeroing
/// momentum on a blocked step exactly as vanilla does for non-player,
/// non-missile things) and then applies STOPSPEED zeroing / `FRICTION` decay.
///
/// Crucially, `P_Move` never creates walk momentum, so a monster only reaches
/// the friction path here when it holds residual thrust momentum (set by
/// `P_DamageMobj`) or a Lost Soul skull-fly charge — reproducing vanilla's
/// per-tic drift-and-decay of a shot monster.
fn p_xy_movement_mobj(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    let (momx0, momy0, flags0) = {
        let Some(mo) = gs.mobjslab.get(handle) else {
            return;
        };
        (mo.momx, mo.momy, mo.flags)
    };

    // Vanilla: with no momentum, only MF_SKULLFLY does anything — the skull
    // slammed into something, so drop the flag and return to its spawn state.
    if momx0 == Fixed16_16::ZERO && momy0 == Fixed16_16::ZERO {
        if flags0 & flags::MF_SKULLFLY != 0 {
            let spawn_state = {
                let mo = gs.mobjslab.get(handle).expect("mobj exists");
                crate::mobjinfo::MOBJINFO[mo.kind as usize].spawn_state
            };
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.flags &= !flags::MF_SKULLFLY;
                mo.momx = Fixed16_16::ZERO;
                mo.momy = Fixed16_16::ZERO;
                mo.momz = Fixed16_16::ZERO;
            }
            p_set_mobj_state(gs, handle, spawn_state, level);
        }
        return;
    }

    // Clamp momentum to MAXMOVE.
    {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };
        mo.momx = mo.momx.clamp(-MAXMOVE, MAXMOVE);
        mo.momy = mo.momy.clamp(-MAXMOVE, MAXMOVE);
    }

    // Step-and-move loop (splits steps larger than MAXMOVE/2, as vanilla does).
    match level {
        Some(lv) => {
            let (mut xmove, mut ymove) = {
                let mo = gs.mobjslab.get(handle).expect("mobj exists");
                (mo.momx.raw(), mo.momy.raw())
            };
            let half = MAXMOVE.raw() / 2;
            loop {
                let (ptryx, ptryy);
                if xmove > half || ymove > half {
                    let mo = gs.mobjslab.get(handle).expect("mobj exists");
                    ptryx = mo.x + Fixed16_16::from_raw(xmove / 2);
                    ptryy = mo.y + Fixed16_16::from_raw(ymove / 2);
                    xmove >>= 1;
                    ymove >>= 1;
                } else {
                    let mo = gs.mobjslab.get(handle).expect("mobj exists");
                    ptryx = mo.x + Fixed16_16::from_raw(xmove);
                    ptryy = mo.y + Fixed16_16::from_raw(ymove);
                    xmove = 0;
                    ymove = 0;
                }

                if !crate::movement::p_try_move_commit(&mut gs.mobjslab, handle, ptryx, ptryy, lv) {
                    // Vanilla: a non-player, non-missile blocked move stops dead
                    // (`mo->momx = mo->momy = 0`). Missiles never reach here.
                    if let Some(mo) = gs.mobjslab.get_mut(handle) {
                        mo.momx = Fixed16_16::ZERO;
                        mo.momy = Fixed16_16::ZERO;
                    }
                }

                if xmove == 0 && ymove == 0 {
                    break;
                }
            }
        }
        None => {
            // No level (unit tests): integrate momentum directly.
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.x += mo.momx;
                mo.y += mo.momy;
            }
        }
    }

    // Friction / STOPSPEED. Missiles and skull-fly charges never get friction.
    let (flags1, mz) = {
        let Some(mo) = gs.mobjslab.get(handle) else {
            return;
        };
        (mo.flags, mo.z)
    };
    if flags1 & (flags::MF_MISSILE | flags::MF_SKULLFLY) != 0 {
        return;
    }

    // No friction while airborne (`mo->z > mo->floorz`). With no level we fall
    // back to the always-friction path (matching the unit-test convention).
    if let Some(lv) = level {
        let (x, y, momx, momy) = {
            let mo = gs.mobjslab.get(handle).expect("mobj exists");
            (mo.x, mo.y, mo.momx, mo.momy)
        };
        if let Some((floorz, _)) = crate::movement::support_state_at(&gs.mobjslab, handle, x, y, lv)
        {
            if mz > floorz {
                return;
            }

            // Vanilla `P_XYMovement` MF_CORPSE clause: a corpse that is still
            // sliding with some momentum (|mom| > FRACUNIT/4) and is halfway
            // off a step — `mo->floorz != mo->subsector->sector->floorheight`,
            // i.e. the bbox-support floor differs from the center-point sector
            // floor — skips friction entirely this tic ("do not stop sliding").
            let center_floor = lv
                .floor_at(x.to_int(), y.to_int())
                .map(|f| Fixed16_16::from_int(f as i32))
                .unwrap_or(floorz);
            if corpse_skips_friction(flags1, momx, momy, floorz, center_floor) {
                return;
            }
        }
    }

    // STOPSPEED zeroing / FRICTION decay. For a non-player thing the vanilla
    // player-input clause is vacuously satisfied, so sub-STOPSPEED momentum
    // snaps to zero and everything else is multiplied by FRICTION (0xE800).
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        let stopspeed = Fixed16_16(0x1000);
        let below_stop = mo.momx > -stopspeed
            && mo.momx < stopspeed
            && mo.momy > -stopspeed
            && mo.momy < stopspeed;
        if below_stop {
            mo.momx = Fixed16_16::ZERO;
            mo.momy = Fixed16_16::ZERO;
        } else {
            mo.momx = mo.momx.fixed_mul(FRICTION);
            mo.momy = mo.momy.fixed_mul(FRICTION);
        }
    }
}

// ---------------------------------------------------------------------------
// Missile movement — vanilla P_XYMovement / P_ZMovement missile branches
// ---------------------------------------------------------------------------

/// Vanilla `P_XYMovement` for an `MF_MISSILE` actor: clamp momentum to
/// `MAXMOVE`, then step-and-collide through `P_TryMove` in the same halving
/// loop the player and monsters use. On a blocked step the missile is exploded
/// (`P_ExplodeMissile`) instead of stopping/sliding. Missiles never get
/// friction, so there is no post-loop decay.
fn p_xy_movement_missile(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    // Clamp momentum to MAXMOVE (vanilla `if (momx > MAXMOVE) momx = MAXMOVE;`).
    {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };
        mo.momx = mo.momx.clamp(-MAXMOVE, MAXMOVE);
        mo.momy = mo.momy.clamp(-MAXMOVE, MAXMOVE);
    }

    match level {
        Some(lv) => {
            let (mut xmove, mut ymove) = {
                let mo = gs.mobjslab.get(handle).expect("missile exists");
                (mo.momx.raw(), mo.momy.raw())
            };
            let half = MAXMOVE.raw() / 2;
            loop {
                let (ptryx, ptryy);
                // Vanilla checks only the positive overflow bound here.
                if xmove > half || ymove > half {
                    let mo = gs.mobjslab.get(handle).expect("missile exists");
                    ptryx = mo.x + Fixed16_16::from_raw(xmove / 2);
                    ptryy = mo.y + Fixed16_16::from_raw(ymove / 2);
                    xmove >>= 1;
                    ymove >>= 1;
                } else {
                    let mo = gs.mobjslab.get(handle).expect("missile exists");
                    ptryx = mo.x + Fixed16_16::from_raw(xmove);
                    ptryy = mo.y + Fixed16_16::from_raw(ymove);
                    xmove = 0;
                    ymove = 0;
                }

                if !p_try_move_missile(gs, handle, ptryx, ptryy, lv) {
                    // Blocked by a thing or a wall: explode the missile.
                    p_explode_missile(gs, handle, level);
                    return;
                }

                if xmove == 0 && ymove == 0 {
                    break;
                }
            }
        }
        None => {
            // No level (unit tests): integrate momentum directly, no collision.
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                let (mx, my) = (mo.momx, mo.momy);
                mo.x += mx;
                mo.y += my;
            }
        }
    }
}

/// Vanilla `P_ZMovement` restricted to the missile case. Projectiles carry
/// `MF_NOGRAVITY`, so this simply advances `z` by `momz` and explodes the
/// missile on contact with the floor or ceiling.
fn p_z_movement_missile(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    // adjust height
    let (x, y, new_z, height) = {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };
        mo.z += mo.momz;
        (mo.x, mo.y, mo.z, mo.height)
    };

    let Some(lv) = level else {
        return; // no level (unit tests): no floor/ceiling to hit
    };

    // Floor: highest contacted floor under the missile's bbox (~tmfloorz).
    let floorz = crate::movement::support_state_at(&gs.mobjslab, handle, x, y, lv)
        .map(|(f, _)| f)
        .unwrap_or(Fixed16_16::ZERO);
    if new_z <= floorz {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.z = floorz;
            mo.momz = Fixed16_16::ZERO;
        }
        p_explode_missile(gs, handle, level);
        return;
    }

    // Ceiling: subsector-sector ceiling at the missile's origin (~tmceilingz).
    let ceilingz = lv
        .sector_index_at(x.to_int(), y.to_int())
        .and_then(|si| lv.sectors.get(si))
        .map(|s| Fixed16_16::from_int(s.ceil_height as i32));
    if let Some(cz) = ceilingz
        && new_z + height > cz
    {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.z = cz - height;
            mo.momz = Fixed16_16::ZERO;
        }
        p_explode_missile(gs, handle, level);
    }
}

/// Vanilla `P_TryMove` for a missile step: run the `PIT_CheckThing` missile
/// pass (things are checked BEFORE lines in `P_CheckPosition`), then the
/// line/geometry pass, committing the new position only when both succeed.
/// Returns `false` when the step is blocked (by a thing or a wall) — the caller
/// then calls `P_ExplodeMissile`.
fn p_try_move_missile(
    gs: &mut GameState,
    handle: MobjHandle,
    x: Fixed16_16,
    y: Fixed16_16,
    level: &Level,
) -> bool {
    // Things first (may deal damage as a side effect and block the step).
    if !missile_check_things(gs, handle, x, y) {
        return false;
    }
    // Then lines. `p_try_move` skips its own thing pass for MF_MISSILE, so this
    // is purely the wall/opening/step geometry check.
    if !crate::movement::p_try_move(&gs.mobjslab, handle, x, y, level) {
        return false;
    }
    // Commit the new position (vanilla P_TryMove sets mo->x/mo->y on success).
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.x = x;
        mo.y = y;
    }
    true
}

/// Vanilla `PIT_CheckThing` restricted to the `MF_MISSILE` branch, evaluated at
/// the proposed step position `(x, y)`. Returns `false` if the missile is
/// blocked by a thing — after dealing `(P_Random()%8+1)*damage` to a shootable
/// victim, or harmlessly against a same-species monster / a solid non-shootable
/// obstacle. Iterates in slot order as an O(n²) stand-in for the blockmap
/// `P_BlockThingsIterator`.
fn missile_check_things(
    gs: &mut GameState,
    handle: MobjHandle,
    x: Fixed16_16,
    y: Fixed16_16,
) -> bool {
    let (m_radius, m_z, m_height, m_target, m_kind) = {
        let Some(mo) = gs.mobjslab.get(handle) else {
            return true;
        };
        (mo.radius, mo.z, mo.height, mo.target, mo.kind)
    };
    let base_damage = crate::projectile::projectile_info(m_kind)
        .map(|pi| pi.damage)
        .unwrap_or(0);
    let target_kind = gs.mobjslab.get(m_target).map(|mt| mt.kind);

    let initial_slot_count = gs.mobjslab.slot_count();
    let initial_generation = gs.mobjslab.next_generation();

    for i in 0..initial_slot_count {
        let Some(other) = gs.mobjslab.handle_at(i) else {
            continue;
        };
        if other == handle || other.generation >= initial_generation {
            continue;
        }
        let Some(t) = gs.mobjslab.get(other) else {
            continue;
        };
        // Vanilla gate: only SOLID / SPECIAL / SHOOTABLE things are considered.
        if t.flags & (flags::MF_SOLID | flags::MF_SPECIAL | flags::MF_SHOOTABLE) == 0 {
            continue;
        }
        let (tx, ty, tz, t_height, t_flags, t_kind) = (t.x, t.y, t.z, t.height, t.flags, t.kind);

        // blockdist = thing->radius + tmthing->radius; reject if bbox misses.
        let blockdist = (t.radius + m_radius).raw();
        if (tx.raw() - x.raw()).abs() >= blockdist || (ty.raw() - y.raw()).abs() >= blockdist {
            continue;
        }

        // See if it went over / under.
        if m_z.raw() > tz.raw() + t_height.raw() {
            continue; // overhead
        }
        if m_z.raw() + m_height.raw() < tz.raw() {
            continue; // underneath
        }

        // Don't hit same species as the originator (covers the shooter itself).
        if let Some(tk) = target_kind {
            let same_species = tk == t_kind
                || (tk == MobjKind::HellKnight && t_kind == MobjKind::BaronOfHell)
                || (tk == MobjKind::BaronOfHell && t_kind == MobjKind::HellKnight);
            if same_species {
                if other == m_target {
                    continue; // never hit the actor that fired it
                }
                if t_kind != MobjKind::Player {
                    // Explode, but do no damage (monsters don't hurt kin).
                    return false;
                }
            }
        }

        if t_flags & flags::MF_SHOOTABLE == 0 {
            // Didn't do any damage: block only if the obstacle is solid.
            return t_flags & flags::MF_SOLID == 0;
        }

        // Damage / explode. `(P_Random()%8+1)*damage` on the playsim stream.
        let damage = ((gs.p_random() % 8) as i32 + 1) * base_damage;
        crate::combat::damage_mobj_source(gs, other, handle, m_target, damage);
        return false; // don't traverse any more
    }

    true
}

/// Vanilla `P_ExplodeMissile` (`p_mobj.c`): stop the missile, switch it to its
/// death animation (which fires the death-state action, e.g. `A_Explode` for a
/// rocket), randomize the first death frame's duration by `P_Random()&3`, and
/// clear `MF_MISSILE` so it is no longer treated as a projectile.
fn p_explode_missile(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    let death_state = {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };
        mo.momx = Fixed16_16::ZERO;
        mo.momy = Fixed16_16::ZERO;
        mo.momz = Fixed16_16::ZERO;
        crate::mobjinfo::MOBJINFO[mo.kind as usize].death_state
    };

    // P_SetMobjState(deathstate) fires the death-state action (rocket ->
    // A_Explode -> radius attack) before the RNG draw below, matching vanilla.
    p_set_mobj_state(gs, handle, death_state, level);

    // th->tics -= P_Random()&3; if (th->tics < 1) th->tics = 1;
    let r = (gs.p_random() & 3) as i16;
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.tics -= r;
        if mo.tics < 1 {
            mo.tics = 1;
        }
        mo.flags &= !flags::MF_MISSILE;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, flags};
    use crate::player::PlayerState;
    use crate::states::ids;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::weapons::AmmoType;
    use doom_types::weapons::WeaponType;
    use doom_types::{Bam, Fixed16_16, TicCmd, bt};

    /// Construct a game state with a live player Mobj at the origin.
    fn make_game_state() -> GameState {
        let mut gs = GameState::new("test");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        ready_player_psprites(&mut gs);
        for _ in 0..24 {
            crate::weapons::tick_psprites(&mut gs, TicCmd::default(), None);
        }
        gs
    }

    fn ready_player_psprites(gs: &mut GameState) {
        crate::weapons::setup_psprites(&mut gs.player);
        for _ in 0..24 {
            crate::weapons::tick_psprites(gs, TicCmd::default(), None);
        }
    }

    fn count_mobjs_of_kind(gs: &GameState, kind: MobjKind) -> usize {
        gs.mobjslab
            .iter_handles()
            .filter_map(|handle| gs.mobjslab.get(handle))
            .filter(|mo| mo.kind == kind)
            .count()
    }

    /// Construct a non-player mobj (trooper) in a specific state with given tics.
    ///
    /// Placed at (5000, 5000), well outside A_Look's 4096 Manhattan distance
    /// from the player at origin. This ensures A_Look action fires but does
    /// NOT transition the trooper to see_state during pure state machine tests.
    fn make_trooper(state: StateNum, tics: i16) -> Mobj {
        let mut mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(5000),
            Fixed16_16::from_int(5000),
            Bam::ZERO,
        );
        mo.health = 30;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = state;
        mo.tics = tics;
        mo
    }

    /// Construct a missile mobj with momentum.
    fn make_missile(momx: i32, momy: i32, momz: i32) -> Mobj {
        let mut mo = Mobj::new(
            MobjKind::Rocket,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        mo.momx = Fixed16_16::from_int(momx);
        mo.momy = Fixed16_16::from_int(momy);
        mo.momz = Fixed16_16::from_int(momz);
        // Give it a state that will tick down.
        mo.state = StateNum(ids::S_POSS_STND); // reuse trooper idle (10 tics, loops)
        mo.tics = 5;
        mo.health = 1;
        mo
    }

    // =======================================================================
    // Tests: missile movement / collision (vanilla P_XYMovement / PIT_CheckThing
    // / P_ExplodeMissile)
    // =======================================================================

    /// Regression for the retired double-move: a missile must advance by its
    /// momentum EXACTLY ONCE per tic. The old engine integrated missiles once in
    /// `tick_mobj` AND again in a separate `p_move_projectiles` pass, doubling
    /// their speed (a fireball reached its target ~twice as fast). With one
    /// unified `P_XYMovement` path, one `tick_mobj` call = one momentum step.
    #[test]
    fn missile_moves_once_per_tic_not_twice() {
        let mut gs = make_game_state();
        let missile = make_missile(7, -4, 0);
        let handle = gs.mobjslab.alloc(missile);

        let _ = tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("missile exists");
        assert_eq!(
            mo.x,
            Fixed16_16::from_int(7),
            "x advances by momx exactly once"
        );
        assert_eq!(
            mo.y,
            Fixed16_16::from_int(-4),
            "y advances by momy exactly once"
        );
    }

    /// Regression pinning `P_ExplodeMissile`: switch the missile to its death
    /// state, zero its momentum, clear `MF_MISSILE`, and draw exactly one
    /// `P_Random()&3` (adjusting the death frame's tics, clamped to >= 1). The
    /// imp fireball death state carries no action, so the only RNG draw here is
    /// that single `&3`.
    #[test]
    fn p_explode_missile_enters_deathstate_clears_flag_and_draws_one_rng() {
        let mut gs = make_game_state();
        let mut mo = Mobj::new(
            MobjKind::ImpFireball,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        mo.momx = Fixed16_16::from_int(9);
        mo.momy = Fixed16_16::from_int(-2);
        mo.momz = Fixed16_16::from_int(1);
        mo.health = 1000;
        let handle = gs.mobjslab.alloc(mo);

        let death_state = crate::mobjinfo::MOBJINFO[MobjKind::ImpFireball as usize].death_state;
        let before = gs.rng.index();
        p_explode_missile(&mut gs, handle, None);
        assert_eq!(
            (gs.rng.index().wrapping_sub(before)) & 255,
            1,
            "P_ExplodeMissile must draw exactly one P_Random()&3"
        );

        let mo = gs.mobjslab.get(handle).expect("exploding missile exists");
        assert_eq!(mo.state, death_state, "missile enters its death state");
        assert_eq!(
            mo.flags & flags::MF_MISSILE,
            0,
            "MF_MISSILE must be cleared"
        );
        assert_eq!(mo.momx, Fixed16_16::ZERO);
        assert_eq!(mo.momy, Fixed16_16::ZERO);
        assert_eq!(mo.momz, Fixed16_16::ZERO);
        let base = crate::states::STATES[death_state.0 as usize].tics;
        assert!(
            mo.tics >= 1 && mo.tics <= base,
            "tics {} in [1,{base}]",
            mo.tics
        );
    }

    /// Regression pinning the `PIT_CheckThing` missile branch: a missile
    /// overlapping a shootable actor deals `(P_Random()%8 + 1) * damage` to it
    /// (drawing that `P_Random`), attributes the hit, and reports the step as
    /// blocked (so the caller explodes the missile). The imp fireball's base
    /// damage is 3, so the victim loses between 3 and 24 health.
    #[test]
    fn missile_check_things_damages_shootable_with_scaled_random_draw() {
        let mut gs = make_game_state();
        // A missile co-located with a shootable imp; fired by the player so the
        // player is skipped as the shooter.
        let mut proj = Mobj::new(
            MobjKind::ImpFireball,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        proj.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        proj.radius = Fixed16_16::from_int(6);
        proj.height = Fixed16_16::from_int(8);
        proj.health = 1000;
        proj.target = gs.player.handle;
        let missile = gs.mobjslab.alloc(proj);

        let mut victim = Mobj::new(MobjKind::Imp, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        victim.health = 60;
        victim.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        victim.radius = Fixed16_16::from_int(20);
        victim.height = Fixed16_16::from_int(56);
        let victim_h = gs.mobjslab.alloc(victim);

        let before = gs.rng.index();
        let blocked = !missile_check_things(&mut gs, missile, Fixed16_16::ZERO, Fixed16_16::ZERO);
        assert!(
            blocked,
            "overlapping a shootable must block the step (explode)"
        );
        assert!(
            (gs.rng.index().wrapping_sub(before)) & 255 >= 1,
            "the (P_Random()%8+1) damage draw must advance the RNG"
        );

        let victim = gs.mobjslab.get(victim_h).expect("victim exists");
        let dealt = 60 - victim.health;
        assert!(
            (3..=24).contains(&dealt),
            "imp fireball deals 3..=24 (=(1..=8)*3), dealt {dealt}"
        );
    }

    /// The missile must skip its own shooter (`mo.target`): a fireball flying
    /// over the imp that launched it passes straight through, dealing no damage
    /// and drawing no RNG.
    #[test]
    fn missile_check_things_skips_its_shooter() {
        let mut gs = make_game_state();
        // Place the shooter + missile well away from the origin player so only
        // the shooter is a collision candidate at the tested position.
        let px = Fixed16_16::from_int(500);
        let py = Fixed16_16::from_int(500);
        let mut shooter = Mobj::new(MobjKind::Imp, px, py, Bam::ZERO);
        shooter.health = 60;
        shooter.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        shooter.radius = Fixed16_16::from_int(20);
        shooter.height = Fixed16_16::from_int(56);
        let shooter_h = gs.mobjslab.alloc(shooter);

        let mut proj = Mobj::new(MobjKind::ImpFireball, px, py, Bam::ZERO);
        proj.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        proj.radius = Fixed16_16::from_int(6);
        proj.height = Fixed16_16::from_int(8);
        proj.health = 1000;
        proj.target = shooter_h; // fired BY this imp
        let missile = gs.mobjslab.alloc(proj);

        let before = gs.rng.index();
        let passed = missile_check_things(&mut gs, missile, px, py);
        assert!(passed, "missile must pass through its own shooter");
        assert_eq!(
            gs.rng.index(),
            before,
            "no RNG draw when skipping the shooter"
        );
        let shooter = gs.mobjslab.get(shooter_h).expect("shooter exists");
        assert_eq!(
            shooter.health, 60,
            "shooter takes no damage from its own missile"
        );
    }

    // =======================================================================
    // Tests: monster momentum (vanilla P_XYMovement)
    // =======================================================================

    #[test]
    fn walking_monster_gains_no_momentum_but_thrust_decays_by_friction() {
        // Regression for the vanilla demo-sync bug: `P_Move` used to inject the
        // walk step as momentum. Vanilla `P_Move` never does — a monster's
        // momentum comes only from thrust and is decayed each tic by
        // `P_XYMovement`'s FRICTION (0xE800). Here we give a trooper explicit
        // thrust momentum (as `P_DamageMobj` would) and tick it; its position
        // must advance by that momentum and the momentum must scale by FRICTION.
        let mut gs = make_game_state();
        let mut mo = make_trooper(StateNum(ids::S_POSS_STND), 100);
        // Above STOPSPEED (0x1000) so friction multiplies rather than zeroing.
        let momx = Fixed16_16::from_raw(0x30000); // 3.0 units/tic
        let momy = Fixed16_16::from_raw(-0x18000); // -1.5 units/tic
        mo.momx = momx;
        mo.momy = momy;
        let (x0, y0) = (mo.x, mo.y);
        let handle = gs.mobjslab.alloc(mo);

        let _ = tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper exists");
        // Position advanced by the pre-friction momentum (integration).
        assert_eq!(mo.x, x0 + momx, "x should advance by momx");
        assert_eq!(mo.y, y0 + momy, "y should advance by momy");
        // Momentum decayed by FRICTION exactly.
        assert_eq!(
            mo.momx,
            momx.fixed_mul(FRICTION),
            "momx must decay by FRICTION"
        );
        assert_eq!(
            mo.momy,
            momy.fixed_mul(FRICTION),
            "momy must decay by FRICTION"
        );
    }

    #[test]
    fn sub_stopspeed_monster_momentum_snaps_to_zero() {
        // Vanilla P_XYMovement snaps a resting thing's sub-STOPSPEED momentum to
        // zero (the player-input clause is vacuous for a monster).
        let mut gs = make_game_state();
        let mut mo = make_trooper(StateNum(ids::S_POSS_STND), 100);
        mo.momx = Fixed16_16::from_raw(0x800); // below STOPSPEED (0x1000)
        mo.momy = Fixed16_16::from_raw(-0x800);
        let handle = gs.mobjslab.alloc(mo);

        let _ = tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper exists");
        assert_eq!(mo.momx, Fixed16_16::ZERO, "sub-STOPSPEED momx snaps to 0");
        assert_eq!(mo.momy, Fixed16_16::ZERO, "sub-STOPSPEED momy snaps to 0");
    }

    // =======================================================================
    // Tests: p_set_mobj_state
    // =======================================================================

    #[test]
    fn p_set_mobj_state_sets_state_and_tics() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum::NULL, -1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = p_set_mobj_state(&mut gs, handle, StateNum(ids::S_POSS_STND), None);
        assert!(result, "p_set_mobj_state must return true for valid state");

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.state, StateNum(ids::S_POSS_STND));
        assert_eq!(mo.tics, 10, "POSS_STND has 10 tics");
    }

    #[test]
    fn p_set_mobj_state_returns_false_for_s_null() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let handle = gs.mobjslab.alloc(trooper);

        let result = p_set_mobj_state(&mut gs, handle, StateNum::NULL, None);
        assert!(!result, "p_set_mobj_state must return false for S_NULL");
    }

    #[test]
    fn p_set_mobj_state_fires_action_on_entry() {
        let mut gs = make_game_state();
        let mut trooper = make_trooper(StateNum::NULL, -1);
        // Set up target so A_Look can find something (won't crash without it).
        trooper.target = gs.player.handle;
        let handle = gs.mobjslab.alloc(trooper);

        // S_POSS_STND has ACTION_LOOK — this should not crash.
        let result = p_set_mobj_state(&mut gs, handle, StateNum(ids::S_POSS_STND), None);
        assert!(result);
    }

    #[test]
    fn p_set_mobj_state_no_action_for_none_action() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum::NULL, -1);
        let handle = gs.mobjslab.alloc(trooper);

        // S_POSS_DIE1 has Action::NoAction — should just set state.
        let result = p_set_mobj_state(&mut gs, handle, StateNum(ids::S_POSS_DIE1), None);
        assert!(result);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.state, StateNum(ids::S_POSS_DIE1));
        assert_eq!(mo.tics, 5, "POSS_DIE1 has 5 tics (vanilla)");
    }

    #[test]
    fn p_set_mobj_state_handles_freed_handle() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum::NULL, -1);
        let handle = gs.mobjslab.alloc(trooper);
        gs.mobjslab.free(handle);

        // Should not crash — the handle is stale.
        let result = p_set_mobj_state(&mut gs, handle, StateNum(ids::S_POSS_STND), None);
        // Returns true because the state lookup succeeds but the mobj mutation
        // is a no-op (get_mut returns None for freed handle).
        assert!(result);
    }

    #[test]
    fn p_set_mobj_state_out_of_bounds_state() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum::NULL, -1);
        let handle = gs.mobjslab.alloc(trooper);

        // State index 9999 is out of bounds.
        let result = p_set_mobj_state(&mut gs, handle, StateNum(9999), None);
        assert!(!result, "out-of-bounds state must return false");
    }

    // =======================================================================
    // Tests: tick_mobj
    // =======================================================================

    #[test]
    fn tick_mobj_decrements_tics() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let handle = gs.mobjslab.alloc(trooper);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.tics, 4, "tics should decrement from 5 to 4");
    }

    #[test]
    fn tick_mobj_transitions_at_zero() {
        let mut gs = make_game_state();
        // S_POSS_STND now alternates between idle frames A and B.
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.state, StateNum(ids::S_POSS_STND2));
        assert_eq!(mo.tics, 10, "should have reloaded tics from STATES table");
    }

    #[test]
    fn tick_mobj_holds_negative_tics() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum(ids::S_POSS_DIE2), -1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.tics, -1, "negative tics should hold indefinitely");
        assert_eq!(
            mo.state,
            StateNum(ids::S_POSS_DIE2),
            "state should not change"
        );
    }

    #[test]
    fn tick_mobj_chain_through_die1_to_die2() {
        let mut gs = make_game_state();
        // S_POSS_DIE1: 5 tics (vanilla), next = S_POSS_DIE2
        let trooper = make_trooper(StateNum(ids::S_POSS_DIE1), 1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.state, StateNum(ids::S_POSS_DIE2));
        assert_eq!(mo.tics, 5, "DIE2 runs for 5 tics before DIE3");
    }

    #[test]
    fn tick_mobj_missile_updates_position_every_tic() {
        let mut gs = make_game_state();
        let missile = make_missile(5, 3, 1);
        let handle = gs.mobjslab.alloc(missile);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        // Missiles advance by momentum every tic (matching P_MobjThinker).
        assert_eq!(
            mo.x,
            Fixed16_16::from_int(5),
            "missile x should advance by momx"
        );
        assert_eq!(
            mo.y,
            Fixed16_16::from_int(3),
            "missile y should advance by momy"
        );
        assert_eq!(
            mo.z,
            Fixed16_16::from_int(1),
            "missile z should advance by momz"
        );
    }

    #[test]
    fn tick_mobj_missile_position_updates_even_without_state_transition() {
        let mut gs = make_game_state();
        // Set tics high so it doesn't transition this tic.
        let mut missile = make_missile(5, 3, 1);
        missile.tics = 10;
        let handle = gs.mobjslab.alloc(missile);

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        // Missile position updates happen EVERY tic, not just on transition.
        assert_eq!(mo.tics, 9, "tics should have decremented");
        assert_eq!(mo.x, Fixed16_16::from_int(5), "missile x should advance");
        assert_eq!(mo.y, Fixed16_16::from_int(3), "missile y should advance");
        assert_eq!(mo.z, Fixed16_16::from_int(1), "missile z should advance");
    }

    #[test]
    fn tick_mobj_freed_handle_returns_remove() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let handle = gs.mobjslab.alloc(trooper);
        gs.mobjslab.free(handle);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Remove));
    }

    // =======================================================================
    // Tests: tick_all_mobjs
    // =======================================================================

    #[test]
    fn tick_all_mobjs_processes_all_actors() {
        let mut gs = make_game_state();
        let t1 = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let t2 = make_trooper(StateNum(ids::S_POSS_STND), 3);
        let h1 = gs.mobjslab.alloc(t1);
        let h2 = gs.mobjslab.alloc(t2);

        tick_all_mobjs(&mut gs, None);

        let mo1 = gs.mobjslab.get(h1).expect("mo1 should exist");
        assert_eq!(mo1.tics, 4, "first trooper tics should decrement");
        let mo2 = gs.mobjslab.get(h2).expect("mo2 should exist");
        assert_eq!(mo2.tics, 2, "second trooper tics should decrement");
    }

    #[test]
    fn tick_all_mobjs_skips_player() {
        let mut gs = make_game_state();
        // Set player mobj to a specific state with known tics.
        let player_handle = gs.player.handle;
        if let Some(mo) = gs.mobjslab.get_mut(player_handle) {
            mo.state = StateNum(ids::S_POSS_STND);
            mo.tics = 5;
        }

        tick_all_mobjs(&mut gs, None);

        let mo = gs.mobjslab.get(player_handle).expect("player should exist");
        assert_eq!(
            mo.tics, 5,
            "player mobj should not be ticked by tick_all_mobjs"
        );
    }

    #[test]
    fn actors_by_generation_orders_by_creation_not_slot() {
        // Reproduce the free-list slot-reuse that desyncs missile-vs-monster
        // thinker order from vanilla: after a low slot is recycled by a
        // later-created actor, iteration MUST still be in creation
        // (generation) order — the actor in the recycled low slot ticks LAST.
        let mut gs = make_game_state();
        let a = gs
            .mobjslab
            .alloc(make_trooper(StateNum(ids::S_POSS_STND), 5));
        let b = gs
            .mobjslab
            .alloc(make_trooper(StateNum(ids::S_POSS_STND), 5));
        // Free `a` (its low slot enters the free list) then allocate `c`, which
        // reuses `a`'s slot but carries a higher generation than `b`.
        gs.mobjslab.free(a);
        let c = gs.mobjslab.alloc(make_missile(1, 0, 0));

        assert!(
            c.index < b.index,
            "test precondition: `c` must reuse the lower recycled slot"
        );
        assert!(
            c.generation > b.generation,
            "test precondition: `c` is created after `b`"
        );

        let order = actors_by_generation(&gs.mobjslab);
        // Player (gen 1) first, then b, then c — creation order, not slot order.
        assert_eq!(
            order.last().copied(),
            Some(c),
            "recycled-slot actor `c` must tick last (highest generation)"
        );
        let bi = order.iter().position(|&h| h == b).unwrap();
        let ci = order.iter().position(|&h| h == c).unwrap();
        assert!(bi < ci, "`b` (older) must tick before `c` (newer)");
    }

    #[test]
    fn tick_world_interleaved_light_pass_matches_legacy_when_no_gameplay_actors() {
        use crate::movers::{LightThinkerKind, SectorLightEffect};

        // A pending LightFlash that fires (draws P_Random) on the next tic.
        fn pending_light() -> SectorLightEffect {
            SectorLightEffect {
                sector_index: 0,
                kind: LightThinkerKind::LightFlash,
                count: 1, // `--count == 0` this tic -> toggles and draws
                max_light: 200,
                min_light: 100,
                max_time: 64,
                min_time: 7,
                dark_time: 0,
                bright_time: 0,
                glow_dir: 0,
            }
        }

        // Legacy path (boundary unset): light pass runs after the mobj pass.
        let mut legacy = make_game_state();
        let mut lv_legacy = make_secret_level(0);
        legacy.movers.sector_lights.push(pending_light());
        assert_eq!(legacy.thinker_setup_boundary, u32::MAX);
        let idx_before = legacy.rng.index();
        tick_world(&mut legacy, Some(&mut lv_legacy));
        let legacy_light = lv_legacy.sectors[0].light_level;
        let legacy_draws = legacy.rng.index().wrapping_sub(idx_before);

        // Interleaved path (boundary frozen): the sector-light pass moves to the
        // vanilla thinker-list position. With no gameplay-spawned actors present
        // it must still tick the light exactly once, identically to legacy.
        let mut interleaved = make_game_state();
        let mut lv_inter = make_secret_level(0);
        interleaved.movers.sector_lights.push(pending_light());
        interleaved.freeze_thinker_setup_boundary();
        assert_ne!(interleaved.thinker_setup_boundary, u32::MAX);
        let idx_before = interleaved.rng.index();
        tick_world(&mut interleaved, Some(&mut lv_inter));
        let inter_light = lv_inter.sectors[0].light_level;
        let inter_draws = interleaved.rng.index().wrapping_sub(idx_before);

        assert_eq!(
            legacy_draws, inter_draws,
            "interleaved light pass must draw the same P_Random count as legacy"
        );
        assert_eq!(
            legacy_light, inter_light,
            "interleaved light pass must produce the same light level as legacy"
        );
        assert!(
            inter_draws >= 1,
            "the pending LightFlash must draw this tic"
        );
    }

    #[test]
    fn tick_all_mobjs_handles_empty_slab() {
        let mut gs = GameState::new("test");
        // No panic expected with empty slab.
        tick_all_mobjs(&mut gs, None);
    }

    #[test]
    fn tick_all_mobjs_skips_freed_mobjs() {
        let mut gs = make_game_state();
        let t1 = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let t2 = make_trooper(StateNum(ids::S_POSS_STND), 3);
        let h1 = gs.mobjslab.alloc(t1);
        let _h2 = gs.mobjslab.alloc(t2);
        gs.mobjslab.free(h1);

        // Should not panic.
        tick_all_mobjs(&mut gs, None);
    }

    // =======================================================================
    // Tests: tick_world
    // =======================================================================

    #[test]
    fn tick_world_increments_level_time() {
        let mut gs = make_game_state();
        assert_eq!(gs.stats.level_time, 0);
        tick_world(&mut gs, None);
        assert_eq!(gs.stats.level_time, 1);
        tick_world(&mut gs, None);
        assert_eq!(gs.stats.level_time, 2);
    }

    #[test]
    fn tick_world_advances_actor_states() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let handle = gs.mobjslab.alloc(trooper);

        tick_world(&mut gs, None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.tics, 4, "tick_world should advance actor state machines");
    }

    #[test]
    fn tick_world_processes_scrolling_walls() {
        let mut gs = make_game_state();
        gs.movers.scrolling_walls.push(crate::state::ScrollingWall {
            linedef_index: 0,
            speed_x: 1,
            speed_y: 0,
            accumulated_x: 0,
            accumulated_y: 0,
        });

        tick_world(&mut gs, None);

        assert_eq!(
            gs.movers.scrolling_walls[0].accumulated_x, 1,
            "tick_world must advance scrolling walls"
        );
    }

    #[test]
    fn tick_world_level_time_wraps() {
        let mut gs = make_game_state();
        gs.stats.level_time = u32::MAX;
        tick_world(&mut gs, None);
        assert_eq!(gs.stats.level_time, 0, "level_time must wrap at u32::MAX");
    }

    // =======================================================================
    // Tests: tick_player
    // =======================================================================

    #[test]
    fn tick_player_applies_angle_turn() {
        let mut gs = make_game_state();
        let cmd = TicCmd {
            angle_turn: 640,
            ..Default::default()
        };
        tick_player(&mut gs, cmd, None);

        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        let expected = Bam((640i32 as u32).wrapping_shl(16));
        assert_eq!(mo.angle, expected);
    }

    #[test]
    fn tick_player_applies_friction() {
        let mut gs = make_game_state();
        gs.mobjslab
            .get_mut(gs.player.handle)
            .expect("item must exist in tests")
            .momx = Fixed16_16::from_int(4);

        tick_player(&mut gs, TicCmd::default(), None);

        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        assert!(
            mo.momx < Fixed16_16::from_int(4),
            "friction must reduce momx"
        );
        assert!(
            mo.momx > Fixed16_16::ZERO,
            "friction must not zero momx in one step"
        );
    }

    #[test]
    fn tick_player_first_pistol_shot_is_accurate() {
        // SAFETY: trig tables are process-global and internally guarded.
        doom_types::Bam::init_trig_tables();

        let mut gs = make_game_state();
        let mut trooper = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(512),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        trooper.health = 20;
        trooper.radius = Fixed16_16::from_int(8);
        trooper.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        let trooper_handle = gs.mobjslab.alloc(trooper);
        gs.rng.set_index(16);
        gs.player.attack_down = false;

        // Vanilla A_FirePistol is on PISTOL2, so the shot lands 4 tics into the
        // fire animation (fifth tick_player call).
        for _ in 0..5 {
            tick_player(
                &mut gs,
                TicCmd {
                    buttons: bt::BT_ATTACK,
                    ..Default::default()
                },
                None,
            );
        }

        assert!(
            gs.mobjslab
                .get(trooper_handle)
                .expect("item must exist in tests")
                .health
                < 20,
            "first pistol shot through tick_player should be accurate"
        );
    }

    #[test]
    fn weapon_fire_samples_pre_move_player_position() {
        // Regression: the weapon action functions (`A_FirePistol` etc.) must
        // sample the player origin vanilla samples. In vanilla `P_MovePlayer`
        // only sets momentum; the position is integrated later in
        // `P_XYMovement` (during `P_RunThinkers`), which runs AFTER
        // `P_MovePsprites`. So the hitscan fires from the pre-integration
        // position (this tic's angle, the previous tic's position). doom-rs
        // integrates the player position inline in `p_move_player`, so
        // `tick_player` must restore the pre-move position for the psprite tick.
        //
        // Here the player moves +y perpendicular to its +x aim. A shot from the
        // pre-move position (y=0) hits a trooper on the x-axis; a shot from the
        // post-move position (y=30, one MAXMOVE step north) would sail 30 units
        // over the 16-unit-radius trooper and miss.
        doom_types::Bam::init_trig_tables();

        let mut gs = make_game_state();

        // Trooper straddling the x-axis, 512 units east of the player.
        let mut trooper = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(512),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        trooper.health = 20;
        trooper.radius = Fixed16_16::from_int(16);
        trooper.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        let trooper_handle = gs.mobjslab.alloc(trooper);
        gs.rng.set_index(16);

        // Give the player northward momentum (clamped to MAXMOVE = 30) and force
        // the pistol into the frame just before `A_FirePistol` fires, so the shot
        // resolves on this single `tick_player` call.
        {
            let mo = gs
                .mobjslab
                .get_mut(gs.player.handle)
                .expect("player must exist");
            mo.momy = Fixed16_16::from_int(40);
        }
        gs.player.psprites[crate::player::psprite_slots::WEAPON] = crate::player::PspriteState {
            state: StateNum(ids::S_PISTOL1),
            tics: 1,
            sx: 0,
            sy: 0,
        };

        tick_player(
            &mut gs,
            TicCmd {
                buttons: bt::BT_ATTACK,
                ..Default::default()
            },
            None,
        );

        // The fire sampled the pre-move (y=0) origin, so the trooper is hit.
        assert!(
            gs.mobjslab
                .get(trooper_handle)
                .expect("trooper must exist")
                .health
                < 20,
            "weapon fire must sample the pre-move player position (vanilla \
             P_MovePsprites runs before player position integration)"
        );

        // The player mobj still ends the tic at its real post-move position: the
        // pre-move restore is only in effect for the psprite tick.
        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player must exist");
        assert_eq!(
            mo.y,
            Fixed16_16::from_int(30),
            "player must end the tic at the integrated post-move position"
        );
    }

    #[test]
    fn dead_player_ignores_input_and_clears_held_buttons() {
        // SAFETY: trig tables are process-global and internally guarded.
        doom_types::Bam::init_trig_tables();

        let mut gs = make_game_state();
        gs.player.apply_damage(200);
        gs.player.attack_down = true;
        gs.player.use_down = true;
        gs.mobjslab
            .get_mut(gs.player.handle)
            .expect("item must exist in tests")
            .health = 0;
        let start_ammo = gs.player.ammo(AmmoType::Bullets as usize);

        tick_player(
            &mut gs,
            TicCmd {
                forward_move: 50,
                angle_turn: 640,
                buttons: bt::BT_ATTACK | bt::BT_USE,
                ..Default::default()
            },
            None,
        );

        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        assert_eq!(mo.x, Fixed16_16::ZERO, "dead player must not move");
        assert_eq!(mo.y, Fixed16_16::ZERO, "dead player must not move");
        assert_eq!(mo.angle, Bam::ZERO, "dead player must not turn");
        assert_eq!(
            gs.player.ammo(AmmoType::Bullets as usize),
            start_ammo,
            "dead player must not fire"
        );
        assert!(!gs.player.attack_down, "dead player must clear held attack");
        assert!(!gs.player.use_down, "dead player must clear held use");
    }

    #[test]
    fn held_pistol_refires_when_the_attack_chain_reenters_ready() {
        let mut gs = make_game_state();
        let cmd = TicCmd {
            buttons: bt::BT_ATTACK,
            ..Default::default()
        };

        // Vanilla pistol: A_FirePistol is on PISTOL2, so the first shot lands
        // 4 tics into the fire animation.
        for _ in 0..5 {
            tick_player(&mut gs, cmd, None);
        }
        assert_eq!(gs.player.ammo(AmmoType::Bullets as usize), 49);

        // The vanilla fire cycle is 14 tics (PISTOL1..PISTOL3 before A_ReFire),
        // so the next shot is not until tic 18.
        for _ in 0..13 {
            tick_player(&mut gs, cmd, None);
        }
        assert_eq!(
            gs.player.ammo(AmmoType::Bullets as usize),
            49,
            "held pistol should not refire before its psprite chain reenters the ready state"
        );

        tick_player(&mut gs, cmd, None);
        assert_eq!(
            gs.player.ammo(AmmoType::Bullets as usize),
            48,
            "held pistol should refire once the vanilla 14-tic cycle completes"
        );
    }

    #[test]
    fn held_shotgun_refires_when_the_attack_chain_reenters_ready() {
        let mut gs = make_game_state();
        gs.player.weapons[WeaponType::Shotgun as usize] = true;
        gs.player.weapon = WeaponType::Shotgun;
        gs.player.give_ammo(AmmoType::Shells as usize, 4);
        ready_player_psprites(&mut gs);
        let cmd = TicCmd {
            buttons: bt::BT_ATTACK,
            ..Default::default()
        };

        // Vanilla shotgun: A_FireShotgun is on SGUN2, so the first shot lands
        // 3 tics into the fire animation.
        for _ in 0..4 {
            tick_player(&mut gs, cmd, None);
        }
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), 3);

        // Vanilla shotgun fire cycle is 37 tics (SGUN1..SGUN8 before A_ReFire),
        // so the next shot is not until tic 40.
        for _ in 0..36 {
            tick_player(&mut gs, cmd, None);
        }
        assert_eq!(
            gs.player.ammo(AmmoType::Shells as usize),
            3,
            "held shotgun should not refire before its psprite chain reenters ready"
        );

        tick_player(&mut gs, cmd, None);
        assert_eq!(
            gs.player.ammo(AmmoType::Shells as usize),
            2,
            "held shotgun should refire once the vanilla 37-tic cycle completes"
        );
    }

    #[test]
    fn rocket_launcher_has_a_windup_and_auto_refires_when_held() {
        let mut gs = make_game_state();
        gs.player.weapons[WeaponType::RocketLauncher as usize] = true;
        gs.player.weapon = WeaponType::RocketLauncher;
        gs.player.give_ammo(AmmoType::Rockets as usize, 3);
        ready_player_psprites(&mut gs);
        let rockets_before = count_mobjs_of_kind(&gs, MobjKind::Rocket);
        let cmd = TicCmd {
            buttons: bt::BT_ATTACK,
            ..Default::default()
        };

        tick_player(&mut gs, cmd, None);
        assert_eq!(
            gs.player.psprites[crate::player::psprite_slots::WEAPON].state,
            StateNum(ids::S_MISSILE1),
            "the first launcher attack tic should enter the windup state"
        );
        assert_eq!(gs.player.ammo(AmmoType::Rockets as usize), 3);
        assert_eq!(
            count_mobjs_of_kind(&gs, MobjKind::Rocket),
            rockets_before,
            "the launcher windup state must not spawn a rocket yet"
        );

        for _ in 0..8 {
            tick_player(&mut gs, cmd, None);
        }

        assert_eq!(
            gs.player.psprites[crate::player::psprite_slots::WEAPON].state,
            StateNum(ids::S_MISSILE2),
            "after the windup, the launcher should advance into its fire state"
        );
        assert_eq!(gs.player.ammo(AmmoType::Rockets as usize), 2);
        assert_eq!(
            count_mobjs_of_kind(&gs, MobjKind::Rocket),
            rockets_before + 1,
            "the launcher should spawn a rocket when the fire state begins"
        );

        // Vanilla `S_MISSILE3` carries A_ReFire, so a held launcher auto-refires
        // once the ~20-tic fire cycle completes (next rocket near tic 28).
        for _ in 0..20 {
            tick_player(&mut gs, cmd, None);
        }

        assert_eq!(
            gs.player.ammo(AmmoType::Rockets as usize),
            1,
            "held rocket launcher auto-refires via A_ReFire once its cycle completes"
        );
    }

    #[test]
    fn tick_player_weapon_change() {
        use doom_types::weapons::WeaponType;
        let mut gs = make_game_state();
        gs.player.weapons[WeaponType::Shotgun as usize] = true;

        let cmd = TicCmd {
            buttons: bt::BT_CHANGE | (2u8 << 3),
            ..Default::default()
        };
        tick_player(&mut gs, cmd, None);

        assert_eq!(gs.player.weapon, WeaponType::Pistol);
        assert_eq!(gs.player.pending_weapon, Some(WeaponType::Shotgun));
        assert_eq!(
            gs.player.psprites[crate::player::psprite_slots::WEAPON].state,
            StateNum(ids::S_PISTOL_DOWN)
        );
    }

    #[test]
    fn tick_player_no_change_unowned_weapon() {
        use doom_types::weapons::WeaponType;
        let mut gs = make_game_state();
        gs.player.weapons[WeaponType::Shotgun as usize] = false;
        let original = gs.player.weapon;

        let cmd = TicCmd {
            buttons: bt::BT_CHANGE | (2u8 << 3),
            ..Default::default()
        };
        tick_player(&mut gs, cmd, None);

        assert_eq!(gs.player.weapon, original);
    }

    #[test]
    fn tick_player_missing_handle_is_noop() {
        let mut gs = GameState::new("test");
        // player.handle defaults to MobjHandle::NULL -- no panic expected.
        tick_player(
            &mut gs,
            TicCmd {
                forward_move: 50,
                ..Default::default()
            },
            None,
        );
    }

    // =======================================================================
    // Tests: state transition chains
    // =======================================================================

    #[test]
    fn tick_mobj_invalid_state_transitions_to_null() {
        let mut gs = make_game_state();
        let invalid_state = StateNum(crate::states::STATES.len() as u16 + 100);
        let trooper = make_trooper(invalid_state, 1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = tick_mobj(&mut gs, handle, None);

        // When transitioning to StateNum::NULL, p_set_mobj_state removes it and returns false,
        // which makes tick_mobj return Remove.
        assert!(matches!(result, TickMobjResult::Remove));
    }

    #[test]
    fn state_transition_run1_to_run2() {
        let mut gs = make_game_state();
        // S_POSS_RUN1: 4 tics, next = S_POSS_RUN2, action = A_Chase.
        // A_Chase needs a valid alive target or it reverts to idle.
        let trooper = make_trooper(StateNum(ids::S_POSS_RUN1), 1);
        let handle = gs.mobjslab.alloc(trooper);
        // Set the player as the trooper's target so A_Chase doesn't revert.
        gs.mobjslab
            .get_mut(handle)
            .expect("item must exist in tests")
            .target = gs.player.handle;

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.state, StateNum(ids::S_POSS_RUN2));
        assert_eq!(mo.tics, 4);
    }

    #[test]
    fn state_transition_run4_back_to_run1() {
        let mut gs = make_game_state();
        // S_POSS_RUN4: 4 tics, next = S_POSS_RUN1, action = A_Chase.
        let trooper = make_trooper(StateNum(ids::S_POSS_RUN4), 1);
        let handle = gs.mobjslab.alloc(trooper);
        gs.mobjslab
            .get_mut(handle)
            .expect("item must exist in tests")
            .target = gs.player.handle;

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.state, StateNum(ids::S_POSS_RUN1));
        assert_eq!(mo.tics, 4);
    }

    #[test]
    fn imp_run_state_advances_every_three_tics() {
        // Regression for the RUN-cadence bug: a chasing imp (TROO) must hold each
        // RUN state for 3 tics (vanilla info.c), not 4. Enter S_TROO_RUN1 fresh
        // (tics loaded from the table via p_set_mobj_state), keep a live target so
        // A_Chase does not revert to idle, and confirm it advances to RUN2 on the
        // 3rd tick and not before.
        let mut gs = make_game_state();
        let mut imp = make_trooper(StateNum(ids::S_TROO_STND), 1);
        imp.kind = MobjKind::Imp;
        let handle = gs.mobjslab.alloc(imp);
        gs.mobjslab
            .get_mut(handle)
            .expect("imp must exist in tests")
            .target = gs.player.handle;

        // Enter RUN1 fresh so tics come from the STATES table.
        assert!(p_set_mobj_state(
            &mut gs,
            handle,
            StateNum(ids::S_TROO_RUN1),
            None
        ));
        assert_eq!(
            gs.mobjslab.get(handle).unwrap().tics,
            3,
            "TROO_RUN1 must load 3 tics"
        );

        // Two ticks: still in RUN1.
        tick_mobj(&mut gs, handle, None);
        assert_eq!(
            gs.mobjslab.get(handle).unwrap().state,
            StateNum(ids::S_TROO_RUN1)
        );
        tick_mobj(&mut gs, handle, None);
        assert_eq!(
            gs.mobjslab.get(handle).unwrap().state,
            StateNum(ids::S_TROO_RUN1)
        );

        // Third tick: advances to RUN2 (cadence = 3, not 4).
        tick_mobj(&mut gs, handle, None);
        assert_eq!(
            gs.mobjslab.get(handle).unwrap().state,
            StateNum(ids::S_TROO_RUN2),
            "imp must advance RUN state every 3 tics, not 4"
        );
    }

    #[test]
    fn state_transition_pain_without_target_returns_to_idle() {
        let mut gs = make_game_state();
        // S_POSS_PAIN now resumes S_POSS_RUN1, but without a target A_Chase
        // immediately falls back to idle.
        let trooper = make_trooper(StateNum(ids::S_POSS_PAIN), 1);
        let handle = gs.mobjslab.alloc(trooper);

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.state, StateNum(ids::S_POSS_STND));
        assert_eq!(mo.tics, 10);
    }

    #[test]
    fn state_transition_atk3_to_run1() {
        let mut gs = make_game_state();
        // S_POSS_ATK3: 4 tics, next = S_POSS_RUN1, action = Action::NoAction.
        // But S_POSS_RUN1 entry fires A_Chase, which needs a target.
        let trooper = make_trooper(StateNum(ids::S_POSS_ATK3), 1);
        let handle = gs.mobjslab.alloc(trooper);
        gs.mobjslab
            .get_mut(handle)
            .expect("item must exist in tests")
            .target = gs.player.handle;

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.state, StateNum(ids::S_POSS_RUN1));
        assert_eq!(mo.tics, 4);
    }

    #[test]
    fn full_death_sequence_die1_through_die2() {
        let mut gs = make_game_state();
        // S_POSS_DIE1: 8 tics, next = S_POSS_DIE2 (8 tics, chains to die3)
        let trooper = make_trooper(StateNum(ids::S_POSS_DIE1), 8);
        let handle = gs.mobjslab.alloc(trooper);

        // Tick 8 times to count down DIE1 (7 decrements + 1 transition).
        for _ in 0..7 {
            let result = tick_mobj(&mut gs, handle, None);
            assert!(matches!(result, TickMobjResult::Alive));
        }
        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(
            mo.state,
            StateNum(ids::S_POSS_DIE1),
            "should still be in DIE1"
        );
        assert_eq!(mo.tics, 1, "one tic left");

        // Final tick transitions to DIE2.
        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.state, StateNum(ids::S_POSS_DIE2));
        assert_eq!(mo.tics, 5, "DIE2 runs for 5 tics before chaining to DIE3");
    }

    // =======================================================================
    // Tests: S_NULL removal
    // =======================================================================

    #[test]
    fn tick_all_mobjs_removes_s_null_actors() {
        let mut gs = make_game_state();
        // S_POSS_DIE5 has next_state = S_NULL and tics = -1 (holds forever).
        // Override tics to 1 so the transition to S_NULL fires.
        let mut trooper = make_trooper(StateNum(ids::S_POSS_DIE5), 1);
        trooper.health = 0;
        let handle = gs.mobjslab.alloc(trooper);

        // Before tick: the mobj exists.
        assert!(gs.mobjslab.get(handle).is_some());

        tick_all_mobjs(&mut gs, None);

        // After tick: the mobj should have been removed.
        assert!(
            gs.mobjslab.get(handle).is_none(),
            "mobj that transitioned to S_NULL must be removed from slab"
        );
    }

    #[test]
    fn s_null_removal_does_not_affect_other_actors() {
        let mut gs = make_game_state();
        // One trooper that will be removed (S_NULL next from DIE5).
        let mut dying = make_trooper(StateNum(ids::S_POSS_DIE5), 1);
        dying.health = 0;
        let dying_handle = gs.mobjslab.alloc(dying);

        // One trooper that stays alive.
        let alive = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let alive_handle = gs.mobjslab.alloc(alive);

        tick_all_mobjs(&mut gs, None);

        assert!(
            gs.mobjslab.get(dying_handle).is_none(),
            "dying mobj should be removed"
        );
        assert!(
            gs.mobjslab.get(alive_handle).is_some(),
            "alive mobj should remain"
        );
        assert_eq!(
            gs.mobjslab
                .get(alive_handle)
                .expect("item must exist in tests")
                .tics,
            4
        );
    }

    // =======================================================================
    // Tests: projectile movement via tick_mobj
    // =======================================================================

    #[test]
    fn missile_position_advances_on_tick() {
        let mut gs = make_game_state();
        let missile = make_missile(10, -5, 2);
        let handle = gs.mobjslab.alloc(missile);

        // tics is 5, so it won't transition yet — but wait, the MF_MISSILE
        // position update only happens on state transition in tick_mobj.
        // Set tics to 1 so it transitions.
        gs.mobjslab
            .get_mut(handle)
            .expect("item must exist in tests")
            .tics = 1;

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(mo.x, Fixed16_16::from_int(10));
        assert_eq!(mo.y, Fixed16_16::from_int(-5));
        assert_eq!(mo.z, Fixed16_16::from_int(2));
    }

    #[test]
    fn non_missile_advances_by_momentum() {
        // Vanilla `P_MobjThinker` runs `P_XYMovement` for ANY mobj carrying
        // momentum, integrating it into position before the state machine. A
        // monster holding thrust momentum therefore advances each tic (and the
        // momentum then decays by friction).
        let mut gs = make_game_state();
        let mut trooper = make_trooper(StateNum(ids::S_POSS_STND), 100);
        let momx = Fixed16_16::from_int(5);
        let momy = Fixed16_16::from_int(3);
        trooper.momx = momx;
        trooper.momy = momy;
        let handle = gs.mobjslab.alloc(trooper);

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(
            mo.x,
            Fixed16_16::from_int(5000) + momx,
            "non-missile x should advance by momx"
        );
        assert_eq!(
            mo.y,
            Fixed16_16::from_int(5000) + momy,
            "non-missile y should advance by momy"
        );
    }

    // =======================================================================
    // Tests: backward-compatible tick
    // =======================================================================

    #[test]
    fn tick_increments_tic_num() {
        let mut gs = make_game_state();
        assert_eq!(gs.tic_num, 0);
        gs.tick(TicCmd::default(), None);
        assert_eq!(gs.tic_num, 1);
        gs.tick(TicCmd::default(), None);
        assert_eq!(gs.tic_num, 2);
    }

    #[test]
    fn tic_num_wraps_at_u32_max() {
        let mut gs = make_game_state();
        gs.tic_num = u32::MAX;
        gs.tick(TicCmd::default(), None);
        assert_eq!(gs.tic_num, 0);
    }

    #[test]
    fn no_input_keeps_player_at_origin() {
        // Trig tables not initialized -> sin/cos = 0 -> no thrust -> no movement.
        let mut gs = make_game_state();
        gs.tick(TicCmd::default(), None);
        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        assert_eq!(mo.x, Fixed16_16::ZERO);
        assert_eq!(mo.y, Fixed16_16::ZERO);
    }

    #[test]
    fn angle_turn_updates_mobj_angle() {
        let mut gs = make_game_state();
        let cmd = TicCmd {
            angle_turn: 640,
            ..Default::default()
        };
        gs.tick(cmd, None);
        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        // angle_turn = 640 << 16 in 32-bit BAM.
        let expected = Bam((640i32 as u32).wrapping_shl(16));
        assert_eq!(mo.angle, expected);
    }

    #[test]
    fn friction_drains_existing_momentum() {
        let mut gs = make_game_state();
        // Inject momentum directly (bypassing thrust, since trig tables uninitialized).
        gs.mobjslab
            .get_mut(gs.player.handle)
            .expect("item must exist in tests")
            .momx = Fixed16_16::from_int(4);

        gs.tick(TicCmd::default(), None);

        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        // After FRICTION (approx 0.906): 4 -> ~3.625. Must be < 4 and > 0.
        assert!(
            mo.momx < Fixed16_16::from_int(4),
            "friction must reduce momx"
        );
        assert!(
            mo.momx > Fixed16_16::ZERO,
            "friction must not zero momx in one step"
        );
    }

    #[test]
    fn momentum_carries_position_forward() {
        let mut gs = make_game_state();
        gs.mobjslab
            .get_mut(gs.player.handle)
            .expect("item must exist in tests")
            .momx = Fixed16_16::from_int(2);

        gs.tick(TicCmd::default(), None);

        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        // x started at 0, momx = 2 -> x = 2 after one tick.
        assert_eq!(mo.x, Fixed16_16::from_int(2));
    }

    #[test]
    fn maxmove_clamps_excessive_velocity() {
        let mut gs = make_game_state();
        // Inject extreme velocity.
        gs.mobjslab
            .get_mut(gs.player.handle)
            .expect("item must exist in tests")
            .momx = Fixed16_16::from_int(1000);

        gs.tick(TicCmd::default(), None);

        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        // After friction + clamp: must be <= MAXMOVE.
        assert!(
            mo.momx <= MAXMOVE,
            "momx={:?} must be clamped to MAXMOVE={:?}",
            mo.momx,
            MAXMOVE
        );
    }

    #[test]
    fn missing_player_handle_is_noop() {
        // If the player handle is NULL, p_move_player should silently do nothing.
        let mut gs = GameState::new("test");
        // player.handle defaults to MobjHandle::NULL -- no panic expected.
        gs.tick(
            TicCmd {
                forward_move: 50,
                ..Default::default()
            },
            None,
        );
        assert_eq!(gs.tic_num, 1);
    }

    #[test]
    fn ticcmd_size_is_8_bytes() {
        assert_eq!(std::mem::size_of::<TicCmd>(), 8);
    }

    #[test]
    fn bt_change_switches_weapon_when_owned() {
        use doom_types::weapons::WeaponType;
        let mut gs = make_game_state();
        // Give player the shotgun.
        gs.player.weapons[WeaponType::Shotgun as usize] = true;
        gs.player.give_ammo(AmmoType::Shells as usize, 4);
        // BT_CHANGE | (weapon_num=2 << 3) = 0x04 | 0x10 = 0x14
        let cmd = TicCmd {
            buttons: bt::BT_CHANGE | (2u8 << 3),
            ..Default::default()
        };
        gs.tick(cmd, None);
        assert_eq!(
            gs.player.weapon,
            WeaponType::Pistol,
            "psprite-owned switching keeps the old weapon active until the lower/raise transition completes"
        );
        assert_eq!(
            gs.player.pending_weapon,
            Some(WeaponType::Shotgun),
            "owned BT_CHANGE should stage the requested weapon"
        );
        for _ in 0..48 {
            gs.tick(TicCmd::default(), None);
        }
        assert_eq!(
            gs.player.weapon,
            WeaponType::Shotgun,
            "staged weapon should become active after the lower/raise transition"
        );
    }

    #[test]
    fn bt_change_ignores_unowned_weapon() {
        use doom_types::weapons::WeaponType;
        let mut gs = make_game_state();
        // Confirm player does NOT have the shotgun.
        gs.player.weapons[WeaponType::Shotgun as usize] = false;
        let original_weapon = gs.player.weapon;

        // Attempt to switch to shotgun (weapon_num=2).
        let cmd = TicCmd {
            buttons: bt::BT_CHANGE | (2u8 << 3),
            ..Default::default()
        };
        gs.tick(cmd, None);

        assert_eq!(
            gs.player.weapon, original_weapon,
            "Should not switch to unowned weapon"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: level_time
    // -----------------------------------------------------------------------

    #[test]
    fn level_time_increments_each_tick() {
        let mut gs = make_game_state();
        assert_eq!(gs.stats.level_time, 0, "level_time starts at 0");
        gs.tick(TicCmd::default(), None);
        assert_eq!(
            gs.stats.level_time, 1,
            "level_time must be 1 after first tick"
        );
        gs.tick(TicCmd::default(), None);
        assert_eq!(
            gs.stats.level_time, 2,
            "level_time must be 2 after second tick"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: exit_request cleared each tick
    // -----------------------------------------------------------------------

    #[test]
    fn exit_request_cleared_at_tick_start() {
        let mut gs = make_game_state();
        gs.exit_request = Some(crate::state::ExitRequest::Normal);
        gs.tick(TicCmd::default(), None);
        assert_eq!(
            gs.exit_request, None,
            "exit_request must be cleared at the start of each tick"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: secret sector detection (special type 9)
    // -----------------------------------------------------------------------

    fn make_secret_level(floor_height: i16) -> doom_map::Level {
        let bm = make_minimal_blockmap();
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("item must exist in tests");
        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height,
                ceil_height: floor_height + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 9, // secret sector
                tag: 0,
            }],
            reject,
            blockmap: bm,
        }
    }

    fn make_minimal_blockmap() -> doom_map::Blockmap {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        doom_map::Blockmap::parse_lump(&bm_data).expect("item must exist in tests")
    }

    fn make_partition_step_level(right_floor: i16, left_floor: i16) -> doom_map::Level {
        use doom_map::{
            Blockmap, FLAG_TWO_SIDED, Linedef, Node, NodeBBox, Reject, Sector, Seg, Sidedef,
            Ssector, Vertex, lumps::NODE_SUBSECTOR_BIT,
        };

        let vertexes = vec![
            Vertex { x: 64, y: -128 },
            Vertex { x: 64, y: 128 },
            Vertex { x: 0, y: -128 },
            Vertex { x: 0, y: 128 },
        ];
        let linedefs = vec![
            Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: FLAG_TWO_SIDED,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: FLAG_TWO_SIDED,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
        ];
        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
        ];
        let sectors = vec![
            Sector {
                floor_height: right_floor,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: left_floor,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        let segs = vec![
            Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            },
            Seg {
                from_vertex: 3,
                to_vertex: 2,
                angle: 0,
                linedef: 1,
                direction: 1,
                offset: 0,
            },
        ];
        let ssectors = vec![
            Ssector {
                seg_count: 1,
                first_seg: 0,
            },
            Ssector {
                seg_count: 1,
                first_seg: 1,
            },
        ];
        let nodes = vec![Node {
            x: 64,
            y: 0,
            dx: 0,
            dy: 1,
            right_bbox: NodeBBox {
                ymax: 128,
                ymin: -128,
                xmin: 64,
                xmax: 256,
            },
            left_bbox: NodeBBox {
                ymax: 128,
                ymin: -128,
                xmin: -128,
                xmax: 64,
            },
            right_child: NODE_SUBSECTOR_BIT,
            left_child: NODE_SUBSECTOR_BIT | 1,
        }];

        let mut bm_data = Vec::new();
        bm_data.extend_from_slice(&0i16.to_le_bytes());
        bm_data.extend_from_slice(&0i16.to_le_bytes());
        bm_data.extend_from_slice(&1u16.to_le_bytes());
        bm_data.extend_from_slice(&1u16.to_le_bytes());
        let data_start = 4u16 + 1;
        bm_data.extend_from_slice(&data_start.to_le_bytes());
        bm_data.extend_from_slice(&0u16.to_le_bytes());
        bm_data.extend_from_slice(&0u16.to_le_bytes());
        bm_data.extend_from_slice(&1u16.to_le_bytes());
        bm_data.extend_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("item must exist in tests");
        let reject = Reject::parse_lump(&[0u8], 2).expect("item must exist in tests");

        doom_map::Level {
            name: "STEP".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes,
            sectors,
            reject,
            blockmap,
        }
    }

    #[test]
    fn secret_sector_increments_secret_count() {
        let mut gs = make_game_state();
        // Player mobj is at z=0, matching the secret sector floor.
        let mut level = make_secret_level(0);
        assert_eq!(gs.player.secret_count, 0);

        gs.tick(TicCmd::default(), Some(&mut level));

        assert_eq!(
            gs.player.secret_count, 1,
            "entering a secret sector (special=9) must increment secret_count"
        );
    }

    #[test]
    fn secret_sector_only_counts_once() {
        let mut gs = make_game_state();
        let mut level = make_secret_level(0);

        gs.tick(TicCmd::default(), Some(&mut level));
        assert_eq!(gs.player.secret_count, 1);
        assert_eq!(
            level.sectors[0].special, 0,
            "secret sector special must be cleared after discovery"
        );

        // Tick again -- sector no longer has special=9, count must not increase.
        gs.tick(TicCmd::default(), Some(&mut level));
        assert_eq!(
            gs.player.secret_count, 1,
            "secret_count must not increment again after sector special is cleared"
        );
    }

    #[test]
    fn damaging_floor_in_gameplay_is_periodic_not_every_tic() {
        let mut gs = make_game_state();
        let mut level = make_damage_level(0, 5);

        for _ in 0..33 {
            gs.tick(TicCmd::default(), Some(&mut level));
        }

        let health = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("item must exist in tests")
            .health;
        assert_eq!(
            health, 80,
            "vanilla hellslime (special 5) deals 10 damage on leveltime 0 and 32 only"
        );
    }

    #[test]
    fn tick_player_keeps_support_floor_while_descending_partial_dropoff() {
        let mut gs = make_game_state();
        let mut level = make_partition_step_level(64, 0);
        let mo = gs
            .mobjslab
            .get_mut(gs.player.handle)
            .expect("player should exist");
        mo.flags |= flags::MF_DROPOFF;
        mo.x = Fixed16_16::from_int(96);
        mo.y = Fixed16_16::ZERO;
        mo.z = Fixed16_16::from_int(64);
        // Vanilla P_XYMovement clamps momentum to MAXMOVE (30) before moving,
        // so this -40 request advances only -30 units this tic (96 -> 66).
        mo.momx = Fixed16_16::from_int(-40);

        tick_player(&mut gs, TicCmd::default(), Some(&mut level));

        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        assert_eq!(mo.x, Fixed16_16::from_int(66));
        assert_eq!(
            mo.z,
            Fixed16_16::from_int(64),
            "player should stay supported by the higher stair until their bbox fully clears the dropoff"
        );
    }

    #[test]
    fn tick_player_can_keep_moving_after_starting_down_stairs() {
        let mut gs = make_game_state();
        let mut level = make_partition_step_level(64, 0);
        let mo = gs
            .mobjslab
            .get_mut(gs.player.handle)
            .expect("player should exist");
        mo.flags |= flags::MF_DROPOFF;
        mo.x = Fixed16_16::from_int(96);
        mo.y = Fixed16_16::ZERO;
        mo.z = Fixed16_16::from_int(64);
        mo.momx = Fixed16_16::from_int(-40);

        tick_player(&mut gs, TicCmd::default(), Some(&mut level));

        let mo = gs
            .mobjslab
            .get_mut(gs.player.handle)
            .expect("player should exist");
        mo.momx = Fixed16_16::from_int(-4);
        mo.momy = Fixed16_16::ZERO;

        tick_player(&mut gs, TicCmd::default(), Some(&mut level));

        let mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        // Tic 1 clamps -40 -> -30 (96 -> 66); tic 2 moves -4 (66 -> 62).
        assert_eq!(
            mo.x,
            Fixed16_16::from_int(62),
            "player should continue descending instead of getting stuck on the stair edge"
        );
    }

    // -----------------------------------------------------------------------
    // MF_CORPSE step-slide friction exception (vanilla P_XYMovement)
    // -----------------------------------------------------------------------

    #[test]
    fn corpse_on_step_with_momentum_skips_friction() {
        // Regression for the DEMO3/E1M7 leveltime-409 one-tic `py` stall: a
        // shot corpse sliding while "halfway off a step" (support floor differs
        // from the center-point sector floor) keeps its momentum this tic.
        let sliding = Fixed16_16::from_int(1); // > FRACUNIT/4
        let support = Fixed16_16::from_int(64);
        let center = Fixed16_16::from_int(0);
        assert!(
            corpse_skips_friction(
                flags::MF_CORPSE,
                Fixed16_16::ZERO,
                -sliding,
                support,
                center
            ),
            "corpse straddling a step with momentum must skip friction"
        );
    }

    #[test]
    fn corpse_flat_floor_still_gets_friction() {
        // Same corpse fully over one sector (support == center) decelerates.
        let sliding = Fixed16_16::from_int(1);
        let floor = Fixed16_16::from_int(64);
        assert!(
            !corpse_skips_friction(flags::MF_CORPSE, Fixed16_16::ZERO, -sliding, floor, floor),
            "corpse on flat floor must not skip friction"
        );
    }

    #[test]
    fn corpse_below_quarter_unit_gets_friction() {
        // |mom| <= FRACUNIT/4 never triggers the exception, even on a step.
        let slow = Fixed16_16(0x4000); // exactly FRACUNIT/4 (not > FRACUNIT/4)
        let support = Fixed16_16::from_int(64);
        let center = Fixed16_16::from_int(0);
        assert!(
            !corpse_skips_friction(flags::MF_CORPSE, slow, slow, support, center),
            "sub-quarter-unit momentum must not skip friction"
        );
    }

    #[test]
    fn non_corpse_on_step_gets_friction() {
        // A live actor (no MF_CORPSE) on a step is unaffected by the clause.
        let sliding = Fixed16_16::from_int(1);
        let support = Fixed16_16::from_int(64);
        let center = Fixed16_16::from_int(0);
        assert!(
            !corpse_skips_friction(0, Fixed16_16::ZERO, -sliding, support, center),
            "non-corpse must not skip friction"
        );
    }

    fn make_walk_exit_level() -> doom_map::Level {
        let bm = make_minimal_blockmap();
        let reject = doom_map::Reject::parse_lump(&[0u8], 2).expect("item must exist in tests");
        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004,
                special: 52, // W1 exit
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            }],
            sidedefs: vec![
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"\0\0\0\0\0\0\0\0",
                    lower_texture: *b"\0\0\0\0\0\0\0\0",
                    middle_texture: *b"\0\0\0\0\0\0\0\0",
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"\0\0\0\0\0\0\0\0",
                    lower_texture: *b"\0\0\0\0\0\0\0\0",
                    middle_texture: *b"\0\0\0\0\0\0\0\0",
                    sector: 1,
                },
            ],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: -10 },
                doom_map::Vertex { x: 0, y: 10 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap: bm,
        }
    }

    fn make_damage_level(floor_height: i16, special: u16) -> doom_map::Level {
        let bm = make_minimal_blockmap();
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("item must exist in tests");
        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height,
                ceil_height: floor_height + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special,
                tag: 0,
            }],
            reject,
            blockmap: bm,
        }
    }

    #[test]
    fn tick_sets_exit_request_when_player_crosses_walk_line() {
        let mut gs = make_game_state();
        gs.mobjslab
            .get_mut(gs.player.handle)
            .expect("item must exist in tests")
            .momx = Fixed16_16::from_int(40);
        let mut level = make_walk_exit_level();

        gs.tick(TicCmd::default(), Some(&mut level));

        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Normal),
            "walk-trigger exits should be processed during the game tick, not only in the app wrapper"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: ExitRequest derive traits
    // -----------------------------------------------------------------------

    #[test]
    fn exit_request_clone_copy_partial_eq() {
        use crate::state::ExitRequest;
        let a = ExitRequest::Normal;
        let b = a; // Copy
        let c = a; // Clone
        assert_eq!(a, b, "ExitRequest must implement Copy");
        assert_eq!(a, c, "ExitRequest must implement Clone");
        assert_ne!(ExitRequest::Normal, ExitRequest::Secret, "Normal != Secret");
    }

    // =======================================================================
    // Tests: tick_world ordering
    // =======================================================================

    #[test]
    fn tick_world_processes_mobjs_before_movers() {
        // Verify that mobjs are ticked before level_time increment.
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 3);
        let handle = gs.mobjslab.alloc(trooper);

        let initial_level_time = gs.stats.level_time;
        tick_world(&mut gs, None);

        // Mobj should have been processed.
        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(
            mo.tics, 2,
            "mobj should be ticked before level_time increment"
        );
        assert_eq!(gs.stats.level_time, initial_level_time + 1);
    }

    #[test]
    fn tick_world_with_multiple_actor_types() {
        let mut gs = make_game_state();

        // Add a trooper (regular actor, far from player).
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let trooper_handle = gs.mobjslab.alloc(trooper);

        // Add a missile far from the player to avoid collision removal.
        let mut missile = Mobj::new(
            MobjKind::Rocket,
            Fixed16_16::from_int(9000),
            Fixed16_16::from_int(9000),
            Bam::ZERO,
        );
        missile.flags = flags::MF_MISSILE | flags::MF_NOGRAVITY | flags::MF_NOBLOCKMAP;
        missile.momx = Fixed16_16::from_int(3);
        missile.momy = Fixed16_16::from_int(4);
        missile.momz = Fixed16_16::ZERO;
        missile.state = StateNum(ids::S_POSS_DIE1); // 8 tics, Action::NoAction
        missile.tics = 5;
        missile.health = 1;
        let missile_handle = gs.mobjslab.alloc(missile);

        // Add a scrolling wall.
        gs.movers.scrolling_walls.push(crate::state::ScrollingWall {
            linedef_index: 0,
            speed_x: 2,
            speed_y: 0,
            accumulated_x: 0,
            accumulated_y: 0,
        });

        tick_world(&mut gs, None);

        // Trooper should have been ticked.
        let mo = gs
            .mobjslab
            .get(trooper_handle)
            .expect("trooper should exist");
        assert_eq!(mo.tics, 4);

        // Missile should have been ticked (still alive since it's far away).
        let missile_mo = gs
            .mobjslab
            .get(missile_handle)
            .expect("missile should exist");
        assert_eq!(missile_mo.tics, 4);

        // Scrolling wall should have advanced.
        assert_eq!(gs.movers.scrolling_walls[0].accumulated_x, 2);

        // Level time should have incremented.
        assert_eq!(gs.stats.level_time, 1);
    }

    // =======================================================================
    // Tests: integration — tick calls tick_world
    // =======================================================================

    #[test]
    fn tick_calls_tick_world_which_advances_actors() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let handle = gs.mobjslab.alloc(trooper);

        gs.tick(TicCmd::default(), None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(
            mo.tics, 4,
            "tick should call tick_world which advances actors"
        );
    }

    #[test]
    fn tick_processes_both_player_and_world() {
        let mut gs = make_game_state();
        gs.mobjslab
            .get_mut(gs.player.handle)
            .expect("item must exist in tests")
            .momx = Fixed16_16::from_int(2);

        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let trooper_handle = gs.mobjslab.alloc(trooper);

        gs.tick(TicCmd::default(), None);

        // Player should have moved.
        let player_mo = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player should exist");
        assert_eq!(player_mo.x, Fixed16_16::from_int(2), "player should move");

        // Trooper should have been ticked.
        let trooper_mo = gs
            .mobjslab
            .get(trooper_handle)
            .expect("trooper should exist");
        assert_eq!(trooper_mo.tics, 4, "trooper should be ticked");
    }

    #[test]
    fn tick_player_tracks_use_button_hold_state() {
        let mut gs = make_game_state();
        assert!(!gs.player.use_down);

        tick_player(
            &mut gs,
            TicCmd {
                buttons: bt::BT_USE,
                ..TicCmd::default()
            },
            None,
        );
        assert!(gs.player.use_down, "use_down must latch while use is held");

        tick_player(&mut gs, TicCmd::default(), None);
        assert!(
            !gs.player.use_down,
            "use_down must clear when the key is released"
        );
    }

    #[test]
    fn tick_mobj_removes_entity_when_current_state_is_null() {
        let mut gs = make_game_state();
        let trooper = make_trooper(crate::mobj::StateNum::NULL, 1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = super::tick_mobj(&mut gs, handle, None);
        assert!(
            matches!(result, super::TickMobjResult::Remove),
            "tick_mobj should return Remove when current state is NULL"
        );
    }

    #[test]
    fn tick_mobj_with_invalid_next_state_removes_entity() {
        let mut gs = make_game_state();
        let trooper = make_trooper(crate::mobj::StateNum(65535), 1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = super::tick_mobj(&mut gs, handle, None);
        assert!(
            matches!(result, super::TickMobjResult::Remove),
            "invalid state fallback to StateNum::NULL must return Remove"
        );
    }

    #[test]
    fn advance_mobj_state_with_invalid_state_holds_forever() {
        let mut gs = make_game_state();
        let trooper = make_trooper(crate::mobj::StateNum(65535), 1);
        let handle = gs.mobjslab.alloc(trooper);

        gs.advance_mobj_state(handle, None);

        let mo = gs.mobjslab.get(handle).expect("trooper mobj should exist");
        assert_eq!(
            mo.tics, -1,
            "advance_mobj_state falling back to StateNum::NULL should hold the state forever (-1 tics)"
        );
    }
}
