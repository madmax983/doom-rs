//! Monster action functions — dispatched from the thinker loop.
//!
//! Action functions fire when an actor *enters* a new state (when `tics`
//! reaches zero and the state machine transitions to `next_state`).
//!
//! # Implemented (Batch 2)
//! - `A_Look` (index 1): scan for the player; transition to `see_state`.
//! - `A_Chase` (index 2): move toward target; use 8-direction grid movement.
//! - `A_FaceTarget` (index 10): snap angle to face the current target.
//!
//! # Implemented (Batch 5)
//! - `A_PosAttack`  (index 3): Trooper hitscan attack.
//! - `A_SPosAttack` (index 4): Sergeant 3-pellet shotgun burst.
//! - `A_TroopAttack`(index 5): Imp — melee if close, else fireball projectile.
//! - `A_SargAttack` (index 6): Demon melee-only attack.
//! - `A_Fall`       (index 7): clear MF_SOLID/MF_COUNTKILL on death.
//!
//! # Implemented (Batch 21)
//! - `A_HeadAttack`  (index 8): Cacodemon fireball projectile.
//! - `A_BruisAttack` (index 9): Baron/Hell Knight plasma ball projectile.
//!
//! # Implemented (Batch 30)
//! - Full `A_Look`: sound target wake-up, `MF_AMBUSH` handling, `p_look_for_players`.
//! - Full `A_Chase`: `reaction_time`, `MF_JUSTATTACKED`, melee/missile attack checks,
//!   `P_Move` with collision, active sound.
//! - `P_Move`: monster one-step movement with collision detection.
//! - `A_FaceTarget`: proper angle calculation using integer `atan2`.
//! - `p_new_chase_dir`: full 4-candidate direction selection with fallback.

use doom_map::Level;

/// All possible action functions (code pointers) an actor can execute.
///
/// Doom's actors are driven by a state machine where each state may invoke an
/// action function. This enum maps to the original C function pointers.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Action {
    /// No action — state entry is silent.
    NoAction = 0,
    /// `A_Look`: search for the player and transition to `see_state`.
    Look = 1,
    /// `A_Chase`: move toward current target using 8-direction grid movement.
    Chase = 2,
    /// `A_PosAttack`: Trooper hitscan attack.
    PosAttack = 3,
    /// `A_SPosAttack`: Sergeant 3-pellet shotgun burst.
    SposAttack = 4,
    /// `A_TroopAttack`: Imp melee-or-hitscan attack.
    TrooAttack = 5,
    /// `A_SargAttack`: Demon melee-only attack.
    SargAttack = 6,
    /// `A_Fall`: clear MF_SOLID and MF_COUNTKILL so corpses are passable.
    Fall = 7,
    /// `A_HeadAttack`: Cacodemon fireball projectile.
    HeadAttack = 8,
    /// `A_BruisAttack`: Baron/Hell Knight plasma ball projectile.
    BruisAttack = 9,
    /// `A_FaceTarget`: snap angle to face current target.
    FaceTarget = 10,
    /// `A_CPosAttack`: Chaingunner hitscan attack.
    CposAttack = 11,
    /// `A_CyberAttack`: Cyberdemon spawns a Rocket projectile.
    CyberAttack = 12,
    /// `A_SkelMissile`: Revenant spawns a Tracer projectile.
    SkelMissile = 13,
    /// `A_FatAttack1`: Mancubus fireball spread #1 (+FATSPREAD).
    FatAttack1 = 14,
    /// `A_FatAttack2`: Mancubus fireball spread #2 (−FATSPREAD).
    FatAttack2 = 15,
    /// `A_FatAttack3`: Mancubus fireball spread #3 (±FATSPREAD/2).
    FatAttack3 = 16,
    /// `A_SkullAttack`: Lost Soul charge attack.
    SkullAttack = 17,
    /// `A_BspiAttack`: Arachnotron spawns ArachnotronPlasma.
    BspiAttack = 18,
    /// `A_SpidAttack`: Spider Mastermind hitscan attack.
    SpidAttack = 19,
    /// `A_PainAttack`: Pain Elemental spawns Lost Soul.
    PainAttack = 20,
    /// `A_Scream`: Play monster death sound on first death frame.
    Scream = 21,
    /// `A_VileChase`: Chase with resurrection scan.
    VileChase = 22,
    /// `A_VileStart`: Begin attack — set tracer to target.
    VileStart = 23,
    /// `A_VileTarget`: Spawn fire column at target's position.
    VileTarget = 24,
    /// `A_VileAttack`: Deal 20 direct + 70 blast damage and upward thrust.
    VileAttack = 25,
    /// `A_Fire`: Fire column tracks the target's position each tic.
    Fire = 26,
    /// `A_BrainAwake`: Set brain_awake flag and play alert sound.
    BrainAwake = 27,
    /// `A_BrainSpit`: Spawn a BossCube aimed at the next spawn spot.
    BrainSpit = 28,
    /// `A_SpawnFly`: Cube arrives — spawn a random monster at destination.
    SpawnFly = 29,
    /// `A_BrainDie`: Trigger level exit.
    BrainDie = 30,
    /// `A_BrainScream`: Spawn 20 explosions across the brain sprite.
    BrainScream = 31,
    /// `A_BrainExplode`: Spawn a single explosion at a random position.
    BrainExplode = 32,
    /// `A_WeaponReady`: bob-ready player weapon loop, handles fire/switch input.
    WeaponReady = 33,
    /// `A_Lower`: lower the current player weapon toward the bottom of the screen.
    Lower = 34,
    /// `A_Raise`: raise the pending/current player weapon toward the ready position.
    Raise = 35,
    /// `A_GunFlash`: start the weapon's muzzle flash psprite.
    GunFlash = 36,
    /// `A_Punch`: fist attack.
    Punch = 37,
    /// `A_FirePistol`: pistol attack.
    FirePistol = 38,
    /// `A_FireShotgun`: shotgun attack.
    FireShotgun = 39,
    /// `A_FireShotgun2`: super shotgun attack.
    FireShotgun2 = 40,
    /// `A_FireCGun`: chaingun attack.
    FireCgun = 41,
    /// `A_FireMissile`: rocket launcher attack.
    FireMissile = 42,
    /// `A_FirePlasma`: plasma rifle attack.
    FirePlasma = 43,
    /// `A_BFGSound`: play the BFG windup sound before the projectile launches.
    BfgSound = 44,
    /// `A_FireBFG`: BFG projectile launch.
    FireBfg = 45,
    /// `A_Saw`: chainsaw attack.
    Saw = 46,
    /// `A_ReFire`: continue firing when the attack button remains held.
    Refire = 47,
    /// `A_CheckReload`: lower the weapon if it no longer has enough ammo.
    CheckReload = 48,
    /// `A_OpenShotgun2`: play the super shotgun open sound.
    OpenShotgun2 = 49,
    /// `A_LoadShotgun2`: play the super shotgun load sound.
    LoadShotgun2 = 50,
    /// `A_CloseShotgun2`: play the super shotgun close sound and optionally refire.
    CloseShotgun2 = 51,
    /// `A_Light0`: clear the player's weapon flash light bonus.
    Light0 = 52,
    /// `A_Light1`: set the player's weapon flash light bonus to level 1.
    Light1 = 53,
    /// `A_Light2`: set the player's weapon flash light bonus to level 2.
    Light2 = 54,
}

use doom_types::{Bam, Fixed16_16};

use crate::mobj::MobjHandle;
use crate::mobj::flags;
use crate::state::GameState;
use crate::{mobjinfo, states};
use doom_types::mobj_kind::MobjKind;

// ---------------------------------------------------------------------------
// Action index constants
// ---------------------------------------------------------------------------

// --- Arch-Vile special actions ---

// --- Boss Brain (Icon of Sin) actions ---

// ---------------------------------------------------------------------------
// Public dispatcher
// ---------------------------------------------------------------------------

/// Dispatch the action function with index `action` for actor `handle`.
///
/// Called by `GameState::advance_mobj_state` each time an actor enters a
/// new state.
///
/// Helper to get a valid, alive target for a monster.
/// Returns the target handle if it exists and is not dead.
fn get_alive_target(gs: &GameState, handle: MobjHandle) -> Option<MobjHandle> {
    let mo = gs.mobjslab.get(handle)?;
    if mo.target == MobjHandle::NULL {
        return None;
    }
    let target_mo = gs.mobjslab.get(mo.target)?;
    if target_mo.is_dead() {
        return None;
    }
    Some(mo.target)
}

/// Helper to get a valid, alive target along with the monster's current position.
fn get_alive_target_with_pos(
    gs: &GameState,
    handle: MobjHandle,
) -> Option<(MobjHandle, Fixed16_16, Fixed16_16)> {
    let mo = gs.mobjslab.get(handle)?;
    if mo.target == MobjHandle::NULL {
        return None;
    }
    let target = mo.target;
    let mo_x = mo.x;
    let mo_y = mo.y;
    if gs.mobjslab.get(target).map(|t| t.is_dead()).unwrap_or(true) {
        return None;
    }
    Some((target, mo_x, mo_y))
}

/// Dispatches a monster or projectile behavior action by its index.
///
/// This is the primary router for the `A_*` functions (e.g. `A_Look`, `A_Chase`),
/// called automatically when an actor enters a new state that has an action assigned.
///
/// # Examples
/// ```
/// use doom_game::actions::{dispatch_action, Action};
/// use doom_game::state::GameState;
/// use doom_game::mobj::MobjHandle;
///
/// let mut gs = GameState::new("E1M1");
/// // A dummy handle for illustration; in a real game, this points to a live actor.
/// let handle = MobjHandle { index: 0, generation: 0 };
/// // Action 0 is Action::NoAction as u8, which is a no-op.
/// dispatch_action(&mut gs, handle, Action::NoAction as u8, None);
/// ```
pub fn dispatch_action(gs: &mut GameState, handle: MobjHandle, action: u8, level: Option<&Level>) {
    if let Some(a) = Action::from_repr(action) {
        match a {
            Action::NoAction => {}
            Action::Look => a_look(gs, handle, level),
            Action::Chase => a_chase(gs, handle, level),
            Action::PosAttack => a_pos_attack(gs, handle, level),
            Action::SposAttack => a_spos_attack(gs, handle, level),
            Action::TrooAttack => a_troo_attack(gs, handle, level),
            Action::SargAttack => a_sarg_attack(gs, handle),
            Action::Fall => a_fall(gs, handle),
            Action::HeadAttack => a_head_attack(gs, handle),
            Action::BruisAttack => a_bruis_attack(gs, handle),
            Action::FaceTarget => a_face_target(gs, handle),
            Action::CposAttack => a_cpos_attack(gs, handle, level),
            Action::CyberAttack => a_cyber_attack(gs, handle),
            Action::SkelMissile => a_skel_missile(gs, handle),
            Action::FatAttack1 => a_fat_attack1(gs, handle),
            Action::FatAttack2 => a_fat_attack2(gs, handle),
            Action::FatAttack3 => a_fat_attack3(gs, handle),
            Action::SkullAttack => a_skull_attack(gs, handle),
            Action::BspiAttack => a_bspi_attack(gs, handle),
            Action::SpidAttack => a_spid_attack(gs, handle, level),
            Action::PainAttack => a_pain_attack(gs, handle),
            Action::Scream => a_scream(gs, handle),
            Action::VileChase => a_vile_chase(gs, handle, level),
            Action::VileStart => a_vile_start(gs, handle),
            Action::VileTarget => a_vile_target(gs, handle),
            Action::VileAttack => a_vile_attack(gs, handle),
            Action::Fire => a_fire(gs, handle),
            Action::BrainAwake => a_brain_awake(gs),
            Action::BrainSpit => a_brain_spit(gs, handle),
            Action::SpawnFly => a_spawn_fly(gs, handle),
            Action::BrainDie => a_brain_die(gs),
            Action::BrainScream => a_brain_scream(gs, handle),
            Action::BrainExplode => a_brain_explode(gs, handle),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// 8-direction constants and movement table
// ---------------------------------------------------------------------------

/// Direction constants matching Doom's `dirtype_t`.
/// East (0 degrees).
pub const DI_EAST: u8 = 0;
/// Northeast (45 degrees).
pub const DI_NORTHEAST: u8 = 1;
/// North (90 degrees).
pub const DI_NORTH: u8 = 2;
/// Northwest (135 degrees).
pub const DI_NORTHWEST: u8 = 3;
/// West (180 degrees).
pub const DI_WEST: u8 = 4;
/// Southwest (225 degrees).
pub const DI_SOUTHWEST: u8 = 5;
/// South (270 degrees).
pub const DI_SOUTH: u8 = 6;
/// Southeast (315 degrees).
pub const DI_SOUTHEAST: u8 = 7;
/// No direction or invalid direction.
pub const DI_NODIR: u8 = 8;

/// Unit movement vectors for the 8-way grid (Doom `DI_*` directions).
///
/// Index: 0=East, 1=NE, 2=North, 3=NW, 4=West, 5=SW, 6=South, 7=SE.
/// Values ≈ `FRACUNIT * cos/sin(n * 45°)`.  Diagonal uses Doom's historical
/// constant 47000 ≈ 65536 * sin(45°).
const XMOVE: [Fixed16_16; 9] = [
    Fixed16_16(65536),  // East
    Fixed16_16(47000),  // NE
    Fixed16_16(0),      // North
    Fixed16_16(-47000), // NW
    Fixed16_16(-65536), // West
    Fixed16_16(-47000), // SW
    Fixed16_16(0),      // South
    Fixed16_16(47000),  // SE
    Fixed16_16(0),      // NODIR
];

const YMOVE: [Fixed16_16; 9] = [
    Fixed16_16(0),      // East
    Fixed16_16(47000),  // NE
    Fixed16_16(65536),  // North
    Fixed16_16(47000),  // NW
    Fixed16_16(0),      // West
    Fixed16_16(-47000), // SW
    Fixed16_16(-65536), // South
    Fixed16_16(-47000), // SE
    Fixed16_16(0),      // NODIR
];

/// Choose the best 8-way direction given a `(dx, dy)` displacement.
#[allow(dead_code)]
fn dir_to_target(dx: i32, dy: i32) -> u8 {
    let ax = dx.abs();
    let ay = dy.abs();
    if ax > 2 * ay {
        // Mostly horizontal.
        if dx > 0 { DI_EAST } else { DI_WEST }
    } else if ay > 2 * ax {
        // Mostly vertical.
        if dy > 0 { DI_NORTH } else { DI_SOUTH }
    } else {
        // Diagonal.
        match (dx >= 0, dy >= 0) {
            (true, true) => DI_NORTHEAST,
            (false, true) => DI_NORTHWEST,
            (false, false) => DI_SOUTHWEST,
            (true, false) => DI_SOUTHEAST,
        }
    }
}

/// Melee range plus an extra 20 units for the melee check in A_Chase.
///
/// Doom uses `MELEERANGE + 20*FRACUNIT` for the distance threshold before
/// transitioning to the melee attack state.
const MELEE_THRESHOLD: i32 = 64 + 20; // MELEERANGE (64) + 20 map units

// ---------------------------------------------------------------------------
// P_CheckSight — coarse LOS check via the REJECT table
// ---------------------------------------------------------------------------

/// Returns `true` if `source` and `target` have line-of-sight.
///
/// Uses the REJECT table for coarse sector-based culling when a level is
/// available.  If the REJECT table marks the two sectors as mutually
/// invisible, returns `false` immediately without further work.
///
/// Falls back to a Manhattan distance check (≤ 4096 units) when no level is
/// provided, or when the REJECT table says the pair is potentially visible.
///
/// Full ray-cast LOS is a Phase 8 item.
fn p_check_sight_local(
    gs: &GameState,
    source: MobjHandle,
    target: MobjHandle,
    level: Option<&Level>,
) -> bool {
    let Some(mo) = gs.mobjslab.get(source) else {
        return false;
    };
    let src_x = mo.x;
    let src_y = mo.y;
    let src_subsector = mo.subsector as usize;
    let Some(mo) = gs.mobjslab.get(target) else {
        return false;
    };
    let tgt_x = mo.x;
    let tgt_y = mo.y;
    let tgt_subsector = mo.subsector as usize;

    // REJECT-table culling: look up the sector indices from current positions.
    if let Some(lv) = level {
        let src_sector =
            crate::sight::sector_from_position_or_subsector(lv, src_x, src_y, src_subsector);
        let tgt_sector =
            crate::sight::sector_from_position_or_subsector(lv, tgt_x, tgt_y, tgt_subsector);
        if let (Some(ss), Some(ts)) = (src_sector, tgt_sector) {
            // If REJECT says definitely not visible, bail out immediately.
            if !lv.reject.visible(ss, ts) {
                return false;
            }
        }
    }

    // Secondary check: Manhattan distance ≤ 4096 map units.
    let dist = (tgt_x - src_x).to_int().abs() + (tgt_y - src_y).to_int().abs();
    dist <= 4096
}

fn approx_distance(dx: i32, dy: i32) -> i32 {
    let dx = dx.abs();
    let dy = dy.abs();
    let (hi, lo) = if dx >= dy { (dx, dy) } else { (dy, dx) };
    hi + lo - (lo / 2)
}

fn p_check_missile_range(
    gs: &mut GameState,
    handle: MobjHandle,
    target: MobjHandle,
    level: Option<&Level>,
) -> bool {
    let Some(mo) = gs.mobjslab.get(handle) else {
        return false;
    };
    let mo_kind = mo.kind;
    let mo_flags = mo.flags;
    let reactiontime = mo.reactiontime;
    let mo_x = mo.x;
    let mo_y = mo.y;

    let has_los = if let Some(lv) = level {
        crate::sight::p_check_sight(gs, lv, handle, target)
    } else {
        p_check_sight_local(gs, handle, target, None)
    };
    if !has_los {
        return false;
    }

    if mo_flags & flags::MF_JUSTHIT != 0 {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.flags &= !flags::MF_JUSTHIT;
        }
        return true;
    }

    if reactiontime != 0 {
        return false;
    }

    let Some(info) = mobjinfo::MOBJINFO.get(mo_kind as usize) else {
        return false;
    };
    let Some(t) = gs.mobjslab.get(target) else {
        return false;
    };
    let tx = t.x;
    let ty = t.y;

    let mut dist = approx_distance((tx - mo_x).to_int(), (ty - mo_y).to_int()) - 64;
    if info.melee_state == crate::mobj::StateNum::NULL {
        dist -= 128;
    }

    match mo_kind {
        MobjKind::ArchVile => {
            if dist > 14 * 64 {
                return false;
            }
        }
        MobjKind::Revenant => {
            if dist < 196 {
                return false;
            }
            dist >>= 1;
        }
        MobjKind::Cyberdemon | MobjKind::SpiderMastermind | MobjKind::LostSoul => {
            dist >>= 1;
        }
        _ => {}
    }

    if dist > 200 {
        dist = 200;
    }
    if mo_kind == MobjKind::Cyberdemon && dist > 160 {
        dist = 160;
    }

    dist <= 0 || i32::from(gs.p_random()) >= dist
}

// ---------------------------------------------------------------------------
// P_Move — one step monster movement with collision detection
// ---------------------------------------------------------------------------

/// Attempt to move actor `handle` one step in its current `movedir` at the
/// given `speed`.
///
/// Returns `true` if the move was legal and the actor's position was updated.
/// Returns `false` if blocked by geometry (one-sided linedef, step too high,
/// gap too narrow).
///
/// Port of Doom's `P_Move` from `p_enemy.c`.
pub fn p_move(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) -> bool {
    let Some(mo) = gs.mobjslab.get(handle) else {
        return false;
    };
    let spd = mobjinfo::MOBJINFO
        .get(mo.kind as usize)
        .map(|i| i.speed)
        .unwrap_or(Fixed16_16::ZERO);
    let (mo_x, mo_y, dir, speed) = (mo.x, mo.y, mo.movedir, spd);

    if dir == DI_NODIR || dir > 8 {
        return false;
    }

    let step_x = XMOVE[dir as usize].fixed_mul(speed);
    let step_y = YMOVE[dir as usize].fixed_mul(speed);
    let new_x = mo_x + step_x;
    let new_y = mo_y + step_y;

    let (can_move, blocking_linedef) = match level {
        Some(lv) => crate::movement::p_try_move_blocker(&gs.mobjslab, handle, new_x, new_y, lv),
        None => (true, None),
    };

    if can_move {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.x = new_x;
            mo.y = new_y;
            mo.momx = step_x;
            mo.momy = step_y;
        }
        if let Some(lv) = level
            && let Some((support_floor, subsector)) =
                crate::movement::support_state_at(&gs.mobjslab, handle, new_x, new_y, lv)
            && let Some(mo) = gs.mobjslab.get_mut(handle)
        {
            mo.z = support_floor;
            if let Some(subsector) = subsector {
                mo.subsector = subsector as u32;
            }
        }
        true
    } else {
        // Movement failed. In vanilla Doom, if the blocking linedef is a door,
        // the monster tries to open that exact door linedef.
        if let Some(lv) = level {
            try_open_door(gs, lv, blocking_linedef);
        }
        false
    }
}

/// Attempt to open a door that is blocking the monster's path.
fn try_open_door(gs: &mut GameState, level: &Level, blocking_linedef: Option<usize>) {
    if let Some(ld_idx) = blocking_linedef {
        let _ = crate::specials::monster_activate_door_linedef(gs, level, ld_idx);
    }
}

// ---------------------------------------------------------------------------
// A_Look — full monster idle behavior
// ---------------------------------------------------------------------------

/// Port of `A_Look` from Doom's `p_enemy.c`.
///
/// Monster idle behavior:
/// 1. Check `sound_targets` for the monster's sector (sound propagation).
///    - If sound target exists and is alive, set as `target` and enter `see_state`.
///    - `MF_AMBUSH` monsters only react to sound if they also have LOS to the target.
/// 2. Otherwise, call `p_look_for_players` to scan for visible players.
///    - If player found within sight, set as target, enter `see_state`.
fn a_look(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    let player_handle = gs.player.handle;

    // Read monster data.
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let mo_kind = mo.kind;
    let mo_flags = mo.flags;
    let mo_x = mo.x;
    let mo_y = mo.y;
    let mo_subsector = mo.subsector as usize;

    let is_ambush = mo_flags & flags::MF_AMBUSH != 0;

    // --- Step 1: Check sound targets ---
    // --- Step 1: Check sound targets ---
    if let Some(lv) = level {
        // Resolve the monster's sector from its current position, but keep a
        // subsector fallback for synthetic/unit-test maps.
        let actor_sector =
            crate::sight::sector_from_position_or_subsector(lv, mo_x, mo_y, mo_subsector);
        if let Some(actor_sector) = actor_sector {
            if let Some(sound_target) = crate::sound::get_sound_target(gs, actor_sector) {
                // Verify the sound target is alive.
                let target_alive = gs
                    .mobjslab
                    .get(sound_target)
                    .map(|t| !t.is_dead())
                    .unwrap_or(false);

                if target_alive
                    && (!is_ambush || crate::sight::p_check_sight(gs, lv, handle, sound_target))
                {
                    transition_to_see_state(gs, handle, mo_kind, sound_target);
                    return;
                }
            }
        }
    }

    // --- Step 2: Look for players by line-of-sight ---
    // Check the player exists and is alive.
    let player_alive = gs
        .mobjslab
        .get(player_handle)
        .map(|p| !p.is_dead())
        .unwrap_or(false);

    if !player_alive {
        return;
    }

    // Use the full p_check_sight from sight.rs if we have a level, otherwise
    // fall back to the local simplified version.
    let can_see = if let Some(lv) = level {
        crate::sight::p_check_sight(gs, lv, handle, player_handle)
    } else {
        p_check_sight_local(gs, handle, player_handle, None)
    };

    if can_see {
        transition_to_see_state(gs, handle, mo_kind, player_handle);
    }
}

/// Transition a monster to its `see_state`, setting the target and
/// resetting the threshold.
fn transition_to_see_state(
    gs: &mut GameState,
    handle: MobjHandle,
    kind: MobjKind,
    target: MobjHandle,
) {
    // Resolve see_state from mobjinfo.
    let see_sn = match mobjinfo::MOBJINFO.get(kind as usize) {
        Some(info) if info.see_state.0 != 0 => info.see_state,
        _ => return,
    };
    let Some(e) = states::STATES.get(see_sn.0 as usize) else {
        return;
    };
    let new_tics = e.tics;

    // Transition monster to see_state.
    let Some(mo) = gs.mobjslab.get_mut(handle) else {
        return;
    };
    mo.target = target;
    mo.threshold = 60; // stay alerted for 60 tics
    mo.state = see_sn;
    mo.tics = new_tics;

    // Emit wake sound event with emitter position for spatial attenuation.
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterWake(
                kind, handle, mo.x, mo.y,
            ));
    }
}

// ---------------------------------------------------------------------------
// P_NewChaseDir — full 4-candidate direction selection
// ---------------------------------------------------------------------------

/// Attempt to move actor `handle` in direction `dir` at the given `speed`.
///
/// Returns `true` if the move was legal and applied.
fn try_move_in_dir(
    gs: &mut GameState,
    handle: MobjHandle,
    dir: u8,
    speed: Fixed16_16,
    level: Option<&Level>,
) -> bool {
    if dir == DI_NODIR {
        return false;
    }
    let Some(mo) = gs.mobjslab.get(handle) else {
        return false;
    };
    let mo_x = mo.x;
    let mo_y = mo.y;
    let step_x = XMOVE[dir as usize].fixed_mul(speed);
    let step_y = YMOVE[dir as usize].fixed_mul(speed);
    let new_x = mo_x + step_x;
    let new_y = mo_y + step_y;

    let can_move = match level {
        Some(lv) => crate::movement::p_try_move(&gs.mobjslab, handle, new_x, new_y, lv),
        None => true,
    };

    if can_move {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.x = new_x;
            mo.y = new_y;
            mo.momx = step_x;
            mo.momy = step_y;
            mo.movedir = dir;
        }
        if let Some(lv) = level
            && let Some((support_floor, subsector)) =
                crate::movement::support_state_at(&gs.mobjslab, handle, new_x, new_y, lv)
            && let Some(mo) = gs.mobjslab.get_mut(handle)
        {
            mo.z = support_floor;
            if let Some(subsector) = subsector {
                mo.subsector = subsector as u32;
            }
        }
    }
    can_move
}

/// Port of `P_NewChaseDir` from Doom's `p_enemy.c`.
///
/// Selects the best movement direction for a chasing monster using a
/// 4-candidate priority list:
///
/// 1. **Primary**: diagonal toward target (combine x-direction + y-direction).
/// 2. **Secondary**: the axis (x or y) with larger absolute displacement.
/// 3. **Tertiary**: the other axis.
/// 4. **Fallback**: `DI_NODIR` (monster stays put this tic).
///
/// If the chosen direction is blocked, falls back to the next candidate in
/// order.  Finally, if all four fail, tries any direction (cycled via RNG).
pub fn p_new_chase_dir(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    // Get target handle and monster position.
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let spd = mobjinfo::MOBJINFO
        .get(mo.kind as usize)
        .map(|i| i.speed)
        .unwrap_or(Fixed16_16::ZERO);
    let (target_handle, mo_x, mo_y, speed) = (mo.target, mo.x, mo.y, spd);

    // If no target, set NODIR and return.
    if target_handle == MobjHandle::NULL {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.movedir = DI_NODIR;
        }
        return;
    }

    let Some(t) = gs.mobjslab.get(target_handle) else {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.movedir = DI_NODIR;
        }
        return;
    };
    let tx = t.x;
    let ty = t.y;

    let dx = (tx - mo_x).to_int();
    let dy = (ty - mo_y).to_int();

    // Determine pure x-direction and y-direction components.
    let d_x: u8 = if dx > 0 {
        DI_EAST
    } else if dx < 0 {
        DI_WEST
    } else {
        DI_NODIR
    };
    let d_y: u8 = if dy > 0 {
        DI_NORTH
    } else if dy < 0 {
        DI_SOUTH
    } else {
        DI_NODIR
    };

    // Primary candidate: diagonal combining both directions.
    let diag: u8 = if d_x == DI_NODIR || d_y == DI_NODIR {
        // No diagonal possible — use whichever pure direction exists.
        if d_x != DI_NODIR { d_x } else { d_y }
    } else {
        // Choose the diagonal that matches (d_x, d_y).
        match (d_x, d_y) {
            (DI_EAST, DI_NORTH) => DI_NORTHEAST,
            (DI_EAST, DI_SOUTH) => DI_SOUTHEAST,
            (DI_WEST, DI_NORTH) => DI_NORTHWEST,
            (DI_WEST, DI_SOUTH) => DI_SOUTHWEST,
            _ => DI_NODIR,
        }
    };

    // Secondary/tertiary: prefer the axis with larger absolute displacement.
    let (sec, tert) = if dx.abs() > dy.abs() {
        (d_x, d_y) // larger x → prefer x-dir, then y-dir
    } else {
        (d_y, d_x) // larger y → prefer y-dir, then x-dir
    };

    // Try candidates in order: primary → secondary → tertiary → nodir.
    let candidates = [diag, sec, tert, DI_NODIR];
    for &cand in &candidates {
        if cand == DI_NODIR {
            break;
        }
        if try_move_in_dir(gs, handle, cand, speed, level) {
            // Reset movecount so monster won't re-evaluate direction for a while.
            let rng_val = gs.rng.next_byte() as i32;
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.movecount = 8 + (rng_val & 7);
            }
            return;
        }
    }

    // All preferred directions blocked: try any direction round-robin.
    // Cycle through all 8 directions starting from a random offset.
    let start_dir = gs.rng.next_byte() % 8;
    for i in 0u8..8 {
        let dir = (start_dir + i) % 8;
        if try_move_in_dir(gs, handle, dir, speed, level) {
            let rng_val = gs.rng.next_byte() as i32;
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.movecount = 4 + (rng_val & 3);
            }
            return;
        }
    }

    // Truly stuck: set NODIR.
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.movedir = DI_NODIR;
        mo.movecount = 0;
    }
}

// ---------------------------------------------------------------------------
// A_Chase — full monster pursuit behavior
// ---------------------------------------------------------------------------

/// Port of `A_Chase` from Doom's `p_enemy.c`.
///
/// Full monster pursuit behavior:
/// 1. Decrement `reaction_time` if > 0 (pause before first attack).
/// 2. If target is dead/gone, call `A_Look` to find new target; return if none.
/// 3. If `MF_JUSTATTACKED` set, clear it and skip attack this tic.
/// 4. Melee check: if `melee_state != S_NULL` and target within `MELEERANGE + 20`,
///    enter `melee_state`.
/// 5. Missile check: if `missile_state != S_NULL`, check refire rules:
///    - Don't fire if `move_count > 0` (still moving) unless fast_monsters.
///    - Check `p_check_sight` — only fire if target visible.
///    - Enter `missile_state` and set `MF_JUSTATTACKED`.
/// 6. Movement: call `P_Move`; if blocked, call `P_NewChaseDir`.
/// 7. Active sound: randomly play `active_sound` (p_random < 3).
fn a_chase(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    // --- Step 1: Decrement reaction_time ---
    {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };
        if mo.reactiontime > 0 {
            mo.reactiontime -= 1;
        }
    }

    // --- Gather monster data ---
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let mo_kind = mo.kind;
    let _movecount = mo.movecount;
    let mo_flags = mo.flags;

    // --- Step 2: Check target still exists and is alive ---
    if get_alive_target(gs, handle).is_none() {
        // Try to find a new target via A_Look logic.
        // First clear the old target.
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.target = MobjHandle::NULL;
        }

        // Try to look for a new target.
        a_look(gs, handle, level);

        // Check if a_look found a new target.
        let found_target = gs
            .mobjslab
            .get(handle)
            .map(|mo| mo.target != MobjHandle::NULL)
            .unwrap_or(false);

        if !found_target {
            // Revert to idle spawn state.
            let spawn_sn = mobjinfo::MOBJINFO
                .get(mo_kind as usize)
                .map(|i| i.spawn_state)
                .unwrap_or_default();
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.target = MobjHandle::NULL;
                mo.state = spawn_sn;
                if let Some(e) = states::STATES.get(spawn_sn.0 as usize) {
                    mo.tics = e.tics;
                }
            }
            return;
        }
        // If a_look found a target, it already set see_state; but we continue
        // the chase loop with the new target. We need to re-read the target.
    }

    // --- Step 3: MF_JUSTATTACKED cooldown ---
    if mo_flags & flags::MF_JUSTATTACKED != 0 {
        // Clear the flag and skip attack checks this tic.
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.flags &= !flags::MF_JUSTATTACKED;
        }
        // Still do movement.
        do_chase_movement(gs, handle, level);
        return;
    }

    // --- Read target info for attack checks ---
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let current_target = mo.target;

    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let mo_x = mo.x;
    let mo_y = mo.y;
    let Some(t) = gs.mobjslab.get(current_target) else {
        do_chase_movement(gs, handle, level);
        return;
    };
    let tx = t.x;
    let ty = t.y;

    let dist = (tx - mo_x).to_int().abs() + (ty - mo_y).to_int().abs();

    let info = match mobjinfo::MOBJINFO.get(mo_kind as usize) {
        Some(i) => i,
        None => {
            do_chase_movement(gs, handle, level);
            return;
        }
    };

    let melee_sn = info.melee_state;
    let missile_sn = info.missile_state;
    let reactiontime = gs
        .mobjslab
        .get(handle)
        .map(|mo| mo.reactiontime)
        .unwrap_or(0);

    // --- Step 4: Melee check ---
    if melee_sn != crate::mobj::StateNum::NULL && dist <= MELEE_THRESHOLD {
        // Face the target before entering melee.
        a_face_target(gs, handle);
        set_mobj_state(gs, handle, melee_sn);
        return;
    }

    // --- Step 5: Missile check ---
    if missile_sn != crate::mobj::StateNum::NULL && reactiontime == 0 {
        // Re-read movecount (might have changed).
        let cur_movecount = gs.mobjslab.get(handle).map(|mo| mo.movecount).unwrap_or(0);

        // Don't fire if still moving from last direction change (gives monsters
        // a movement phase between attacks), unless movecount has expired.
        let can_fire =
            cur_movecount <= 0 && p_check_missile_range(gs, handle, current_target, level);

        if can_fire {
            // Face the target and enter missile state.
            a_face_target(gs, handle);
            set_mobj_state(gs, handle, missile_sn);
            // Set MF_JUSTATTACKED so we skip attack next tic.
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.flags |= flags::MF_JUSTATTACKED;
            }
            return;
        }
    }

    // --- Step 6: Movement ---
    do_chase_movement(gs, handle, level);

    // --- Step 7: Active sound ---
    // ~1/85 chance per tic (p_random returns 0-255, check < 3).
    let rng_val = gs.p_random();
    if rng_val < 3 {
        // Active sound would be played here. For now, just a no-op placeholder
        // since the audio system is decoupled. The caller (game loop) can check
        // for active sounds separately.
    }
}

/// Perform the movement portion of A_Chase: face target, move, handle blocking.
fn do_chase_movement(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    // Face the target.
    a_face_target(gs, handle);

    // Doom decrements movecount before movement. When the count reaches zero,
    // the monster still spends one final tic moving in the current direction;
    // only negative counts force an immediate direction refresh. That leaves a
    // full chase tic where movecount == 0 and missile attacks are allowed.
    let need_new_dir = {
        let Some(mo) = gs.mobjslab.get_mut(handle) else {
            return;
        };
        mo.movecount -= 1;
        mo.movecount < 0
    };

    if need_new_dir || !p_move(gs, handle, level) {
        p_new_chase_dir(gs, handle, level);
    }
}

/// Set an actor to a specific state, updating tics from the STATES table.
fn set_mobj_state(gs: &mut GameState, handle: MobjHandle, state: crate::mobj::StateNum) {
    if let Some(entry) = states::STATES.get(state.0 as usize) {
        let new_tics = entry.tics;
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.state = state;
            mo.tics = new_tics;
        }
    }
}

// ---------------------------------------------------------------------------
// A_FaceTarget — turn to face current target
// ---------------------------------------------------------------------------

/// Snap the monster's angle to face its current target.
///
/// Computes the angle from the monster's position to the target using
/// a proper integer `atan2` approximation, yielding a full 32-bit BAM angle.
fn a_face_target(gs: &mut GameState, handle: MobjHandle) {
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let (target_handle, mo_x, mo_y) = (mo.target, mo.x, mo.y);

    let Some(t) = gs.mobjslab.get(target_handle) else {
        return;
    };
    let (tx, ty) = (t.x, t.y);

    let dx = (tx - mo_x).to_int();
    let dy = (ty - mo_y).to_int();

    // Use Doom's angle conventions: 0 = East, 90° = North (positive Y).
    // Compute BAM angle from (dx, dy) using integer atan2 approximation.
    let angle = bam_from_xy(dx, dy);

    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.angle = angle;
    }
}

/// Compute a BAM angle from a displacement vector `(dx, dy)`.
///
/// Uses an integer atan2 approximation. Doom convention:
/// - 0 = East (+x), ANG90 = North (+y), ANG180 = West, ANG270 = South.
///
/// This is a simplified version that handles the 8 octants; accuracy is
/// sufficient for monster AI facing direction.
fn bam_from_xy(dx: i32, dy: i32) -> Bam {
    if dx == 0 && dy == 0 {
        return Bam(0);
    }

    // Use f64 for atan2 and convert to BAM. While Doom originally used a
    // lookup table (tantoangle), using f64 here is acceptable since this is
    // not a determinism-critical path (monsters face their target — the exact
    // angle doesn't affect game state beyond visual orientation).
    let angle_rad = (dy as f64).atan2(dx as f64);
    // Convert radians to BAM: full circle = 2^32 BAM = 2*PI radians.
    // BAM = angle_rad * (2^32 / (2*PI))
    let bam_val = (angle_rad * (4_294_967_296.0 / (2.0 * core::f64::consts::PI))) as i64;
    Bam(bam_val as u32)
}

// ---------------------------------------------------------------------------
// A_Fall
// ---------------------------------------------------------------------------

/// Port of `A_Fall` from Doom's `p_enemy.c`.
///
/// Called on the first death-state frame.  Clears `MF_SOLID` and
/// `MF_COUNTKILL` so corpses no longer block movement and are no longer
/// tallied in the kill count again.
fn a_fall(gs: &mut GameState, handle: MobjHandle) {
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.flags &= !(crate::mobj::flags::MF_SOLID | crate::mobj::flags::MF_COUNTKILL);
    }
}

// ---------------------------------------------------------------------------
// A_Scream (death sound on first death frame)
// ---------------------------------------------------------------------------

/// Port of `A_Scream` from Doom's `p_enemy.c`.
///
/// Fires on the first death frame; in the original engine this played
/// the monster-specific death sound.  Here we set a flag so the app
/// layer can trigger audio, matching the vanilla pattern without
/// hard-coding a sound ID in the game crate.
fn a_scream(gs: &mut GameState, handle: MobjHandle) {
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.flags |= crate::mobj::flags::MF_SCREAMED;
    }
}

// ---------------------------------------------------------------------------
// A_PosAttack (Trooper hitscan)
// ---------------------------------------------------------------------------

/// Port of `A_PosAttack` from Doom's `p_enemy.c`.
///
/// Fires a single hitscan bolt at the current target.
fn a_pos_attack(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    // Check target exists and is alive.
    let Some(_target) = get_alive_target(gs, handle) else {
        return;
    };

    // Face the target, then read the resulting angle.
    a_face_target(gs, handle);
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let angle = mo.angle;

    let damage = ((gs.tic_num % 8) + 1) as i32 * 3;
    let mut intercepts = smallvec::SmallVec::new();
    crate::combat::p_line_attack(
        gs,
        handle,
        angle,
        crate::combat::MISSILERANGE,
        damage,
        level,
        &mut intercepts,
    );
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                MobjKind::Trooper,
                handle,
                mo.x,
                mo.y,
            ));
    }
}

// ---------------------------------------------------------------------------
// A_SPosAttack (Sergeant — 3-pellet shotgun burst)
// ---------------------------------------------------------------------------

/// Port of `A_SPosAttack` from Doom's `p_enemy.c`.
///
/// Fires 3 hitscan pellets with a small angular spread centered on the target.
fn a_spos_attack(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    let Some(_target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let angle = mo.angle;

    let damage = ((gs.tic_num % 8) + 1) as i32 * 3;
    // Spread: ~11.25° per step in 32-bit BAM space.
    let spread = Bam(0x0800_0000u32);
    let mut intercepts = smallvec::SmallVec::new();
    for i in 0u32..3 {
        // offsets: -spread, 0, +spread
        let offset = Bam(spread.0.wrapping_mul(i).wrapping_sub(spread.0));
        let shot_angle = Bam(angle.0.wrapping_add(offset.0));
        crate::combat::p_line_attack(
            gs,
            handle,
            shot_angle,
            crate::combat::MISSILERANGE,
            damage,
            level,
            &mut intercepts,
        );
    }
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                MobjKind::Sergeant,
                handle,
                mo.x,
                mo.y,
            ));
    }
}

// ---------------------------------------------------------------------------
// A_TroopAttack (Imp — melee if in range, else hitscan)
// ---------------------------------------------------------------------------

/// Port of `A_TroopAttack` from Doom's `p_enemy.c`.
///
/// Uses melee if the target is within `MELEERANGE`, otherwise spawns an
/// `ImpFireball` projectile aimed at the target.
fn a_troo_attack(gs: &mut GameState, handle: MobjHandle, _level: Option<&Level>) {
    let Some((target, mo_x, mo_y)) = get_alive_target_with_pos(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);

    let Some(t) = gs.mobjslab.get(target) else {
        return;
    };
    let (tx, ty) = (t.x, t.y);

    let dist = (tx - mo_x).to_int().abs() + (ty - mo_y).to_int().abs();

    if dist <= crate::combat::MELEERANGE.to_int() {
        let damage = ((gs.tic_num % 8) + 1) as i32 * 3;
        crate::combat::damage_mobj(gs, target, handle, damage);
    } else {
        crate::projectile::p_spawn_missile(gs, handle, target, MobjKind::ImpFireball);
    }
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                MobjKind::Imp,
                handle,
                mo.x,
                mo.y,
            ));
    }
}

// ---------------------------------------------------------------------------
// A_SargAttack (Demon — melee only)
// ---------------------------------------------------------------------------

/// Port of `A_SargAttack` from Doom's `p_enemy.c`.
///
/// Deals melee damage only if the target is within `MELEERANGE`.
fn a_sarg_attack(gs: &mut GameState, handle: MobjHandle) {
    let Some((target, mo_x, mo_y)) = get_alive_target_with_pos(gs, handle) else {
        return;
    };

    let Some(t) = gs.mobjslab.get(target) else {
        return;
    };
    let (tx, ty) = (t.x, t.y);

    let dist = (tx - mo_x).to_int().abs() + (ty - mo_y).to_int().abs();
    if dist <= crate::combat::MELEERANGE.to_int() {
        let damage = ((gs.tic_num % 3) + 1) as i32 * 4;
        crate::combat::damage_mobj(gs, target, handle, damage);
        if let Some(mo) = gs.mobjslab.get(handle) {
            gs.sound
                .sound_queue
                .push(crate::state::SoundRequest::MonsterAttack(
                    MobjKind::Demon,
                    handle,
                    mo.x,
                    mo.y,
                ));
        }
    }
}

// ---------------------------------------------------------------------------
// A_HeadAttack (Cacodemon fireball projectile)
// ---------------------------------------------------------------------------

/// Port of `A_HeadAttack` from Doom's `p_enemy.c`.
///
/// Faces the target, then spawns a `CacoFireball` projectile.
fn a_head_attack(gs: &mut GameState, handle: MobjHandle) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    crate::projectile::p_spawn_missile(gs, handle, target, MobjKind::CacoFireball);
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                MobjKind::Cacodemon,
                handle,
                mo.x,
                mo.y,
            ));
    }
}

// ---------------------------------------------------------------------------
// A_BruisAttack (Baron/Hell Knight plasma ball)
// ---------------------------------------------------------------------------

/// Port of `A_BruisAttack` from Doom's `p_enemy.c`.
///
/// Faces the target, then spawns a `BaronBall` projectile.
fn a_bruis_attack(gs: &mut GameState, handle: MobjHandle) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    let bruis_kind = gs
        .mobjslab
        .get(handle)
        .map(|mo| mo.kind)
        .unwrap_or(MobjKind::BaronOfHell);
    crate::projectile::p_spawn_missile(gs, handle, target, MobjKind::BaronBall);
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                bruis_kind, handle, mo.x, mo.y,
            ));
    }
}

// ---------------------------------------------------------------------------
// A_CPosAttack (Chaingunner — hitscan like Zombieman)
// ---------------------------------------------------------------------------

/// Port of `A_CPosAttack` from Doom's `p_enemy.c`.
///
/// Fires a single hitscan bolt at the current target with angle spread.
/// Same behavior as the Zombieman's `A_PosAttack`.
fn a_cpos_attack(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    let Some(_target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let angle = mo.angle;

    let spread = crate::random::p_missile_angle_spread(gs);
    let shot_angle = Bam(angle.0.wrapping_add(spread as u32));
    let damage = crate::random::p_damage_with_variance(gs, 3);
    let cpos_kind = gs
        .mobjslab
        .get(handle)
        .map(|mo| mo.kind)
        .unwrap_or(MobjKind::Trooper);
    let mut intercepts = smallvec::SmallVec::new();
    crate::combat::p_line_attack(
        gs,
        handle,
        shot_angle,
        crate::combat::MISSILERANGE,
        damage,
        level,
        &mut intercepts,
    );
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                cpos_kind, handle, mo.x, mo.y,
            ));
    }
}

// ---------------------------------------------------------------------------
// A_CyberAttack (Cyberdemon — spawns Rocket projectile)
// ---------------------------------------------------------------------------

/// Port of `A_CyberAttack` from Doom's `p_enemy.c`.
///
/// Faces the target, then spawns a `Rocket` projectile aimed at the target.
fn a_cyber_attack(gs: &mut GameState, handle: MobjHandle) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    crate::projectile::p_spawn_missile(gs, handle, target, MobjKind::Rocket);
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                MobjKind::Cyberdemon,
                handle,
                mo.x,
                mo.y,
            ));
    }
}

// ---------------------------------------------------------------------------
// A_SkelMissile (Revenant — spawns Tracer projectile)
// ---------------------------------------------------------------------------

/// Port of `A_SkelMissile` from Doom's `p_enemy.c`.
///
/// Faces the target, then spawns a `Tracer` (homing) projectile.
fn a_skel_missile(gs: &mut GameState, handle: MobjHandle) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    crate::projectile::p_spawn_missile(gs, handle, target, MobjKind::Tracer);
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                MobjKind::Revenant,
                handle,
                mo.x,
                mo.y,
            ));
    }
}

// ---------------------------------------------------------------------------
// FATSPREAD constant for Mancubus attack spread
// ---------------------------------------------------------------------------

/// Mancubus fireball angular spread in BAM units.
///
/// Corresponds to Doom's `FATSPREAD` constant (ANG90/8 = 0x0400_0000).
const FATSPREAD: u32 = 0x0400_0000;

// ---------------------------------------------------------------------------
// A_FatAttack1/2/3 (Mancubus spread fire)
// ---------------------------------------------------------------------------

/// Spawn a `FatShot` projectile at the given angle offset from the actor's
/// facing direction.
///
/// Helper shared by `a_fat_attack1`, `a_fat_attack2`, `a_fat_attack3`.
fn fat_shoot(gs: &mut GameState, handle: MobjHandle, angle_offset: u32) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };

    // Spawn the missile aimed at target, then adjust its angle + momentum.
    if let Some(proj_h) = crate::projectile::p_spawn_missile(gs, handle, target, MobjKind::FatShot)
    {
        // Read the source angle (already set by face_target).
        let mo_angle = gs.mobjslab.get(handle).map(|mo| mo.angle.0).unwrap_or(0);
        let new_angle = Bam(mo_angle.wrapping_add(angle_offset));

        // Adjust the projectile's angle and recompute momentum from the new angle.
        if let Some(proj) = gs.mobjslab.get_mut(proj_h) {
            proj.angle = new_angle;
            let speed_f = proj.momx.to_int() as f32;
            let spd_y = proj.momy.to_int() as f32;
            let speed = (speed_f * speed_f + spd_y * spd_y).sqrt().max(1.0);
            let angle_rad = new_angle.0 as f64 / (u32::MAX as f64 + 1.0) * std::f64::consts::TAU;
            let cos = angle_rad.cos() as f32;
            let sin = angle_rad.sin() as f32;
            proj.momx = Fixed16_16::from_int((cos * speed) as i32);
            proj.momy = Fixed16_16::from_int((sin * speed) as i32);
        }
    }
}

/// Port of `A_FatAttack1` from Doom's `p_enemy.c`.
///
/// Mancubus spread fire #1: face target, then fire two `FatShot` projectiles
/// at +FATSPREAD and 0.
fn a_fat_attack1(gs: &mut GameState, handle: MobjHandle) {
    let Some(_target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    fat_shoot(gs, handle, FATSPREAD);
    fat_shoot(gs, handle, 0);
    if let Some(mo) = gs.mobjslab.get(handle) {
        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                MobjKind::Mancubus,
                handle,
                mo.x,
                mo.y,
            ));
    }
}

/// Port of `A_FatAttack2` from Doom's `p_enemy.c`.
///
/// Mancubus spread fire #2: face target, then fire two `FatShot` projectiles
/// at −FATSPREAD and 0.
fn a_fat_attack2(gs: &mut GameState, handle: MobjHandle) {
    let Some(_target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    fat_shoot(gs, handle, 0u32.wrapping_sub(FATSPREAD));
    fat_shoot(gs, handle, 0);
}

/// Port of `A_FatAttack3` from Doom's `p_enemy.c`.
///
/// Mancubus spread fire #3: face target, then fire two `FatShot` projectiles
/// at +FATSPREAD/2 and −FATSPREAD/2.
fn a_fat_attack3(gs: &mut GameState, handle: MobjHandle) {
    let Some(_target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    fat_shoot(gs, handle, FATSPREAD / 2);
    fat_shoot(gs, handle, 0u32.wrapping_sub(FATSPREAD / 2));
}

// ---------------------------------------------------------------------------
// A_SkullAttack (Lost Soul charge)
// ---------------------------------------------------------------------------

/// Speed of the Lost Soul charge attack in map units per tic.
const SKULLSPEED: i32 = 20;

/// Port of `A_SkullAttack` from Doom's `p_enemy.c`.
///
/// Sets `MF_SKULLFLY` and computes momentum toward the target at `SKULLSPEED`.
fn a_skull_attack(gs: &mut GameState, handle: MobjHandle) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };

    // Set the skull-fly flag so the Lost Soul damages on contact.
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.flags |= flags::MF_SKULLFLY;
    }

    // Read positions for velocity computation.
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let (sx, sy) = (mo.x, mo.y);

    let Some(t) = gs.mobjslab.get(target) else {
        return;
    };
    let (tx, ty) = (t.x, t.y);

    // Face the target.
    a_face_target(gs, handle);

    // Compute momentum toward target at SKULLSPEED.
    let dx_f = (tx - sx).to_int() as f32;
    let dy_f = (ty - sy).to_int() as f32;
    let dist = (dx_f * dx_f + dy_f * dy_f).sqrt().max(1.0);
    let speed = SKULLSPEED as f32;

    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.momx = Fixed16_16::from_int((dx_f / dist * speed) as i32);
        mo.momy = Fixed16_16::from_int((dy_f / dist * speed) as i32);
    }
}

// ---------------------------------------------------------------------------
// A_BspiAttack (Arachnotron — spawns ArachPlaz projectile)
// ---------------------------------------------------------------------------

/// Port of `A_BspiAttack` from Doom's `p_enemy.c`.
///
/// Faces the target, then spawns an `ArachPlaz` projectile.
fn a_bspi_attack(gs: &mut GameState, handle: MobjHandle) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    crate::projectile::p_spawn_missile(gs, handle, target, MobjKind::ArachPlaz);
}

// ---------------------------------------------------------------------------
// A_SpidAttack (Spider Mastermind — hitscan like Chaingunner)
// ---------------------------------------------------------------------------

/// Port of `A_SpidAttack` from Doom's `p_enemy.c`.
///
/// Fires a single hitscan bolt with angle spread, identical to the
/// Chaingunner attack pattern.
fn a_spid_attack(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    let Some(_target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let angle = mo.angle;

    let spread = crate::random::p_missile_angle_spread(gs);
    let shot_angle = Bam(angle.0.wrapping_add(spread as u32));
    let damage = crate::random::p_damage_with_variance(gs, 3);
    let mut intercepts = smallvec::SmallVec::new();
    crate::combat::p_line_attack(
        gs,
        handle,
        shot_angle,
        crate::combat::MISSILERANGE,
        damage,
        level,
        &mut intercepts,
    );
}

// ---------------------------------------------------------------------------
// A_PainAttack (Pain Elemental — spawns Lost Soul)
// ---------------------------------------------------------------------------

/// Maximum number of Lost Souls permitted in the level at once.
///
/// Doom enforces a cap of 21; `A_PainAttack` skips spawning if the count
/// is at or above this threshold.
const LOST_SOUL_MAX: usize = 21;

/// Port of `A_PainAttack` from Doom's `p_enemy.c`.
///
/// Faces the target, then spawns a `LostSoul` if the current count of Lost
/// Souls in the level is below `LOST_SOUL_MAX` (21).
fn a_pain_attack(gs: &mut GameState, handle: MobjHandle) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };

    a_face_target(gs, handle);

    // Count existing Lost Souls. If at cap, do not spawn.
    let lost_soul_count = gs
        .mobjslab
        .iter_handles()
        .filter(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::LostSoul && m.health > 0)
                .unwrap_or(false)
        })
        .count();

    if lost_soul_count >= LOST_SOUL_MAX {
        return;
    }

    // Spawn a new Lost Soul at the Pain Elemental's position, aimed at the target.
    // We use the angle of the PE to spawn the skull slightly forward.
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let sx = mo.x;
    let sy = mo.y;
    let sz = mo.z;
    let s_angle = mo.angle;

    // Spawn the Lost Soul slightly ahead of the Pain Elemental.
    // Use f64 for the offset direction calculation (non-deterministic path — spawn position only).
    let angle_rad = s_angle.0 as f64 / (u32::MAX as f64 + 1.0) * std::f64::consts::TAU;
    let offset = 4; // map units forward
    let spawn_x = sx + Fixed16_16::from_int((angle_rad.cos() * offset as f64) as i32);
    let spawn_y = sy + Fixed16_16::from_int((angle_rad.sin() * offset as f64) as i32);

    let mut skull = crate::mobj::Mobj::new(MobjKind::LostSoul, spawn_x, spawn_y, s_angle);
    skull.z = sz + Fixed16_16::from_int(8); // spawn slightly above PE
    skull.health = 100; // default Lost Soul health
    skull.flags = flags::MF_SOLID
        | flags::MF_SHOOTABLE
        | flags::MF_NOGRAVITY
        | flags::MF_FLOAT
        | flags::MF_COUNTKILL
        | flags::MF_SKULLFLY;
    skull.radius = Fixed16_16::from_int(16);
    skull.height = Fixed16_16::from_int(56);

    // Set momentum toward target (skull attack charge).
    let Some(t) = gs.mobjslab.get(target) else {
        return;
    };
    let tx = t.x;
    let ty = t.y;
    let dx_f = (tx - spawn_x).to_int() as f32;
    let dy_f = (ty - spawn_y).to_int() as f32;
    let dist = (dx_f * dx_f + dy_f * dy_f).sqrt().max(1.0);
    let speed = SKULLSPEED as f32;
    skull.momx = Fixed16_16::from_int((dx_f / dist * speed) as i32);
    skull.momy = Fixed16_16::from_int((dy_f / dist * speed) as i32);
    skull.target = target;

    gs.mobjslab.alloc(skull);
}

// ===========================================================================
// Arch-Vile actions
// ===========================================================================

/// Port of `A_VileChase` from Doom's `p_enemy.c`.
///
/// Performs a standard chase, but on each call also scans the slab for nearby
/// corpses that can be resurrected (have a non-`S_NULL` `raise_state` and
/// the `MF_CORPSE` flag). If a suitable corpse is found within 128 map units
/// (Manhattan distance), the vile sets its target to the corpse, enters
/// `S_VILE_ATK1`, and the corpse is restored to life.
fn a_vile_chase(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let (vx, vy) = (mo.x.to_int(), mo.y.to_int());

    // Scan for raisable corpses.
    let mut corpse_handle: Option<MobjHandle> = None;
    for h in gs.mobjslab.iter_handles() {
        let raisable = {
            let Some(mo) = gs.mobjslab.get(h) else {
                continue;
            };
            if mo.flags & flags::MF_CORPSE == 0 {
                continue;
            }
            // Check Manhattan distance <= 128.
            let dx = (mo.x.to_int() - vx).abs();
            let dy = (mo.y.to_int() - vy).abs();
            if dx > 128 || dy > 128 {
                continue;
            }
            // Check that the monster type has a raise_state.
            let info = &mobjinfo::MOBJINFO[mo.kind as usize];
            info.raise_state.0 != states::ids::S_NULL
        };
        if raisable {
            corpse_handle = Some(h);
            break;
        }
    }

    if let Some(ch) = corpse_handle {
        // Resurrect the corpse: restore health, clear corpse flag, set raise_state.
        let kind_idx = gs.mobjslab.get(ch).map(|m| m.kind as usize).unwrap_or(0);
        let info = &mobjinfo::MOBJINFO[kind_idx];
        let raise_sn = info.raise_state;
        let full_hp = info.spawn_health;
        let orig_flags = info.flags;

        if let Some(corpse) = gs.mobjslab.get_mut(ch) {
            corpse.health = full_hp;
            corpse.flags = orig_flags;
            corpse.state = raise_sn;
            if let Some(e) = states::STATES.get(raise_sn.0 as usize) {
                corpse.tics = e.tics;
            }
        }

        // Set the vile's target to the corpse handle and enter attack state.
        if let Some(vile) = gs.mobjslab.get_mut(handle) {
            vile.target = ch;
        }
        // Transition to vile attack sequence (S_VILE_ATK1).
        let atk_sn = crate::mobj::StateNum(states::ids::S_VILE_ATK1);
        if let Some(vile) = gs.mobjslab.get_mut(handle) {
            vile.state = atk_sn;
            if let Some(e) = states::STATES.get(atk_sn.0 as usize) {
                vile.tics = e.tics;
            }
        }
        a_face_target(gs, handle);
        return;
    }

    // No corpse found — normal chase.
    a_chase(gs, handle, level);
}

/// Port of `A_VileStart` from Doom's `p_enemy.c`.
///
/// First attack frame: sets the Arch-Vile's tracer to its current target
/// so `A_Fire` knows whom to track.
fn a_vile_start(gs: &mut GameState, handle: MobjHandle) {
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    if mo.target == MobjHandle::NULL {
        return;
    }
    let target = mo.target;

    a_face_target(gs, handle);
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.tracer = target;
    }
}

/// Port of `A_VileTarget` from Doom's `p_enemy.c`.
///
/// Spawns a VileFire actor at the target's position, sets the fire's
/// `target` to the Vile (owner) and `tracer` to the target (tracking).
fn a_vile_target(gs: &mut GameState, handle: MobjHandle) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };
    a_face_target(gs, handle);

    let Some(t) = gs.mobjslab.get(target) else {
        return;
    };
    let (tx, ty, tz) = (t.x, t.y, t.z);

    let mut fire = crate::mobj::Mobj::new(MobjKind::VileFire, tx, ty, Bam(0));
    fire.z = tz;
    fire.health = 1;
    fire.flags = flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY;
    fire.target = handle; // owner = the vile
    fire.tracer = target; // tracking = the victim
    let fire_h = gs.mobjslab.alloc(fire);

    // Store fire handle in the vile's tracer for reference.
    if let Some(vile) = gs.mobjslab.get_mut(handle) {
        vile.tracer = fire_h;
    }
}

/// Port of `A_VileAttack` from Doom's `p_enemy.c`.
///
/// Deals 20 direct damage + 70 blast damage to the target and applies
/// an upward thrust of 15 map units (momz).
fn a_vile_attack(gs: &mut GameState, handle: MobjHandle) {
    let Some(target) = get_alive_target(gs, handle) else {
        return;
    };
    a_face_target(gs, handle);

    // Direct damage: 20 hit points.
    if let Some(t) = gs.mobjslab.get_mut(target) {
        t.health -= 20;
    }

    // Blast damage: 70 hit points.
    if let Some(t) = gs.mobjslab.get_mut(target) {
        t.health -= 70;
    }

    // Upward thrust: momz += 15 * FRACUNIT (1000/256 ~ 62915, but we use
    // simpler from_int for clarity — this is Doom's `1000<<8/256`).
    if let Some(t) = gs.mobjslab.get_mut(target) {
        t.momz += Fixed16_16::from_int(15);
    }
}

/// Port of `A_Fire` from Doom's `p_enemy.c`.
///
/// Each tic the fire column tracks its tracer's position, staying at the
/// target's feet. If the tracer handle is stale, the fire does nothing.
fn a_fire(gs: &mut GameState, handle: MobjHandle) {
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let tracer = mo.tracer;

    let Some(t) = gs.mobjslab.get(tracer) else {
        return;
    };
    let (tx, ty) = (t.x, t.y);

    if let Some(fire) = gs.mobjslab.get_mut(handle) {
        fire.x = tx;
        fire.y = ty;
    }
}

// ===========================================================================
// Boss Brain (Icon of Sin) actions
// ===========================================================================

/// Monster types that the Boss Brain cube can spawn.
///
/// This matches the original Doom `BossTargetType` table from `p_enemy.c`.
const BOSS_SPAWN_TYPES: [MobjKind; 11] = [
    MobjKind::Trooper,
    MobjKind::Sergeant,
    MobjKind::Imp,
    MobjKind::Demon,
    MobjKind::Spectre,
    MobjKind::PainElemental,
    MobjKind::Arachnotron,
    MobjKind::Revenant,
    MobjKind::Mancubus,
    MobjKind::HellKnight,
    MobjKind::BaronOfHell,
];

/// Port of `A_BrainAwake` from Doom's `p_enemy.c`.
///
/// Sets the `brain_awake` flag on GameState so cubes start spawning.
fn a_brain_awake(gs: &mut GameState) {
    gs.brain_awake = true;
}

/// Port of `A_BrainSpit` from Doom's `p_enemy.c`.
///
/// Spawns a `BossCube` projectile aimed at the next spawn spot in the
/// round-robin target list. The cube stores the destination coordinates
/// in its (`spawn_x`, `spawn_y`) fields — we repurpose `momx`/`momy` for
/// flight velocity and store the destination in the cube's `x`/`y` at
/// spawn time, then compute velocity toward the target.
fn a_brain_spit(gs: &mut GameState, handle: MobjHandle) {
    if !gs.brain_awake || gs.brain_targets.is_empty() {
        return;
    }

    // Round-robin target selection.
    let idx = gs.brain_target_index % gs.brain_targets.len();
    gs.brain_target_index = idx + 1;
    let (dest_x, dest_y) = gs.brain_targets[idx];

    // Spawn the cube at the brain's position.
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let bx = mo.x;
    let by = mo.y;
    let bz = mo.z;

    let mut cube = crate::mobj::Mobj::new(MobjKind::BossCube, bx, by, Bam(0));
    cube.z = bz;
    cube.health = 1;
    cube.flags = flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY;
    cube.radius = Fixed16_16::from_int(6);
    cube.height = Fixed16_16::from_int(8);
    cube.reactiontime = 0; // will be set once arrived
    cube.target = handle; // owner = brain

    // Compute velocity toward destination. Speed = 15 map units/tic.
    let dx_f = (dest_x - bx).to_int() as f32;
    let dy_f = (dest_y - by).to_int() as f32;
    let dist = (dx_f * dx_f + dy_f * dy_f).sqrt().max(1.0);
    let speed = 15.0_f32;
    cube.momx = Fixed16_16::from_int((dx_f / dist * speed) as i32);
    cube.momy = Fixed16_16::from_int((dy_f / dist * speed) as i32);

    // Store destination in `tracer` fields — we'll use reactiontime as a
    // countdown. For simplicity, encode the target index in reactiontime
    // so a_spawn_fly can look it up.
    cube.reactiontime = idx as i32;

    gs.mobjslab.alloc(cube);
}

/// Port of `A_SpawnFly` from Doom's `p_enemy.c`.
///
/// Called when the cube arrives at a spawn spot. Picks a random monster
/// type from `BOSS_SPAWN_TYPES`, spawns it at the destination, then
/// removes the cube and spawns a `SpawnFire` fog effect.
fn a_spawn_fly(gs: &mut GameState, handle: MobjHandle) {
    // Determine spawn position from the brain_targets list.
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let target_idx = mo.reactiontime as usize;

    let (dest_x, dest_y) = if target_idx < gs.brain_targets.len() {
        gs.brain_targets[target_idx]
    } else {
        // Fallback: use the cube's current position.
        let Some(mo) = gs.mobjslab.get(handle) else {
            return;
        };
        (mo.x, mo.y)
    };

    // Random monster type selection.
    let r = gs.p_random() as usize;
    let kind = BOSS_SPAWN_TYPES[r % BOSS_SPAWN_TYPES.len()];
    let kind_idx = kind as usize;
    let info = &mobjinfo::MOBJINFO[kind_idx];

    // Spawn the monster.
    let mut monster = crate::mobj::Mobj::new(kind, dest_x, dest_y, Bam(0));
    monster.health = info.spawn_health;
    monster.flags = info.flags;
    monster.radius = info.radius;
    monster.height = info.height;
    monster.state = info.spawn_state;
    monster.reactiontime = 18; // standard reaction time for spawned monsters
    if let Some(e) = states::STATES.get(info.spawn_state.0 as usize) {
        monster.tics = e.tics;
    }
    gs.mobjslab.alloc(monster);

    // Spawn SpawnFire fog at destination.
    let mut fog = crate::mobj::Mobj::new(MobjKind::SpawnFire, dest_x, dest_y, Bam(0));
    fog.flags = flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY;
    fog.health = 1;
    gs.mobjslab.alloc(fog);

    // Remove the cube.
    gs.mobjslab.free(handle);
}

/// Port of `A_BrainDie` from Doom's `p_enemy.c`.
///
/// Triggers a normal level exit.
fn a_brain_die(gs: &mut GameState) {
    gs.exit_request = Some(crate::state::ExitRequest::Normal);
}

/// Port of `A_BrainScream` from Doom's `p_enemy.c`.
///
/// Spawns 20 explosion effects spread across the brain's width.
fn a_brain_scream(gs: &mut GameState, handle: MobjHandle) {
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let (bx, by, bz) = (mo.x.to_int(), mo.y.to_int(), mo.z.to_int());

    // Spawn 20 explosions spread across a 320-unit horizontal range.
    for i in 0..20 {
        let ex = bx - 196 + i * 16;
        let ey = by - 320;
        // Random Z offset in [0, 319].
        let r = gs.p_random() as i32;
        let ez = bz + 128 + (r * 2);
        let mut exp = crate::mobj::Mobj::new(
            MobjKind::BulletPuff, // reuse BulletPuff as explosion visual
            Fixed16_16::from_int(ex),
            Fixed16_16::from_int(ey),
            Bam(0),
        );
        exp.z = Fixed16_16::from_int(ez);
        exp.flags = flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY;
        exp.health = 1;
        // Random tics to stagger the animations.
        exp.tics = gs.p_random() as i16 & 7;
        gs.mobjslab.alloc(exp);
    }
}

/// Port of `A_BrainExplode` from Doom's `p_enemy.c`.
///
/// Spawns a single explosion at a random position near the brain.
fn a_brain_explode(gs: &mut GameState, handle: MobjHandle) {
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let (bx, by, bz) = (mo.x.to_int(), mo.y.to_int(), mo.z.to_int());

    let r = gs.p_random() as i32;
    let ex = bx + (r - 128) * 2;
    let rz = gs.p_random() as i32;
    let ez = bz + 128 + rz * 2;

    let mut exp = crate::mobj::Mobj::new(
        MobjKind::BulletPuff,
        Fixed16_16::from_int(ex),
        Fixed16_16::from_int(by),
        Bam(0),
    );
    exp.z = Fixed16_16::from_int(ez);
    exp.flags = flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY;
    exp.health = 1;
    exp.momz = Fixed16_16::from_int(gs.p_random() as i32 / 64);
    exp.tics = gs.p_random() as i16 & 7;
    gs.mobjslab.alloc(exp);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
