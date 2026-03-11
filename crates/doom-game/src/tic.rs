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
use doom_types::{ANG90, Bam, Fixed16_16};

use crate::mobj::{MobjHandle, StateNum, flags};
use crate::player::WeaponType;
use crate::state::GameState;

// ---------------------------------------------------------------------------
// TicCmd
// ---------------------------------------------------------------------------

/// One tic of player input — the wire-compatible command struct.
///
/// Layout is `repr(C)` with deterministic padding for netcode serialization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct TicCmd {
    /// Forward/backward movement (-128..127, positive = forward).
    pub forward_move: i8,
    /// Lateral strafe (-128..127, positive = right).
    pub side_move: i8,
    /// Angle delta in 16-bit BAM units (shifted left 16 -> 32-bit BAM).
    pub angle_turn: i16,
    /// Button bitfield (`bt::BT_*` flags).
    pub buttons: u8,
    /// ASCII chat character (0 = none).
    pub chatchar: u8,
    _pad: [u8; 2],
}

/// Button flag constants.
pub mod bt {
    /// Fire / attack.
    pub const BT_ATTACK: u8 = 0x01;
    /// Use / open / activate.
    pub const BT_USE: u8 = 0x02;
    /// Change weapon (weapon number encoded in `BT_WEAPONMASK`).
    pub const BT_CHANGE: u8 = 0x04;
    /// Bits 3-5 encode the target weapon number.
    pub const BT_WEAPONMASK: u8 = 0x38;
}

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

/// Maximum horizontal velocity per axis (15 map units).
pub const MAXMOVE: Fixed16_16 = Fixed16_16(15 << 16);

// ---------------------------------------------------------------------------
// p_set_mobj_state — canonical state transition (port of P_SetMobjState)
// ---------------------------------------------------------------------------

/// Transition an actor to a new state.
///
/// Port of Doom's `P_SetMobjState`:
/// - Sets `mobj.state = new_state`
/// - Sets `mobj.tics` from `STATES[new_state].tics`
/// - Fires the action function on state *entry* (if `action != ACTION_NONE`)
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
    // S_NULL means the actor is done — caller should remove it.
    if new_state == StateNum::NULL {
        return false;
    }

    let action = match crate::states::STATES.get(new_state.0 as usize) {
        Some(entry) => {
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.state = new_state;
                mo.tics = entry.tics;
            }
            entry.action
        }
        None => return false,
    };

    // Fire action on state entry (needs &mut self — all borrows released above).
    if action != crate::actions::ACTION_NONE {
        crate::actions::dispatch_action(gs, handle, action, level);
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
    // --- Phase 1: Momentum-based position update for missiles ---
    // In Doom's P_MobjThinker, missiles advance by momentum EVERY tic,
    // before the state machine countdown. This is separate from the full
    // projectile collision system in p_move_projectiles.
    {
        let Some(mo) = gs.mobjslab.get(handle) else {
            return TickMobjResult::Remove;
        };
        if mo.flags & flags::MF_MISSILE != 0 {
            let momx = mo.momx;
            let momy = mo.momy;
            let momz = mo.momz;
            // Release the shared borrow, take a mutable one.
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.x += momx;
                mo.y += momy;
                mo.z += momz;
            }
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
pub fn tick_all_mobjs(gs: &mut GameState, level: Option<&Level>) {
    // Collect handles first to avoid borrow conflicts during iteration.
    let handles: Vec<MobjHandle> = gs.mobjslab.iter_handles().collect();
    let player_handle = gs.player.handle;
    let is_nightmare = gs.skill == crate::spawn::Skill::Nightmare;

    for handle in handles {
        // Skip the player mobj — it's handled by tick_player.
        if handle == player_handle {
            continue;
        }

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
/// 10. `p_move_projectiles` — projectile movement and collision
/// 11. Increment `gs.level_time`
///
/// This does NOT process player input — call `tick_player` before this.
pub fn tick_world(gs: &mut GameState, mut level: Option<&mut Level>) {
    // 1. Advance all actor state machines.
    tick_all_mobjs(gs, level.as_deref());

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

    // 8. Scrolling walls (no level mutation needed).
    crate::specials::tick_scrollers(gs);

    // 9. Conveyor belts: push actors standing in conveyor sectors.
    crate::specials::tick_conveyors(gs, level.as_deref());

    // 10. Move projectiles: advance missile actors and check collisions.
    crate::projectile::p_move_projectiles(gs, level.as_deref());

    // 11. Increment level time.
    gs.level_time = gs.level_time.wrapping_add(1);
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
    // Movement + attack (immutable level borrow).
    p_move_player(gs, cmd, level.as_deref());

    // Pickup check: scan MF_SPECIAL actors.
    if !gs.player.is_dead() {
        crate::pickups::p_check_pickups(gs);
    }

    // BT_ATTACK: fire current weapon.
    // Auto-fire weapons (chaingun, plasma) fire every tic the button is held.
    // All others are edge-triggered: fire only on the leading edge of the press.
    let attack_held = cmd.buttons & bt::BT_ATTACK != 0;
    let is_auto_weapon = matches!(
        gs.player.weapon,
        WeaponType::Chaingun | WeaponType::PlasmaRifle | WeaponType::Chainsaw
    );
    if attack_held && (!gs.player.attack_down || is_auto_weapon) {
        crate::weapon_fire::fire_current_weapon(gs, level.as_deref());
    }
    gs.player.attack_down = attack_held;

    // BT_USE: activate linedef ahead of player on the leading edge only.
    let use_held = cmd.buttons & bt::BT_USE != 0;
    if use_held && !gs.player.use_down {
        if let Some(lv) = level.as_deref_mut() {
            let handle = gs.player.handle;
            crate::specials::p_use_lines(gs, lv, handle);
        }
    }
    gs.player.use_down = use_held;

    // BT_CHANGE: weapon switch.
    if cmd.buttons & bt::BT_CHANGE != 0 {
        let weapon_num = ((cmd.buttons & bt::BT_WEAPONMASK) >> 3) as usize;
        if let Some(weapon) = WeaponType::from_num(weapon_num) {
            if gs.player.weapons[weapon as usize] {
                gs.player.weapon = weapon;
            }
        }
    }

    // Sector specials: damage floors, etc. (immutable level borrow).
    if let Some(lv) = level.as_deref() {
        let handle = gs.player.handle;
        crate::specials::tick_sector_specials(gs, lv, handle);
        // Periodic sector damage with RadSuit protection.
        crate::specials::tick_sector_damage(gs, lv);
    }

    // Secret sector detection (special type 9): when the player is standing
    // on a secret sector, increment their secret_count and clear the sector
    // special so it only counts once.
    if !gs.player.is_dead() {
        if let Some(ref mut lv) = level {
            let player_z = gs.mobjslab.get(gs.player.handle).map(|mo| mo.z.to_int());
            if let Some(pz) = player_z {
                for sector in &mut lv.sectors {
                    if sector.special == 9 && pz == sector.floor_height as i32 {
                        gs.player.secret_count += 1;
                        sector.special = 0;
                    }
                }
            }
        }
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
        self.sound_queue.clear();

        // Player input processing.
        tick_player(self, cmd, level.as_deref_mut());

        // World simulation: mobjs, movers, projectiles, level_time.
        tick_world(self, level);
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
        // Collect handles first to avoid borrow conflicts during iteration.
        let handles: Vec<MobjHandle> = self.mobjslab.iter_handles().collect();

        for handle in handles {
            self.advance_mobj_state(handle, level);
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
fn p_move_player(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    let handle = gs.player.handle;

    // Thrust block: apply turn + acceleration, then release borrow.
    {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };

        // 1. Turn: cmd.angle_turn is 16-bit BAM; shift to 32-bit BAM space.
        mo.angle = mo.angle + Bam((cmd.angle_turn as i32 as u32).wrapping_shl(16));

        // 2. Forward / backward thrust.
        if cmd.forward_move != 0 {
            let angle = mo.angle;
            p_thrust(mo, angle, cmd.forward_move);
        }

        // 3. Strafe: thrust perpendicular (90 degrees left of facing direction).
        if cmd.side_move != 0 {
            let strafe_angle = mo.angle - ANG90;
            p_thrust(mo, strafe_angle, cmd.side_move);
        }
    }

    // 4. Compute proposed position, then collision-test (needs shared borrow).
    let (old_x, old_y, new_x, new_y) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x, mo.y, mo.x + mo.momx, mo.y + mo.momy),
        None => return,
    };
    let mut moved = match level {
        Some(lv) => crate::movement::p_try_move(&gs.mobjslab, handle, new_x, new_y, lv),
        None => true,
    };
    let mut final_x = new_x;
    let mut final_y = new_y;

    if !moved {
        if let Some(lv) = level {
            let (sx, sy) =
                crate::movement::p_slide_move(&gs.mobjslab, handle, old_x, old_y, new_x, new_y, lv);
            if sx != old_x || sy != old_y {
                moved = true;
                final_x = sx;
                final_y = sy;
            }
        }
    }

    // 5. Apply position + friction + clamp.
    let Some(mo) = gs.mobjslab.get_mut(handle) else {
        return;
    };
    if moved {
        mo.x = final_x;
        mo.y = final_y;
        mo.momx = final_x - old_x;
        mo.momy = final_y - old_y;
    }
    mo.momx = mo.momx.fixed_mul(FRICTION);
    mo.momy = mo.momy.fixed_mul(FRICTION);
    mo.momx = mo.momx.clamp(-MAXMOVE, MAXMOVE);
    mo.momy = mo.momy.clamp(-MAXMOVE, MAXMOVE);

    // 6. Update floor height (mo.z) to track the sector the player is now in.
    // This is critical for stair climbing: the step-height check in p_try_move
    // compares `open_floor - mo_z` against MAX_STEP_HEIGHT (24 units).
    // Without this update, mo.z stays at the spawn-point floor and multi-step
    // stairs become impassable after the first step.
    if let Some(lv) = level {
        let fx = mo.x.to_int();
        let fy = mo.y.to_int();
        if let Some(floor_h) = lv.floor_at(fx, fy) {
            mo.z = Fixed16_16::from_int(floor_h as i32);
        }
        if let Some(subsector) = lv.subsector_index_at(fx, fy) {
            mo.subsector = subsector as u32;
        }
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind, flags};
    use crate::player::PlayerState;
    use crate::states::ids;
    use doom_types::{Bam, Fixed16_16};

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
        gs
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
    // Tests: p_set_mobj_state
    // =======================================================================

    #[test]
    fn p_set_mobj_state_sets_state_and_tics() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum::NULL, -1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = p_set_mobj_state(&mut gs, handle, StateNum(ids::S_POSS_STND), None);
        assert!(result, "p_set_mobj_state must return true for valid state");

        let mo = gs.mobjslab.get(handle).unwrap();
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

        // S_POSS_DIE1 has ACTION_NONE — should just set state.
        let result = p_set_mobj_state(&mut gs, handle, StateNum(ids::S_POSS_DIE1), None);
        assert!(result);

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.state, StateNum(ids::S_POSS_DIE1));
        assert_eq!(mo.tics, 8, "POSS_DIE1 has 8 tics");
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

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.tics, 4, "tics should decrement from 5 to 4");
    }

    #[test]
    fn tick_mobj_transitions_at_zero() {
        let mut gs = make_game_state();
        // S_POSS_STND loops to itself with tics=10 and ACTION_LOOK.
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).unwrap();
        // S_POSS_STND next_state = S_POSS_STND (loops), tics = 10.
        assert_eq!(mo.state, StateNum(ids::S_POSS_STND));
        assert_eq!(mo.tics, 10, "should have reloaded tics from STATES table");
    }

    #[test]
    fn tick_mobj_holds_negative_tics() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum(ids::S_POSS_DIE2), -1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).unwrap();
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
        // S_POSS_DIE1: 8 tics, next = S_POSS_DIE2
        let trooper = make_trooper(StateNum(ids::S_POSS_DIE1), 1);
        let handle = gs.mobjslab.alloc(trooper);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.state, StateNum(ids::S_POSS_DIE2));
        assert_eq!(mo.tics, 8, "DIE2 runs for 8 tics before DIE3");
    }

    #[test]
    fn tick_mobj_missile_updates_position_every_tic() {
        let mut gs = make_game_state();
        let missile = make_missile(5, 3, 1);
        let handle = gs.mobjslab.alloc(missile);

        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).unwrap();
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

        let mo = gs.mobjslab.get(handle).unwrap();
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

        let mo1 = gs.mobjslab.get(h1).unwrap();
        assert_eq!(mo1.tics, 4, "first trooper tics should decrement");
        let mo2 = gs.mobjslab.get(h2).unwrap();
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

        let mo = gs.mobjslab.get(player_handle).unwrap();
        assert_eq!(
            mo.tics, 5,
            "player mobj should not be ticked by tick_all_mobjs"
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
        assert_eq!(gs.level_time, 0);
        tick_world(&mut gs, None);
        assert_eq!(gs.level_time, 1);
        tick_world(&mut gs, None);
        assert_eq!(gs.level_time, 2);
    }

    #[test]
    fn tick_world_advances_actor_states() {
        let mut gs = make_game_state();
        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let handle = gs.mobjslab.alloc(trooper);

        tick_world(&mut gs, None);

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.tics, 4, "tick_world should advance actor state machines");
    }

    #[test]
    fn tick_world_processes_scrolling_walls() {
        let mut gs = make_game_state();
        gs.scrolling_walls.push(crate::state::ScrollingWall {
            linedef_index: 0,
            speed_x: 1,
            speed_y: 0,
            accumulated_x: 0,
            accumulated_y: 0,
        });

        tick_world(&mut gs, None);

        assert_eq!(
            gs.scrolling_walls[0].accumulated_x, 1,
            "tick_world must advance scrolling walls"
        );
    }

    #[test]
    fn tick_world_level_time_wraps() {
        let mut gs = make_game_state();
        gs.level_time = u32::MAX;
        tick_world(&mut gs, None);
        assert_eq!(gs.level_time, 0, "level_time must wrap at u32::MAX");
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

        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
        let expected = Bam((640i32 as u32).wrapping_shl(16));
        assert_eq!(mo.angle, expected);
    }

    #[test]
    fn tick_player_applies_friction() {
        let mut gs = make_game_state();
        gs.mobjslab.get_mut(gs.player.handle).unwrap().momx = Fixed16_16::from_int(4);

        tick_player(&mut gs, TicCmd::default(), None);

        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
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
    fn tick_player_pistol_can_hit_slightly_off_axis_target() {
        // SAFETY: trig tables are process-global and internally guarded.
        unsafe {
            doom_types::Bam::init_trig_tables();
        }

        let mut gs = make_game_state();
        let mut trooper = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(512),
            Fixed16_16::from_int(-21),
            Bam::ZERO,
        );
        trooper.health = 20;
        trooper.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        let trooper_handle = gs.mobjslab.alloc(trooper);

        tick_player(
            &mut gs,
            TicCmd {
                buttons: bt::BT_ATTACK,
                ..Default::default()
            },
            None,
        );

        assert!(
            gs.mobjslab.get(trooper_handle).unwrap().health < 20,
            "player attack should use Doom-style bullet spread, not a zero-spread laser"
        );
    }

    #[test]
    fn tick_player_weapon_change() {
        use crate::player::WeaponType;
        let mut gs = make_game_state();
        gs.player.weapons[WeaponType::Shotgun as usize] = true;

        let cmd = TicCmd {
            buttons: bt::BT_CHANGE | (2u8 << 3),
            ..Default::default()
        };
        tick_player(&mut gs, cmd, None);

        assert_eq!(gs.player.weapon, WeaponType::Shotgun);
    }

    #[test]
    fn tick_player_no_change_unowned_weapon() {
        use crate::player::WeaponType;
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
    fn state_transition_run1_to_run2() {
        let mut gs = make_game_state();
        // S_POSS_RUN1: 4 tics, next = S_POSS_RUN2, action = A_Chase.
        // A_Chase needs a valid alive target or it reverts to idle.
        let trooper = make_trooper(StateNum(ids::S_POSS_RUN1), 1);
        let handle = gs.mobjslab.alloc(trooper);
        // Set the player as the trooper's target so A_Chase doesn't revert.
        gs.mobjslab.get_mut(handle).unwrap().target = gs.player.handle;

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.state, StateNum(ids::S_POSS_RUN2));
        assert_eq!(mo.tics, 4);
    }

    #[test]
    fn state_transition_run2_back_to_run1() {
        let mut gs = make_game_state();
        // S_POSS_RUN2: 4 tics, next = S_POSS_RUN1, action = A_Chase.
        let trooper = make_trooper(StateNum(ids::S_POSS_RUN2), 1);
        let handle = gs.mobjslab.alloc(trooper);
        gs.mobjslab.get_mut(handle).unwrap().target = gs.player.handle;

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.state, StateNum(ids::S_POSS_RUN1));
        assert_eq!(mo.tics, 4);
    }

    #[test]
    fn state_transition_pain_to_idle() {
        let mut gs = make_game_state();
        // S_POSS_PAIN: 6 tics, next = S_POSS_STND
        let trooper = make_trooper(StateNum(ids::S_POSS_PAIN), 1);
        let handle = gs.mobjslab.alloc(trooper);

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.state, StateNum(ids::S_POSS_STND));
        assert_eq!(mo.tics, 10);
    }

    #[test]
    fn state_transition_atk3_to_run1() {
        let mut gs = make_game_state();
        // S_POSS_ATK3: 4 tics, next = S_POSS_RUN1, action = ACTION_NONE.
        // But S_POSS_RUN1 entry fires A_Chase, which needs a target.
        let trooper = make_trooper(StateNum(ids::S_POSS_ATK3), 1);
        let handle = gs.mobjslab.alloc(trooper);
        gs.mobjslab.get_mut(handle).unwrap().target = gs.player.handle;

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).unwrap();
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
        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(
            mo.state,
            StateNum(ids::S_POSS_DIE1),
            "should still be in DIE1"
        );
        assert_eq!(mo.tics, 1, "one tic left");

        // Final tick transitions to DIE2.
        let result = tick_mobj(&mut gs, handle, None);
        assert!(matches!(result, TickMobjResult::Alive));

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.state, StateNum(ids::S_POSS_DIE2));
        assert_eq!(mo.tics, 8, "DIE2 runs for 8 tics before chaining to DIE3");
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
        assert_eq!(gs.mobjslab.get(alive_handle).unwrap().tics, 4);
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
        gs.mobjslab.get_mut(handle).unwrap().tics = 1;

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.x, Fixed16_16::from_int(10));
        assert_eq!(mo.y, Fixed16_16::from_int(-5));
        assert_eq!(mo.z, Fixed16_16::from_int(2));
    }

    #[test]
    fn non_missile_does_not_advance_position() {
        let mut gs = make_game_state();
        let mut trooper = make_trooper(StateNum(ids::S_POSS_STND), 1);
        trooper.momx = Fixed16_16::from_int(5);
        trooper.momy = Fixed16_16::from_int(3);
        let handle = gs.mobjslab.alloc(trooper);

        tick_mobj(&mut gs, handle, None);

        let mo = gs.mobjslab.get(handle).unwrap();
        // Non-missile actors don't get position updates from tick_mobj.
        assert_eq!(
            mo.x,
            Fixed16_16::from_int(5000),
            "non-missile x should not change"
        );
        assert_eq!(
            mo.y,
            Fixed16_16::from_int(5000),
            "non-missile y should not change"
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
        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
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
        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
        // angle_turn = 640 << 16 in 32-bit BAM.
        let expected = Bam((640i32 as u32).wrapping_shl(16));
        assert_eq!(mo.angle, expected);
    }

    #[test]
    fn friction_drains_existing_momentum() {
        let mut gs = make_game_state();
        // Inject momentum directly (bypassing thrust, since trig tables uninitialized).
        gs.mobjslab.get_mut(gs.player.handle).unwrap().momx = Fixed16_16::from_int(4);

        gs.tick(TicCmd::default(), None);

        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
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
        gs.mobjslab.get_mut(gs.player.handle).unwrap().momx = Fixed16_16::from_int(2);

        gs.tick(TicCmd::default(), None);

        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
        // x started at 0, momx = 2 -> x = 2 after one tick.
        assert_eq!(mo.x, Fixed16_16::from_int(2));
    }

    #[test]
    fn maxmove_clamps_excessive_velocity() {
        let mut gs = make_game_state();
        // Inject extreme velocity.
        gs.mobjslab.get_mut(gs.player.handle).unwrap().momx = Fixed16_16::from_int(1000);

        gs.tick(TicCmd::default(), None);

        let mo = gs.mobjslab.get(gs.player.handle).unwrap();
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
        use crate::player::WeaponType;
        let mut gs = make_game_state();
        // Give player the shotgun.
        gs.player.weapons[WeaponType::Shotgun as usize] = true;
        // BT_CHANGE | (weapon_num=2 << 3) = 0x04 | 0x10 = 0x14
        let cmd = TicCmd {
            buttons: bt::BT_CHANGE | (2u8 << 3),
            ..Default::default()
        };
        gs.tick(cmd, None);
        assert_eq!(
            gs.player.weapon,
            WeaponType::Shotgun,
            "Should switch to Shotgun when owned"
        );
    }

    #[test]
    fn bt_change_ignores_unowned_weapon() {
        use crate::player::WeaponType;
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
        assert_eq!(gs.level_time, 0, "level_time starts at 0");
        gs.tick(TicCmd::default(), None);
        assert_eq!(gs.level_time, 1, "level_time must be 1 after first tick");
        gs.tick(TicCmd::default(), None);
        assert_eq!(gs.level_time, 2, "level_time must be 2 after second tick");
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
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).unwrap();
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
        doom_map::Blockmap::parse_lump(&bm_data).unwrap()
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

    // -----------------------------------------------------------------------
    // Tests: ExitRequest derive traits
    // -----------------------------------------------------------------------

    #[test]
    fn exit_request_clone_copy_partial_eq() {
        use crate::state::ExitRequest;
        let a = ExitRequest::Normal;
        let b = a; // Copy
        let c = a.clone(); // Clone
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

        let initial_level_time = gs.level_time;
        tick_world(&mut gs, None);

        // Mobj should have been processed.
        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(
            mo.tics, 2,
            "mobj should be ticked before level_time increment"
        );
        assert_eq!(gs.level_time, initial_level_time + 1);
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
        missile.state = StateNum(ids::S_POSS_DIE1); // 8 tics, ACTION_NONE
        missile.tics = 5;
        missile.health = 1;
        let missile_handle = gs.mobjslab.alloc(missile);

        // Add a scrolling wall.
        gs.scrolling_walls.push(crate::state::ScrollingWall {
            linedef_index: 0,
            speed_x: 2,
            speed_y: 0,
            accumulated_x: 0,
            accumulated_y: 0,
        });

        tick_world(&mut gs, None);

        // Trooper should have been ticked.
        let mo = gs.mobjslab.get(trooper_handle).unwrap();
        assert_eq!(mo.tics, 4);

        // Missile should have been ticked (still alive since it's far away).
        let missile_mo = gs.mobjslab.get(missile_handle).unwrap();
        assert_eq!(missile_mo.tics, 4);

        // Scrolling wall should have advanced.
        assert_eq!(gs.scrolling_walls[0].accumulated_x, 2);

        // Level time should have incremented.
        assert_eq!(gs.level_time, 1);
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

        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(
            mo.tics, 4,
            "tick should call tick_world which advances actors"
        );
    }

    #[test]
    fn tick_processes_both_player_and_world() {
        let mut gs = make_game_state();
        gs.mobjslab.get_mut(gs.player.handle).unwrap().momx = Fixed16_16::from_int(2);

        let trooper = make_trooper(StateNum(ids::S_POSS_STND), 5);
        let trooper_handle = gs.mobjslab.alloc(trooper);

        gs.tick(TicCmd::default(), None);

        // Player should have moved.
        let player_mo = gs.mobjslab.get(gs.player.handle).unwrap();
        assert_eq!(player_mo.x, Fixed16_16::from_int(2), "player should move");

        // Trooper should have been ticked.
        let trooper_mo = gs.mobjslab.get(trooper_handle).unwrap();
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
}
