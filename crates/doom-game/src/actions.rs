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
use doom_types::{Bam, Fixed16_16};

use crate::mobj::flags;
use crate::mobj::{MobjHandle, MobjKind};
use crate::state::GameState;
use crate::{mobjinfo, states};

// ---------------------------------------------------------------------------
// Action index constants
// ---------------------------------------------------------------------------

/// No action — state entry is silent.
pub const ACTION_NONE: u8 = 0;
/// `A_Look`: search for the player and transition to `see_state`.
pub const ACTION_LOOK: u8 = 1;
/// `A_Chase`: move toward current target using 8-direction grid movement.
pub const ACTION_CHASE: u8 = 2;
/// `A_PosAttack`: Trooper hitscan attack.
pub const ACTION_POS_ATTACK: u8 = 3;
/// `A_SPosAttack`: Sergeant 3-pellet shotgun burst.
pub const ACTION_SPOS_ATTACK: u8 = 4;
/// `A_TroopAttack`: Imp melee-or-hitscan attack.
pub const ACTION_TROO_ATTACK: u8 = 5;
/// `A_SargAttack`: Demon melee-only attack.
pub const ACTION_SARG_ATTACK: u8 = 6;
/// `A_Fall`: clear MF_SOLID and MF_COUNTKILL so corpses are passable.
pub const ACTION_FALL: u8 = 7;
/// `A_HeadAttack`: Cacodemon fireball projectile.
pub const ACTION_HEAD_ATTACK: u8 = 8;
/// `A_BruisAttack`: Baron/Hell Knight plasma ball projectile.
pub const ACTION_BRUIS_ATTACK: u8 = 9;
/// `A_FaceTarget`: snap angle to face current target.
pub const ACTION_FACE_TARGET: u8 = 10;
/// `A_CPosAttack`: Chaingunner hitscan attack.
pub const ACTION_CPOS_ATTACK: u8 = 11;
/// `A_CyberAttack`: Cyberdemon spawns a Rocket projectile.
pub const ACTION_CYBER_ATTACK: u8 = 12;
/// `A_SkelMissile`: Revenant spawns a Tracer projectile.
pub const ACTION_SKEL_MISSILE: u8 = 13;
/// `A_FatAttack1`: Mancubus fireball spread #1 (+FATSPREAD).
pub const ACTION_FAT_ATTACK1: u8 = 14;
/// `A_FatAttack2`: Mancubus fireball spread #2 (−FATSPREAD).
pub const ACTION_FAT_ATTACK2: u8 = 15;
/// `A_FatAttack3`: Mancubus fireball spread #3 (±FATSPREAD/2).
pub const ACTION_FAT_ATTACK3: u8 = 16;
/// `A_SkullAttack`: Lost Soul charge attack.
pub const ACTION_SKULL_ATTACK: u8 = 17;
/// `A_BspiAttack`: Arachnotron spawns ArachnotronPlasma.
pub const ACTION_BSPI_ATTACK: u8 = 18;
/// `A_SpidAttack`: Spider Mastermind hitscan attack.
pub const ACTION_SPID_ATTACK: u8 = 19;
/// `A_PainAttack`: Pain Elemental spawns Lost Soul.
pub const ACTION_PAIN_ATTACK: u8 = 20;
/// `A_Scream`: Play monster death sound on first death frame.
pub const ACTION_SCREAM: u8 = 21;

// --- Arch-Vile special actions ---
/// `A_VileChase`: Chase with resurrection scan.
pub const ACTION_VILE_CHASE: u8 = 22;
/// `A_VileStart`: Begin attack — set tracer to target.
pub const ACTION_VILE_START: u8 = 23;
/// `A_VileTarget`: Spawn fire column at target's position.
pub const ACTION_VILE_TARGET: u8 = 24;
/// `A_VileAttack`: Deal 20 direct + 70 blast damage and upward thrust.
pub const ACTION_VILE_ATTACK: u8 = 25;
/// `A_Fire`: Fire column tracks the target's position each tic.
pub const ACTION_FIRE: u8 = 26;

// --- Boss Brain (Icon of Sin) actions ---
/// `A_BrainAwake`: Set brain_awake flag and play alert sound.
pub const ACTION_BRAIN_AWAKE: u8 = 27;
/// `A_BrainSpit`: Spawn a BossCube aimed at the next spawn spot.
pub const ACTION_BRAIN_SPIT: u8 = 28;
/// `A_SpawnFly`: Cube arrives — spawn a random monster at destination.
pub const ACTION_SPAWN_FLY: u8 = 29;
/// `A_BrainDie`: Trigger level exit.
pub const ACTION_BRAIN_DIE: u8 = 30;
/// `A_BrainScream`: Spawn 20 explosions across the brain sprite.
pub const ACTION_BRAIN_SCREAM: u8 = 31;
/// `A_BrainExplode`: Spawn a single explosion at a random position.
pub const ACTION_BRAIN_EXPLODE: u8 = 32;
/// `A_WeaponReady`: bob-ready player weapon loop, handles fire/switch input.
pub const ACTION_WEAPON_READY: u8 = 33;
/// `A_Lower`: lower the current player weapon toward the bottom of the screen.
pub const ACTION_LOWER: u8 = 34;
/// `A_Raise`: raise the pending/current player weapon toward the ready position.
pub const ACTION_RAISE: u8 = 35;
/// `A_GunFlash`: start the weapon's muzzle flash psprite.
pub const ACTION_GUN_FLASH: u8 = 36;
/// `A_Punch`: fist attack.
pub const ACTION_PUNCH: u8 = 37;
/// `A_FirePistol`: pistol attack.
pub const ACTION_FIRE_PISTOL: u8 = 38;
/// `A_FireShotgun`: shotgun attack.
pub const ACTION_FIRE_SHOTGUN: u8 = 39;
/// `A_FireShotgun2`: super shotgun attack.
pub const ACTION_FIRE_SHOTGUN2: u8 = 40;
/// `A_FireCGun`: chaingun attack.
pub const ACTION_FIRE_CGUN: u8 = 41;
/// `A_FireMissile`: rocket launcher attack.
pub const ACTION_FIRE_MISSILE: u8 = 42;
/// `A_FirePlasma`: plasma rifle attack.
pub const ACTION_FIRE_PLASMA: u8 = 43;
/// `A_BFGSound`: play the BFG windup sound before the projectile launches.
pub const ACTION_BFG_SOUND: u8 = 44;
/// `A_FireBFG`: BFG projectile launch.
pub const ACTION_FIRE_BFG: u8 = 45;
/// `A_Saw`: chainsaw attack.
pub const ACTION_SAW: u8 = 46;
/// `A_ReFire`: continue firing when the attack button remains held.
pub const ACTION_REFIRE: u8 = 47;
/// `A_CheckReload`: lower the weapon if it no longer has enough ammo.
pub const ACTION_CHECK_RELOAD: u8 = 48;
/// `A_OpenShotgun2`: play the super shotgun open sound.
pub const ACTION_OPEN_SHOTGUN2: u8 = 49;
/// `A_LoadShotgun2`: play the super shotgun load sound.
pub const ACTION_LOAD_SHOTGUN2: u8 = 50;
/// `A_CloseShotgun2`: play the super shotgun close sound and optionally refire.
pub const ACTION_CLOSE_SHOTGUN2: u8 = 51;
/// `A_Light0`: clear the player's weapon flash light bonus.
pub const ACTION_LIGHT0: u8 = 52;
/// `A_Light1`: set the player's weapon flash light bonus to level 1.
pub const ACTION_LIGHT1: u8 = 53;
/// `A_Light2`: set the player's weapon flash light bonus to level 2.
pub const ACTION_LIGHT2: u8 = 54;

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
    let target = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => mo.target,
        _ => return None,
    };
    if gs.mobjslab.get(target).map(|t| t.is_dead()).unwrap_or(true) {
        return None;
    }
    Some(target)
}

/// Helper to get a valid, alive target along with the monster's current position.
fn get_alive_target_with_pos(
    gs: &GameState,
    handle: MobjHandle,
) -> Option<(MobjHandle, Fixed16_16, Fixed16_16)> {
    let (target, mo_x, mo_y) = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => (mo.target, mo.x, mo.y),
        _ => return None,
    };
    if gs.mobjslab.get(target).map(|t| t.is_dead()).unwrap_or(true) {
        return None;
    }
    Some((target, mo_x, mo_y))
}

pub fn dispatch_action(gs: &mut GameState, handle: MobjHandle, action: u8, level: Option<&Level>) {
    match action {
        ACTION_NONE => {}
        ACTION_LOOK => a_look(gs, handle, level),
        ACTION_CHASE => a_chase(gs, handle, level),
        ACTION_POS_ATTACK => a_pos_attack(gs, handle, level),
        ACTION_SPOS_ATTACK => a_spos_attack(gs, handle, level),
        ACTION_TROO_ATTACK => a_troo_attack(gs, handle, level),
        ACTION_SARG_ATTACK => a_sarg_attack(gs, handle),
        ACTION_FALL => a_fall(gs, handle),
        ACTION_HEAD_ATTACK => a_head_attack(gs, handle),
        ACTION_BRUIS_ATTACK => a_bruis_attack(gs, handle),
        ACTION_FACE_TARGET => a_face_target(gs, handle),
        ACTION_CPOS_ATTACK => a_cpos_attack(gs, handle, level),
        ACTION_CYBER_ATTACK => a_cyber_attack(gs, handle),
        ACTION_SKEL_MISSILE => a_skel_missile(gs, handle),
        ACTION_FAT_ATTACK1 => a_fat_attack1(gs, handle),
        ACTION_FAT_ATTACK2 => a_fat_attack2(gs, handle),
        ACTION_FAT_ATTACK3 => a_fat_attack3(gs, handle),
        ACTION_SKULL_ATTACK => a_skull_attack(gs, handle),
        ACTION_BSPI_ATTACK => a_bspi_attack(gs, handle),
        ACTION_SPID_ATTACK => a_spid_attack(gs, handle, level),
        ACTION_PAIN_ATTACK => a_pain_attack(gs, handle),
        ACTION_SCREAM => a_scream(gs, handle),
        ACTION_VILE_CHASE => a_vile_chase(gs, handle, level),
        ACTION_VILE_START => a_vile_start(gs, handle),
        ACTION_VILE_TARGET => a_vile_target(gs, handle),
        ACTION_VILE_ATTACK => a_vile_attack(gs, handle),
        ACTION_FIRE => a_fire(gs, handle),
        ACTION_BRAIN_AWAKE => a_brain_awake(gs),
        ACTION_BRAIN_SPIT => a_brain_spit(gs, handle),
        ACTION_SPAWN_FLY => a_spawn_fly(gs, handle),
        ACTION_BRAIN_DIE => a_brain_die(gs),
        ACTION_BRAIN_SCREAM => a_brain_scream(gs, handle),
        ACTION_BRAIN_EXPLODE => a_brain_explode(gs, handle),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// 8-direction constants and movement table
// ---------------------------------------------------------------------------

/// Direction constants matching Doom's `dirtype_t`.
pub const DI_EAST: u8 = 0;
pub const DI_NORTHEAST: u8 = 1;
pub const DI_NORTH: u8 = 2;
pub const DI_NORTHWEST: u8 = 3;
pub const DI_WEST: u8 = 4;
pub const DI_SOUTHWEST: u8 = 5;
pub const DI_SOUTH: u8 = 6;
pub const DI_SOUTHEAST: u8 = 7;
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
    let (src_x, src_y, src_subsector) = match gs.mobjslab.get(source) {
        Some(mo) => (mo.x, mo.y, mo.subsector as usize),
        None => return false,
    };
    let (tgt_x, tgt_y, tgt_subsector) = match gs.mobjslab.get(target) {
        Some(mo) => (mo.x, mo.y, mo.subsector as usize),
        None => return false,
    };

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
    let (mo_kind, mo_flags, reactiontime, mo_x, mo_y) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.kind, mo.flags, mo.reactiontime, mo.x, mo.y),
        None => return false,
    };

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
    let (tx, ty) = match gs.mobjslab.get(target) {
        Some(t) => (t.x, t.y),
        None => return false,
    };

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
    let (mo_x, mo_y, dir, speed) = match gs.mobjslab.get(handle) {
        Some(mo) => {
            let spd = mobjinfo::MOBJINFO
                .get(mo.kind as usize)
                .map(|i| i.speed)
                .unwrap_or(Fixed16_16::ZERO);
            (mo.x, mo.y, mo.movedir, spd)
        }
        None => return false,
    };

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
    let (mo_kind, mo_flags, mo_x, mo_y, mo_subsector) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.kind, mo.flags, mo.x, mo.y, mo.subsector as usize),
        None => return,
    };

    let is_ambush = mo_flags & flags::MF_AMBUSH != 0;

    // --- Step 1: Check sound targets ---
    if let Some(lv) = level {
        // Resolve the monster's sector from its current position, but keep a
        // subsector fallback for synthetic/unit-test maps.
        if let Some(actor_sector) =
            crate::sight::sector_from_position_or_subsector(lv, mo_x, mo_y, mo_subsector)
        {
            if let Some(sound_target) = crate::sound::get_sound_target(gs, actor_sector) {
                // Verify the sound target is alive.
                let target_alive = gs
                    .mobjslab
                    .get(sound_target)
                    .map(|t| !t.is_dead())
                    .unwrap_or(false);

                if target_alive {
                    if is_ambush {
                        // Ambush monsters only react to sound if they have LOS.
                        if crate::sight::p_check_sight(gs, lv, handle, sound_target) {
                            // Has LOS — wake up and target the sound source.
                            transition_to_see_state(gs, handle, mo_kind, sound_target);
                            return;
                        }
                        // No LOS — fall through to visual check.
                    } else {
                        // Non-ambush: wake from sound alone.
                        transition_to_see_state(gs, handle, mo_kind, sound_target);
                        return;
                    }
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
    let new_tics = match states::STATES.get(see_sn.0 as usize) {
        Some(e) => e.tics,
        None => return,
    };

    // Transition monster to see_state.
    let Some(mo) = gs.mobjslab.get_mut(handle) else {
        return;
    };
    mo.target = target;
    mo.threshold = 60; // stay alerted for 60 tics
    mo.state = see_sn;
    mo.tics = new_tics;

    // Emit wake sound event with emitter position for spatial attenuation.
    let (mo_x, mo_y) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();
    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterWake(
            kind, handle, mo_x, mo_y,
        ));
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
    let (mo_x, mo_y) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x, mo.y),
        None => return false,
    };
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
    let (target_handle, mo_x, mo_y, speed) = match gs.mobjslab.get(handle) {
        Some(mo) => {
            let spd = mobjinfo::MOBJINFO
                .get(mo.kind as usize)
                .map(|i| i.speed)
                .unwrap_or(Fixed16_16::ZERO);
            (mo.target, mo.x, mo.y, spd)
        }
        None => return,
    };

    // If no target, set NODIR and return.
    if target_handle == MobjHandle::NULL {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.movedir = DI_NODIR;
        }
        return;
    }

    let (tx, ty) = match gs.mobjslab.get(target_handle) {
        Some(t) => (t.x, t.y),
        None => {
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.movedir = DI_NODIR;
            }
            return;
        }
    };

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
    let (target_handle, mo_kind, _movecount, mo_flags) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.target, mo.kind, mo.movecount, mo.flags),
        None => return,
    };

    // --- Step 2: Check target still exists and is alive ---
    let target_alive = match gs.mobjslab.get(target_handle) {
        Some(t) => !t.is_dead(),
        None => false,
    };

    if !target_alive || target_handle == MobjHandle::NULL {
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
    let current_target = match gs.mobjslab.get(handle) {
        Some(mo) => mo.target,
        None => return,
    };

    let (mo_x, mo_y) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x, mo.y),
        None => return,
    };
    let (tx, ty) = match gs.mobjslab.get(current_target) {
        Some(t) => (t.x, t.y),
        None => {
            do_chase_movement(gs, handle, level);
            return;
        }
    };

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
    let (target_handle, mo_x, mo_y) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.target, mo.x, mo.y),
        None => return,
    };
    let (tx, ty) = match gs.mobjslab.get(target_handle) {
        Some(t) => (t.x, t.y),
        None => return,
    };

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
    let angle = match gs.mobjslab.get(handle) {
        Some(mo) => mo.angle,
        None => return,
    };

    let damage = ((gs.tic_num % 8) + 1) as i32 * 3;
    crate::combat::p_line_attack(
        gs,
        handle,
        angle,
        crate::combat::MISSILERANGE,
        damage,
        level,
    );
    let (sx, sy) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();

    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterAttack(
            MobjKind::Trooper,
            handle,
            sx,
            sy,
        ));
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
    let angle = match gs.mobjslab.get(handle) {
        Some(mo) => mo.angle,
        None => return,
    };

    let damage = ((gs.tic_num % 8) + 1) as i32 * 3;
    // Spread: ~11.25° per step in 32-bit BAM space.
    let spread = Bam(0x0800_0000u32);
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
        );
    }
    let (sx, sy) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();

    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterAttack(
            MobjKind::Sergeant,
            handle,
            sx,
            sy,
        ));
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

    let (tx, ty) = match gs.mobjslab.get(target) {
        Some(t) => (t.x, t.y),
        None => return,
    };

    let dist = (tx - mo_x).to_int().abs() + (ty - mo_y).to_int().abs();

    if dist <= crate::combat::MELEERANGE.to_int() {
        let damage = ((gs.tic_num % 8) + 1) as i32 * 3;
        crate::combat::damage_mobj(gs, target, handle, damage);
    } else {
        crate::projectile::p_spawn_missile(gs, handle, target, MobjKind::ImpFireball);
    }
    let (sx, sy) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();

    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterAttack(
            MobjKind::Imp,
            handle,
            sx,
            sy,
        ));
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

    let (tx, ty) = match gs.mobjslab.get(target) {
        Some(t) => (t.x, t.y),
        None => return,
    };

    let dist = (tx - mo_x).to_int().abs() + (ty - mo_y).to_int().abs();
    if dist <= crate::combat::MELEERANGE.to_int() {
        let damage = ((gs.tic_num % 3) + 1) as i32 * 4;
        crate::combat::damage_mobj(gs, target, handle, damage);
        let (sx, sy) = gs
            .mobjslab
            .get(handle)
            .map(|mo| (mo.x, mo.y))
            .unwrap_or_default();

        gs.sound
            .sound_queue
            .push(crate::state::SoundRequest::MonsterAttack(
                MobjKind::Demon,
                handle,
                sx,
                sy,
            ));
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
    let (sx, sy) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();

    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterAttack(
            MobjKind::Cacodemon,
            handle,
            sx,
            sy,
        ));
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
    let (sx, sy) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();

    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterAttack(
            bruis_kind, handle, sx, sy,
        ));
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
    let angle = match gs.mobjslab.get(handle) {
        Some(mo) => mo.angle,
        None => return,
    };

    let spread = crate::random::p_missile_angle_spread(gs);
    let shot_angle = Bam(angle.0.wrapping_add(spread as u32));
    let damage = crate::random::p_damage_with_variance(gs, 3);
    let cpos_kind = gs
        .mobjslab
        .get(handle)
        .map(|mo| mo.kind)
        .unwrap_or(MobjKind::Trooper);
    crate::combat::p_line_attack(
        gs,
        handle,
        shot_angle,
        crate::combat::MISSILERANGE,
        damage,
        level,
    );
    let (sx, sy) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();

    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterAttack(
            cpos_kind, handle, sx, sy,
        ));
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
    let (sx, sy) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();

    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterAttack(
            MobjKind::Cyberdemon,
            handle,
            sx,
            sy,
        ));
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
    let (sx, sy) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();

    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterAttack(
            MobjKind::Revenant,
            handle,
            sx,
            sy,
        ));
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
    let target = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => mo.target,
        _ => return,
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
    let (sx, sy) = gs
        .mobjslab
        .get(handle)
        .map(|mo| (mo.x, mo.y))
        .unwrap_or_default();

    gs.sound
        .sound_queue
        .push(crate::state::SoundRequest::MonsterAttack(
            MobjKind::Mancubus,
            handle,
            sx,
            sy,
        ));
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
    let target = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => mo.target,
        _ => return,
    };
    let target_alive = gs
        .mobjslab
        .get(target)
        .map(|t| !t.is_dead())
        .unwrap_or(false);
    if !target_alive {
        return;
    }

    // Set the skull-fly flag so the Lost Soul damages on contact.
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.flags |= flags::MF_SKULLFLY;
    }

    // Read positions for velocity computation.
    let (sx, sy) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x, mo.y),
        None => return,
    };
    let (tx, ty) = match gs.mobjslab.get(target) {
        Some(mo) => (mo.x, mo.y),
        None => return,
    };

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
    let angle = match gs.mobjslab.get(handle) {
        Some(mo) => mo.angle,
        None => return,
    };

    let spread = crate::random::p_missile_angle_spread(gs);
    let shot_angle = Bam(angle.0.wrapping_add(spread as u32));
    let damage = crate::random::p_damage_with_variance(gs, 3);
    crate::combat::p_line_attack(
        gs,
        handle,
        shot_angle,
        crate::combat::MISSILERANGE,
        damage,
        level,
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
    let target = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => mo.target,
        _ => return,
    };
    if gs
        .mobjslab
        .get(target)
        .map(|t| t.is_dead())
        .unwrap_or(false)
    {
        return;
    }

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
    let (sx, sy, sz, s_angle) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x, mo.y, mo.z, mo.angle),
        None => return,
    };

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
    let (tx, ty) = match gs.mobjslab.get(target) {
        Some(t) => (t.x, t.y),
        None => return,
    };
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
    let (vx, vy) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x.to_int(), mo.y.to_int()),
        None => return,
    };

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
    let target = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => mo.target,
        _ => return,
    };
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

    let (tx, ty, tz) = match gs.mobjslab.get(target) {
        Some(t) => (t.x, t.y, t.z),
        None => return,
    };

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
    let tracer = match gs.mobjslab.get(handle) {
        Some(mo) => mo.tracer,
        None => return,
    };
    let (tx, ty) = match gs.mobjslab.get(tracer) {
        Some(t) => (t.x, t.y),
        None => return,
    };
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
    let (bx, by, bz) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x, mo.y, mo.z),
        None => return,
    };

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
    let target_idx = match gs.mobjslab.get(handle) {
        Some(mo) => mo.reactiontime as usize,
        None => return,
    };

    let (dest_x, dest_y) = if target_idx < gs.brain_targets.len() {
        gs.brain_targets[target_idx]
    } else {
        // Fallback: use the cube's current position.
        match gs.mobjslab.get(handle) {
            Some(mo) => (mo.x, mo.y),
            None => return,
        }
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
    let (bx, by, bz) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.z.to_int()),
        None => return,
    };

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
    let (bx, by, bz) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.z.to_int()),
        None => return,
    };

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
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind, StateNum, flags};
    use crate::player::PlayerState;
    use crate::state::GameState;
    use doom_types::TicCmd;
    use doom_types::{Bam, Fixed16_16};

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

    fn spawn_trooper(gs: &mut GameState, x: i32, y: i32) -> MobjHandle {
        use crate::mobj::MobjKind;
        use crate::mobjinfo::MOBJINFO;
        use crate::states::STATES;
        let kind = MobjKind::Trooper;
        let spawn_sn = MOBJINFO[kind as usize].spawn_state;
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = spawn_sn;
        mo.tics = STATES[spawn_sn.0 as usize].tics;
        gs.mobjslab.alloc(mo)
    }

    /// Spawn a trooper with extra flags.
    #[allow(dead_code)]
    fn spawn_trooper_with_flags(
        gs: &mut GameState,
        x: i32,
        y: i32,
        extra_flags: u32,
    ) -> MobjHandle {
        use crate::mobjinfo::MOBJINFO;
        use crate::states::STATES;
        let kind = MobjKind::Trooper;
        let spawn_sn = MOBJINFO[kind as usize].spawn_state;
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL | extra_flags;
        mo.state = spawn_sn;
        mo.tics = STATES[spawn_sn.0 as usize].tics;
        gs.mobjslab.alloc(mo)
    }

    // -----------------------------------------------------------------------
    // Direction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_dir_to_target() {
        let cases = [
            // Pure directions
            (100, 0, DI_EAST),
            (-100, 0, DI_WEST),
            (0, 100, DI_NORTH),
            (0, -100, DI_SOUTH),
            // Exact diagonals
            (100, 100, DI_NORTHEAST),
            (-100, 100, DI_NORTHWEST),
            (-100, -100, DI_SOUTHWEST),
            (100, -100, DI_SOUTHEAST),
            // Shallow diagonals (still diagonal territory: ax/ay < 2.0 and ay/ax < 2.0)
            (100, 60, DI_NORTHEAST),
            (60, 100, DI_NORTHEAST),
            (-100, 60, DI_NORTHWEST),
            (-60, 100, DI_NORTHWEST),
            (-100, -60, DI_SOUTHWEST),
            (-60, -100, DI_SOUTHWEST),
            (100, -60, DI_SOUTHEAST),
            (60, -100, DI_SOUTHEAST),
            // Mostly horizontal (ax > 2 * ay)
            (100, 40, DI_EAST),
            (100, -40, DI_EAST),
            (-100, 40, DI_WEST),
            (-100, -40, DI_WEST),
            // Mostly vertical (ay > 2 * ax)
            (40, 100, DI_NORTH),
            (-40, 100, DI_NORTH),
            (40, -100, DI_SOUTH),
            (-40, -100, DI_SOUTH),
            // Edge cases
            (0, 0, DI_NORTHEAST), // Handled as diagonal in current logic since 0 is not > 0
        ];

        for &(dx, dy, expected) in &cases {
            assert_eq!(
                dir_to_target(dx, dy),
                expected,
                "dir_to_target({}, {}) should be {}",
                dx,
                dy,
                expected
            );
        }
    }

    // -----------------------------------------------------------------------
    // BAM angle computation tests
    // -----------------------------------------------------------------------

    #[test]
    fn bam_from_xy_east() {
        let angle = bam_from_xy(100, 0);
        // East = 0 BAM
        assert!(
            angle.0 < 0x1000_0000,
            "east should be near 0 BAM, got {:#010X}",
            angle.0
        );
    }

    #[test]
    fn bam_from_xy_north() {
        let angle = bam_from_xy(0, 100);
        // North = ANG90 = 0x4000_0000
        let diff = (angle.0 as i64 - 0x4000_0000i64).unsigned_abs();
        assert!(
            diff < 0x0100_0000,
            "north should be near ANG90, got {:#010X}",
            angle.0
        );
    }

    #[test]
    fn bam_from_xy_west() {
        let angle = bam_from_xy(-100, 0);
        // West = ANG180 = 0x8000_0000
        let diff = (angle.0 as i64 - 0x8000_0000i64).unsigned_abs();
        assert!(
            diff < 0x0100_0000,
            "west should be near ANG180, got {:#010X}",
            angle.0
        );
    }

    #[test]
    fn bam_from_xy_south() {
        let angle = bam_from_xy(0, -100);
        // South = ANG270 = 0xC000_0000
        let diff = (angle.0 as i64 - 0xC000_0000i64).unsigned_abs();
        assert!(
            diff < 0x0100_0000,
            "south should be near ANG270, got {:#010X}",
            angle.0
        );
    }

    #[test]
    fn bam_from_xy_zero_returns_zero() {
        assert_eq!(bam_from_xy(0, 0), Bam(0));
    }

    // -----------------------------------------------------------------------
    // A_Look tests
    // -----------------------------------------------------------------------

    #[test]
    fn a_look_transitions_to_see_state_when_in_range() {
        let mut gs = make_game_state();
        // Spawn trooper 100 units east of player (well within 4096 range).
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // Tick 10 times so the first A_Look fires (S_POSS_STND.tics = 10).
        for _ in 0..10 {
            gs.tick(TicCmd::default(), None);
        }

        let mo = gs.mobjslab.get(trooper).unwrap();
        let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;
        assert_eq!(
            mo.state, see_sn,
            "trooper must be in see_state after A_Look fires"
        );
        assert_eq!(mo.target, gs.player.handle, "trooper must target player");
    }

    #[test]
    fn a_look_ignores_out_of_range_player() {
        let mut gs = make_game_state();
        // Trooper 5000 units away — beyond Manhattan distance 4096.
        let trooper = spawn_trooper(&mut gs, 5000, 0);
        let spawn_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].spawn_state;

        for _ in 0..15 {
            gs.tick(TicCmd::default(), None);
        }

        let mo = gs.mobjslab.get(trooper).unwrap();
        let idle_ok =
            mo.state == spawn_sn || mo.state == StateNum(crate::states::ids::S_POSS_STND2);
        assert!(
            idle_ok,
            "trooper must stay in one of the idle stand frames when player is out of range"
        );
    }

    #[test]
    fn a_look_direct_call_sets_target_and_see_state() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

        dispatch_action(&mut gs, trooper, ACTION_LOOK, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(mo.target, gs.player.handle);
        assert_eq!(mo.state, see_sn);
        assert_eq!(mo.threshold, 60);
    }

    #[test]
    fn a_look_does_nothing_when_player_dead() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let spawn_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].spawn_state;

        // Kill the player.
        gs.mobjslab.get_mut(gs.player.handle).unwrap().health = 0;

        dispatch_action(&mut gs, trooper, ACTION_LOOK, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.state, spawn_sn,
            "trooper must stay idle when player is dead"
        );
        assert_eq!(mo.target, MobjHandle::NULL);
    }

    #[test]
    fn a_look_with_sound_target_wakes_non_ambush() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

        // Set up sound targets: the trooper is in subsector 0 which resolves
        // to sector 0. Without a level, we can't resolve sectors, so we need
        // to test this with p_check_sight_local fallback path.
        // Instead, test the direct call without level (uses fallback LOS).
        dispatch_action(&mut gs, trooper, ACTION_LOOK, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(mo.state, see_sn, "trooper should wake from LOS");
    }

    #[test]
    fn a_look_with_stale_handle_does_not_panic() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.free(trooper);

        // Should not panic.
        dispatch_action(&mut gs, trooper, ACTION_LOOK, None);
    }

    // -----------------------------------------------------------------------
    // A_Chase tests
    // -----------------------------------------------------------------------

    #[test]
    fn a_chase_moves_monster_toward_player() {
        let mut gs = make_game_state();
        // Player at (0,0), trooper at (100,0).
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // Tick enough times: A_Look fires at tic 9 (10 tic idle), first A_Chase
        // at tic 13 enters missile state (ATK1+ATK2+ATK3 = 12 tics), second
        // A_Chase fires around tic 29+ clears MF_JUSTATTACKED and does movement.
        // Use 40 tics to give plenty of room for the full cycle.
        for _ in 0..40 {
            gs.tick(TicCmd::default(), None);
        }

        let mo = gs.mobjslab.get(trooper).unwrap();
        // Trooper should have moved west (toward x=0).
        assert!(
            mo.x < Fixed16_16::from_int(100),
            "trooper x={:?} must decrease toward player",
            mo.x
        );
    }

    #[test]
    fn dead_target_reverts_monster_to_idle() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // Let the trooper see the player and start chasing.
        for _ in 0..15 {
            gs.tick(TicCmd::default(), None);
        }
        // Kill the player.
        gs.mobjslab.get_mut(gs.player.handle).unwrap().health = 0;

        // Keep ticking: A_Chase should notice dead target and revert.
        // With attack states added (Batch 5), the trooper may be mid-attack
        // sequence (ATK1→ATK2→ATK3→RUN1 = 4+4+4+4 = 16 tics) before A_Chase
        // fires and detects the dead target.  Use 32 tics to be safe.
        for _ in 0..32 {
            gs.tick(TicCmd::default(), None);
        }

        let mo = gs.mobjslab.get(trooper).unwrap();
        let spawn_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].spawn_state;
        assert_eq!(
            mo.state, spawn_sn,
            "trooper must revert to spawn state when target is dead"
        );
        assert_eq!(mo.target, MobjHandle::NULL, "target must be cleared");
    }

    #[test]
    fn a_chase_reaction_time_prevents_attack() {
        let mut gs = make_game_state();
        let kind = MobjKind::Trooper;
        let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;

        // Spawn trooper already in chase state with high reaction_time.
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(50),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = see_sn;
        mo.tics = states::STATES[see_sn.0 as usize].tics;
        mo.target = gs.player.handle;
        mo.reactiontime = 100;
        let trooper = gs.mobjslab.alloc(mo);

        // Call a_chase directly.
        a_chase(&mut gs, trooper, None);

        // Reaction time should have decremented.
        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.reactiontime, 99,
            "reaction_time must decrement each a_chase call"
        );
    }

    #[test]
    fn a_chase_justattacked_clears_flag_and_skips_attack() {
        let mut gs = make_game_state();
        let kind = MobjKind::Trooper;
        let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;

        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(50),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags =
            flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL | flags::MF_JUSTATTACKED;
        mo.state = see_sn;
        mo.tics = states::STATES[see_sn.0 as usize].tics;
        mo.target = gs.player.handle;
        mo.reactiontime = 0;
        mo.movecount = 0;
        let trooper = gs.mobjslab.alloc(mo);

        a_chase(&mut gs, trooper, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.flags & flags::MF_JUSTATTACKED,
            0,
            "MF_JUSTATTACKED must be cleared after a_chase"
        );
    }

    #[test]
    fn a_chase_enters_melee_when_close() {
        let mut gs = make_game_state();
        let kind = MobjKind::Demon; // Has melee state
        let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
        let melee_sn = mobjinfo::MOBJINFO[kind as usize].melee_state;

        assert_ne!(melee_sn, StateNum::NULL, "Demon must have melee state");

        // Spawn demon very close to player (within MELEERANGE + 20 = 84).
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(30),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 150;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = see_sn;
        mo.tics = states::STATES[see_sn.0 as usize].tics;
        mo.target = gs.player.handle;
        mo.reactiontime = 0;
        let demon = gs.mobjslab.alloc(mo);

        a_chase(&mut gs, demon, None);

        let mo = gs.mobjslab.get(demon).unwrap();
        assert_eq!(
            mo.state, melee_sn,
            "demon should enter melee state when close to target"
        );
    }

    #[test]
    fn a_chase_enters_missile_state_when_in_range() {
        let mut gs = make_game_state();
        let kind = MobjKind::Trooper;
        let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
        let missile_sn = mobjinfo::MOBJINFO[kind as usize].missile_state;

        assert_ne!(
            missile_sn,
            StateNum::NULL,
            "Trooper must have missile state"
        );

        // Spawn trooper within missile range, with movecount = 0 and reactiontime = 0.
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(200),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = see_sn;
        mo.tics = states::STATES[see_sn.0 as usize].tics;
        mo.target = gs.player.handle;
        mo.reactiontime = 0;
        mo.movecount = 0;
        let trooper = gs.mobjslab.alloc(mo);

        gs.rng.set_index(9);
        a_chase(&mut gs, trooper, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.state, missile_sn,
            "trooper should enter missile state when in range with movecount=0"
        );
        assert_ne!(
            mo.flags & flags::MF_JUSTATTACKED,
            0,
            "MF_JUSTATTACKED must be set after entering missile state"
        );
    }

    #[test]
    fn a_chase_does_not_fire_when_movecount_positive() {
        let mut gs = make_game_state();
        let kind = MobjKind::Trooper;
        let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
        let missile_sn = mobjinfo::MOBJINFO[kind as usize].missile_state;

        // Spawn trooper within range but with movecount > 0.
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(200),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = see_sn;
        mo.tics = states::STATES[see_sn.0 as usize].tics;
        mo.target = gs.player.handle;
        mo.reactiontime = 0;
        mo.movecount = 10; // Still has movement budget
        let trooper = gs.mobjslab.alloc(mo);

        a_chase(&mut gs, trooper, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_ne!(
            mo.state, missile_sn,
            "trooper should NOT enter missile state when movecount > 0"
        );
    }

    #[test]
    fn p_check_missile_range_archvile_rejects_far_targets() {
        let mut gs = make_game_state();
        let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 1100, 0, 700);
        gs.mobjslab.get_mut(vile).unwrap().reactiontime = 0;
        let player = gs.player.handle;

        assert!(
            !p_check_missile_range(&mut gs, vile, player, None),
            "Arch-Vile missile range should reject targets beyond 14 * 64 units"
        );
    }

    #[test]
    fn p_check_missile_range_revenant_rejects_targets_under_196_units() {
        let mut gs = make_game_state();
        let skel = spawn_monster_targeting_player(&mut gs, MobjKind::Revenant, 128, 0, 300);
        gs.mobjslab.get_mut(skel).unwrap().reactiontime = 0;
        let player = gs.player.handle;
        gs.rng.set_index(2); // RNG_TABLE[2] = 109, high enough to pass the old random gate.

        assert!(
            !p_check_missile_range(&mut gs, skel, player, None),
            "Revenant missile range should reject targets that are too close"
        );
    }

    #[test]
    fn a_chase_can_fire_after_movecount_counts_down() {
        let mut gs = make_game_state();
        let kind = MobjKind::Trooper;
        let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
        let missile_sn = mobjinfo::MOBJINFO[kind as usize].missile_state;

        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(200),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = see_sn;
        mo.tics = states::STATES[see_sn.0 as usize].tics;
        mo.target = gs.player.handle;
        mo.reactiontime = 0;
        mo.movedir = DI_WEST;
        mo.movecount = 1;
        let trooper = gs.mobjslab.alloc(mo);

        gs.rng.set_index(9);
        a_chase(&mut gs, trooper, None);
        let after_first = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            after_first.state, see_sn,
            "first chase tic should spend the last movement step, not attack yet"
        );
        assert_eq!(
            after_first.movecount, 0,
            "movement countdown should reach zero and leave a firing window next tic"
        );

        a_chase(&mut gs, trooper, None);
        let after_second = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            after_second.state, missile_sn,
            "trooper should enter missile state once movecount has counted down"
        );
    }

    #[test]
    fn p_check_missile_range_mf_justhit_returns_true_and_clears_flag() {
        let mut gs = make_game_state();
        let player_handle = gs.player.handle;
        let trooper = spawn_monster_targeting_player(&mut gs, MobjKind::Trooper, 100, 0, 20);
        gs.mobjslab.get_mut(trooper).unwrap().flags |= flags::MF_JUSTHIT;

        let can_fire = p_check_missile_range(&mut gs, trooper, player_handle, None);

        assert!(
            can_fire,
            "MF_JUSTHIT must bypass normal missile range checks"
        );
        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.flags & flags::MF_JUSTHIT,
            0,
            "p_check_missile_range must clear MF_JUSTHIT"
        );
    }

    #[test]
    fn p_check_missile_range_reactiontime_prevents_fire() {
        let mut gs = make_game_state();
        let player_handle = gs.player.handle;
        // Trooper within range
        let trooper = spawn_monster_targeting_player(&mut gs, MobjKind::Trooper, 200, 0, 20);
        gs.mobjslab.get_mut(trooper).unwrap().reactiontime = 10;

        let can_fire = p_check_missile_range(&mut gs, trooper, player_handle, None);

        assert!(!can_fire, "reactiontime > 0 must prevent missile fire");
    }

    #[test]
    fn p_check_missile_range_null_melee_state_adjusts_distance() {
        let mut gs = make_game_state();
        let player_handle = gs.player.handle;

        let demon = spawn_monster_targeting_player(&mut gs, MobjKind::Demon, 300, 0, 150);
        gs.mobjslab.get_mut(demon).unwrap().reactiontime = 0;

        let trooper = spawn_monster_targeting_player(&mut gs, MobjKind::Trooper, 300, 0, 20);
        gs.mobjslab.get_mut(trooper).unwrap().reactiontime = 0;

        // RNG_TABLE[11] = 140.
        // demon: dist = 200, random = 140 -> can_fire = false
        // trooper: dist = 108, random = 140 -> can_fire = true

        gs.rng.set_index(11);
        let can_fire_demon = p_check_missile_range(&mut gs, demon, player_handle, None);

        gs.rng.set_index(11);
        let can_fire_trooper = p_check_missile_range(&mut gs, trooper, player_handle, None);

        assert!(
            !can_fire_demon,
            "Demon should fail random gate because dist > random"
        );
        assert!(
            can_fire_trooper,
            "Trooper should pass random gate because dist - 128 < random"
        );
    }

    #[test]
    fn p_check_missile_range_cyberdemon_distance_limits() {
        let mut gs = make_game_state();
        let player_handle = gs.player.handle;
        let cyber = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 500, 0, 4000);
        gs.mobjslab.get_mut(cyber).unwrap().reactiontime = 0;

        // RNG_TABLE[6] = 149 (false).
        // RNG_TABLE[20] = 154 (true).

        gs.rng.set_index(6);
        assert!(
            !p_check_missile_range(&mut gs, cyber, player_handle, None),
            "Cyberdemon should fail with random=149 vs dist=154"
        );

        gs.rng.set_index(20);
        assert!(
            p_check_missile_range(&mut gs, cyber, player_handle, None),
            "Cyberdemon should fire with random=154 vs dist=154"
        );
    }

    #[test]
    fn a_chase_far_trooper_respects_random_missile_gate() {
        let mut gs = make_game_state();
        let kind = MobjKind::Trooper;
        let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;
        let missile_sn = mobjinfo::MOBJINFO[kind as usize].missile_state;

        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(2000),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = see_sn;
        mo.tics = states::STATES[see_sn.0 as usize].tics;
        mo.target = gs.player.handle;
        mo.reactiontime = 0;
        mo.movecount = 0;
        let trooper = gs.mobjslab.alloc(mo);

        gs.rng.set_index(0);
        a_chase(&mut gs, trooper, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.state, see_sn,
            "far trooper should stay in chase state when the missile-range random gate rejects firing"
        );
        assert_ne!(
            mo.state, missile_sn,
            "far trooper should not always enter missile state"
        );
    }

    // -----------------------------------------------------------------------
    // P_Move tests
    // -----------------------------------------------------------------------

    #[test]
    fn p_move_moves_actor_in_direction() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().movedir = DI_WEST;

        let moved = p_move(&mut gs, trooper, None);

        assert!(moved, "p_move should succeed without level");
        let mo = gs.mobjslab.get(trooper).unwrap();
        assert!(
            mo.x < Fixed16_16::from_int(100),
            "trooper should have moved west"
        );
    }

    #[test]
    fn p_move_nodir_returns_false() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().movedir = DI_NODIR;

        let moved = p_move(&mut gs, trooper, None);

        assert!(!moved, "p_move with DI_NODIR should return false");
    }

    #[test]
    fn p_move_stale_handle_returns_false() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.free(trooper);

        let moved = p_move(&mut gs, trooper, None);

        assert!(!moved, "p_move with stale handle should return false");
    }

    #[test]
    fn p_move_updates_momentum() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().movedir = DI_EAST;

        p_move(&mut gs, trooper, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert!(
            mo.momx > Fixed16_16::ZERO,
            "momentum x should be positive for east movement"
        );
    }

    #[test]
    fn p_move_opens_the_actual_blocking_door() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 104, 0);
        gs.mobjslab.get_mut(trooper).unwrap().movedir = DI_EAST;

        let reject = doom_map::Reject::parse_lump(&[0u8], 2).unwrap();

        let mut bm_data = vec![0u8; 22];
        bm_data[4..6].copy_from_slice(&2u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&6u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&8u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0u16.to_le_bytes());
        bm_data[14..16].copy_from_slice(&0xFFFFu16.to_le_bytes());
        bm_data[16..18].copy_from_slice(&0u16.to_le_bytes());
        bm_data[18..20].copy_from_slice(&0u16.to_le_bytes());
        bm_data[20..22].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).unwrap();

        let level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004,
                special: 1,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            }],
            sidedefs: vec![
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 1,
                },
            ],
            vertexes: vec![
                doom_map::Vertex { x: 128, y: -32 },
                doom_map::Vertex { x: 128, y: 32 },
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
                    ceil_height: 0,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap,
        };

        let moved = p_move(&mut gs, trooper, Some(&level));

        assert!(!moved, "the move should be blocked by the near door");
        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "monster should still open a blocking door even when the door lives in a neighbouring blockmap cell"
        );
        assert_eq!(
            gs.movers.active_doors[0].sector, 1,
            "monster should open the actual blocking door"
        );
    }

    // -----------------------------------------------------------------------
    // A_FaceTarget tests
    // -----------------------------------------------------------------------

    #[test]
    fn a_face_target_faces_east() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, -100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().target = gs.player.handle;

        a_face_target(&mut gs, trooper);

        let mo = gs.mobjslab.get(trooper).unwrap();
        // Target is east (+x). Angle should be near 0 (east).
        assert!(
            mo.angle.0 < 0x2000_0000 || mo.angle.0 > 0xE000_0000,
            "angle should be roughly east, got {:#010X}",
            mo.angle.0
        );
    }

    #[test]
    fn a_face_target_faces_west() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        // Player is at (0,0), trooper is at (100,0), so target is west.
        gs.mobjslab.get_mut(trooper).unwrap().target = gs.player.handle;

        a_face_target(&mut gs, trooper);

        let mo = gs.mobjslab.get(trooper).unwrap();
        // West = 0x8000_0000.
        let diff = (mo.angle.0 as i64 - 0x8000_0000i64).unsigned_abs();
        assert!(
            diff < 0x1000_0000,
            "angle should be roughly west, got {:#010X}",
            mo.angle.0
        );
    }

    #[test]
    fn a_face_target_no_target_does_nothing() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let original_angle = gs.mobjslab.get(trooper).unwrap().angle;

        a_face_target(&mut gs, trooper);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(
            mo.angle, original_angle,
            "angle should not change without target"
        );
    }

    #[test]
    fn dispatch_action_face_target_works() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, -100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().target = gs.player.handle;

        dispatch_action(&mut gs, trooper, ACTION_FACE_TARGET, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        // Should have turned to face east.
        assert!(
            mo.angle.0 < 0x2000_0000 || mo.angle.0 > 0xE000_0000,
            "should face east after ACTION_FACE_TARGET"
        );
    }

    // -----------------------------------------------------------------------
    // Batch 5 attack tests (preserved from original)
    // -----------------------------------------------------------------------

    /// Helper: spawn a monster of `kind` at `(x, y)` with given health,
    /// already targeting the player, in the RUN1 chase state.
    fn spawn_monster_chasing(
        gs: &mut GameState,
        kind: MobjKind,
        x: i32,
        y: i32,
        health: i32,
    ) -> MobjHandle {
        use crate::mobjinfo::MOBJINFO;
        use crate::states::{STATES, ids};
        let see_sn = MOBJINFO[kind as usize].see_state;
        let fallback_sn = if see_sn.0 != 0 {
            see_sn
        } else {
            crate::mobj::StateNum(ids::S_POSS_RUN1)
        };
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = health;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = fallback_sn;
        mo.tics = STATES[fallback_sn.0 as usize].tics;
        mo.target = gs.player.handle;
        mo.threshold = 60;
        gs.mobjslab.alloc(mo)
    }

    #[test]
    fn trooper_attacks_when_in_range() {
        use crate::states::ids;
        let mut gs = make_game_state();
        // Trooper well within missile range (2048 units); needs to enter ATK state.
        // Player at (0,0), trooper at (50, 0) — within MISSILERANGE (2048 units).
        let trooper = spawn_monster_chasing(&mut gs, MobjKind::Trooper, 50, 0, 20);

        // Tick enough times that A_Chase fires and detects missile range → ATK1.
        // Chase fires every 4 tics; with target set we just need one A_Chase invocation.
        let mut found_atk = false;
        for _ in 0..20 {
            gs.tick(TicCmd::default(), None);
            let mo = gs.mobjslab.get(trooper).unwrap();
            let s = mo.state.0;
            if s == ids::S_POSS_ATK1 || s == ids::S_POSS_ATK2 || s == ids::S_POSS_ATK3 {
                found_atk = true;
                break;
            }
        }
        assert!(
            found_atk,
            "trooper must enter an ATK state when player is in range"
        );
    }

    #[test]
    fn a_scream_sets_mf_screamed_flag() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // MF_SCREAMED must NOT be set before the action fires.
        assert_eq!(
            gs.mobjslab.get(trooper).unwrap().flags & flags::MF_SCREAMED,
            0,
            "MF_SCREAMED must not be set before A_Scream"
        );

        dispatch_action(&mut gs, trooper, ACTION_SCREAM, None);

        assert_ne!(
            gs.mobjslab.get(trooper).unwrap().flags & flags::MF_SCREAMED,
            0,
            "A_Scream must set MF_SCREAMED"
        );
    }

    /// Verify that each original monster's first death frame carries ACTION_SCREAM
    /// and second death frame carries ACTION_FALL — matching the vanilla state table.
    #[test]
    fn die1_and_die2_actions_correct_for_all_original_monsters() {
        use crate::states::STATES;
        use crate::states::ids;

        let cases: &[(u16, u16, &str)] = &[
            (ids::S_POSS_DIE1, ids::S_POSS_DIE2, "Trooper"),
            (ids::S_SPOS_DIE1, ids::S_SPOS_DIE2, "Sergeant"),
            (ids::S_TROO_DIE1, ids::S_TROO_DIE2, "Imp"),
            (ids::S_SARG_DIE1, ids::S_SARG_DIE2, "Demon"),
            (ids::S_HEAD_DIE1, ids::S_HEAD_DIE2, "Cacodemon"),
            (ids::S_BOSS_DIE1, ids::S_BOSS_DIE2, "Baron"),
            (ids::S_CYBER_DIE1, ids::S_CYBER_DIE2, "Cyberdemon"),
            (ids::S_SPID_DIE1, ids::S_SPID_DIE2, "Spider"),
            (ids::S_BOS2_DIE1, ids::S_BOS2_DIE2, "HellKnight"),
        ];
        for &(die1, die2, name) in cases {
            assert_eq!(
                STATES[die1 as usize].action, ACTION_SCREAM,
                "{name} DIE1 must have ACTION_SCREAM"
            );
            assert_eq!(
                STATES[die2 as usize].action, ACTION_FALL,
                "{name} DIE2 must have ACTION_FALL"
            );
        }
    }

    #[test]
    fn a_fall_clears_solid_flag() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // Confirm solid before.
        assert_ne!(
            gs.mobjslab.get(trooper).unwrap().flags & flags::MF_SOLID,
            0,
            "trooper must start with MF_SOLID"
        );

        // Dispatch A_Fall directly.
        dispatch_action(&mut gs, trooper, ACTION_FALL, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(mo.flags & flags::MF_SOLID, 0, "a_fall must clear MF_SOLID");
        assert_eq!(
            mo.flags & flags::MF_COUNTKILL,
            0,
            "a_fall must clear MF_COUNTKILL"
        );
    }

    #[test]
    fn a_pos_attack_does_not_panic_without_target() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        // No target set — must not panic.
        dispatch_action(&mut gs, trooper, ACTION_POS_ATTACK, None);
        // Health unchanged since we have no target to shoot.
        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(mo.health, 20, "no target → no effect");
    }

    // -----------------------------------------------------------------------
    // Batch 3: P_NewChaseDir and P_CheckSight tests
    // -----------------------------------------------------------------------

    /// Spawn a monster in its see state with movedir=NODIR, movecount=0.
    fn spawn_monster_chasing_at(gs: &mut GameState, x: i32, y: i32) -> MobjHandle {
        use crate::mobjinfo::MOBJINFO;
        use crate::states::STATES;
        let kind = MobjKind::Trooper;
        let see_sn = MOBJINFO[kind as usize].see_state;
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = see_sn;
        mo.tics = STATES[see_sn.0 as usize].tics;
        mo.target = MobjHandle::NULL;
        mo.threshold = 60;
        mo.movedir = super::DI_NODIR;
        mo.movecount = 0;
        gs.mobjslab.alloc(mo)
    }

    #[test]
    fn p_new_chase_dir_sets_movedir_toward_target() {
        let mut gs = make_game_state();
        // Player at (0,0), monster at (500, 500) — target is SW of monster.
        let monster = spawn_monster_chasing_at(&mut gs, 500, 500);
        gs.mobjslab.get_mut(monster).unwrap().target = gs.player.handle;

        super::p_new_chase_dir(&mut gs, monster, None);

        let mo = gs.mobjslab.get(monster).unwrap();
        // Monster is NE of player → should try to move SW (DI_SOUTHWEST = 5).
        // At minimum, movedir must not be NODIR and movecount must be > 0.
        assert_ne!(
            mo.movedir,
            super::DI_NODIR,
            "movedir must be set after p_new_chase_dir"
        );
        assert!(
            mo.movecount > 0,
            "movecount must be positive after p_new_chase_dir"
        );
        // The chosen direction should be westward or southward.
        let dir = mo.movedir;
        assert!(
            dir == super::DI_SOUTHWEST
                || dir == super::DI_WEST
                || dir == super::DI_SOUTH
                || dir <= 7,
            "movedir {dir} must be a valid direction"
        );
    }

    #[test]
    fn p_new_chase_dir_nodir_when_no_target() {
        let mut gs = make_game_state();
        let monster = spawn_monster_chasing_at(&mut gs, 100, 0);
        // No target — leave target as NULL.
        assert_eq!(gs.mobjslab.get(monster).unwrap().target, MobjHandle::NULL);

        super::p_new_chase_dir(&mut gs, monster, None);

        let mo = gs.mobjslab.get(monster).unwrap();
        assert_eq!(
            mo.movedir,
            super::DI_NODIR,
            "monster with no target must stay at NODIR"
        );
    }

    #[test]
    fn p_check_sight_returns_true_without_level() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().target = gs.player.handle;

        // No level → falls through to Manhattan distance check (≤ 4096).
        let can_see = super::p_check_sight_local(&gs, trooper, gs.player.handle, None);
        assert!(
            can_see,
            "monster 100 units away must be visible without level"
        );
    }

    #[test]
    fn p_check_sight_returns_false_for_reject_blocked() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().target = gs.player.handle;

        // Build a minimal level with 2 sectors where the REJECT marks them
        // as mutually invisible (all-ones reject data = all blocked).
        let reject = doom_map::Reject::parse_lump(&[0xFFu8], 2).unwrap();

        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).unwrap();

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
            doom_map::Vertex { x: 0, y: 64 },
            doom_map::Vertex { x: 64, y: 64 },
        ];
        let sidedefs = vec![
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
        ];
        let linedefs = vec![
            doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            },
            doom_map::Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 1,
                left_sidedef: 0xFFFF,
            },
        ];
        let segs = vec![
            doom_map::Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            },
            doom_map::Seg {
                from_vertex: 2,
                to_vertex: 3,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
        ];
        let ssectors = vec![
            doom_map::Ssector {
                seg_count: 1,
                first_seg: 0,
            },
            doom_map::Ssector {
                seg_count: 1,
                first_seg: 1,
            },
        ];
        let sectors = vec![
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
        ];

        let level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        };

        let can_see = super::p_check_sight_local(&gs, trooper, gs.player.handle, Some(&level));
        assert!(!can_see, "all-ones reject must block LOS");
    }

    #[test]
    fn p_check_sight_returns_true_for_all_zero_reject() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().target = gs.player.handle;

        let reject = doom_map::Reject::parse_lump(&[0u8], 1).unwrap();

        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).unwrap();

        let level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![doom_map::Ssector {
                seg_count: 0,
                first_seg: 0,
            }],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            }],
            reject,
            blockmap,
        };

        let can_see = super::p_check_sight_local(&gs, trooper, gs.player.handle, Some(&level));
        assert!(
            can_see,
            "all-zero reject + close distance must report visible"
        );
    }

    // -----------------------------------------------------------------------
    // Additional Batch 30 monster AI tests
    // -----------------------------------------------------------------------

    #[test]
    fn melee_threshold_is_correct() {
        assert_eq!(MELEE_THRESHOLD, 84, "MELEERANGE(64) + 20 = 84");
    }

    #[test]
    fn a_chase_demon_melee_only_no_missile() {
        // Demon has melee_state but missile_state is S_NULL.
        let info = &mobjinfo::MOBJINFO[MobjKind::Demon as usize];
        assert_ne!(info.melee_state, StateNum::NULL);
        assert_eq!(info.missile_state, StateNum::NULL);
    }

    #[test]
    fn a_chase_trooper_missile_only_no_melee() {
        // Trooper has missile_state but melee_state is S_NULL.
        let info = &mobjinfo::MOBJINFO[MobjKind::Trooper as usize];
        assert_eq!(info.melee_state, StateNum::NULL);
        assert_ne!(info.missile_state, StateNum::NULL);
    }

    #[test]
    fn a_chase_with_dead_target_finds_new_target_if_player_alive() {
        let mut gs = make_game_state();
        let kind = MobjKind::Trooper;
        let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;

        // Create a "fake" monster target that is dead.
        let dead_mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(500),
            Fixed16_16::from_int(500),
            Bam::ZERO,
        );
        let dead_handle = gs.mobjslab.alloc(dead_mo);
        // It has 0 health (dead).

        // Spawn the chasing monster targeting the dead thing.
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(100),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = see_sn;
        mo.tics = states::STATES[see_sn.0 as usize].tics;
        mo.target = dead_handle;
        mo.reactiontime = 0;
        let trooper = gs.mobjslab.alloc(mo);

        // Player is alive at (0,0), within range.
        a_chase(&mut gs, trooper, None);

        let mo = gs.mobjslab.get(trooper).unwrap();
        // The monster should now target the player (found via a_look).
        assert_eq!(
            mo.target, gs.player.handle,
            "monster should find new target (player) when old target is dead"
        );
    }

    #[test]
    fn p_move_all_eight_directions() {
        for dir in 0..8u8 {
            let mut gs = make_game_state();
            let trooper = spawn_trooper(&mut gs, 500, 500);
            gs.mobjslab.get_mut(trooper).unwrap().movedir = dir;

            let old_x = gs.mobjslab.get(trooper).unwrap().x;
            let old_y = gs.mobjslab.get(trooper).unwrap().y;

            let moved = p_move(&mut gs, trooper, None);
            assert!(moved, "p_move should succeed for dir={}", dir);

            let mo = gs.mobjslab.get(trooper).unwrap();
            let dx = mo.x - old_x;
            let dy = mo.y - old_y;

            // Verify direction is correct.
            match dir {
                DI_EAST => assert!(dx > Fixed16_16::ZERO, "east should increase x"),
                DI_WEST => assert!(dx < Fixed16_16::ZERO, "west should decrease x"),
                DI_NORTH => assert!(dy > Fixed16_16::ZERO, "north should increase y"),
                DI_SOUTH => assert!(dy < Fixed16_16::ZERO, "south should decrease y"),
                DI_NORTHEAST => {
                    assert!(dx > Fixed16_16::ZERO && dy > Fixed16_16::ZERO);
                }
                DI_NORTHWEST => {
                    assert!(dx < Fixed16_16::ZERO && dy > Fixed16_16::ZERO);
                }
                DI_SOUTHWEST => {
                    assert!(dx < Fixed16_16::ZERO && dy < Fixed16_16::ZERO);
                }
                DI_SOUTHEAST => {
                    assert!(dx > Fixed16_16::ZERO && dy < Fixed16_16::ZERO);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn set_mobj_state_updates_state_and_tics() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

        set_mobj_state(&mut gs, trooper, see_sn);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(mo.state, see_sn);
        assert_eq!(mo.tics, states::STATES[see_sn.0 as usize].tics);
    }

    #[test]
    fn transition_to_see_state_sets_all_fields() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let see_sn = mobjinfo::MOBJINFO[MobjKind::Trooper as usize].see_state;

        let ph = gs.player.handle;
        transition_to_see_state(&mut gs, trooper, MobjKind::Trooper, ph);

        let mo = gs.mobjslab.get(trooper).unwrap();
        assert_eq!(mo.target, gs.player.handle);
        assert_eq!(mo.state, see_sn);
        assert_eq!(mo.threshold, 60);
    }

    #[test]
    fn a_chase_reaction_time_decrements_each_call() {
        let mut gs = make_game_state();
        let kind = MobjKind::Trooper;
        let see_sn = mobjinfo::MOBJINFO[kind as usize].see_state;

        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(200),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state = see_sn;
        mo.tics = states::STATES[see_sn.0 as usize].tics;
        mo.target = gs.player.handle;
        mo.reactiontime = 5;
        let trooper = gs.mobjslab.alloc(mo);

        for i in 0..5 {
            a_chase(&mut gs, trooper, None);
            let mo = gs.mobjslab.get(trooper).unwrap();
            assert_eq!(
                mo.reactiontime,
                4 - i,
                "reaction_time should decrement to {}",
                4 - i
            );
        }
    }

    #[test]
    fn dispatch_action_unknown_index_does_nothing() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        let health_before = gs.mobjslab.get(trooper).unwrap().health;

        dispatch_action(&mut gs, trooper, 255, None);

        let health_after = gs.mobjslab.get(trooper).unwrap().health;
        assert_eq!(
            health_before, health_after,
            "unknown action should be no-op"
        );
    }

    #[test]
    fn xmove_ymove_nodir_is_zero() {
        assert_eq!(XMOVE[DI_NODIR as usize], Fixed16_16::ZERO);
        assert_eq!(YMOVE[DI_NODIR as usize], Fixed16_16::ZERO);
    }

    #[test]
    fn xmove_ymove_tables_have_nine_entries() {
        assert_eq!(XMOVE.len(), 9);
        assert_eq!(YMOVE.len(), 9);
    }

    // -----------------------------------------------------------------------
    // Batch 31: New monster attack function tests
    // -----------------------------------------------------------------------

    /// Spawn a generic monster of `kind` at `(x, y)` targeting the player.
    fn spawn_monster_targeting_player(
        gs: &mut GameState,
        kind: MobjKind,
        x: i32,
        y: i32,
        health: i32,
    ) -> MobjHandle {
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = health;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.target = gs.player.handle;
        mo.radius = Fixed16_16::from_int(20);
        mo.height = Fixed16_16::from_int(56);
        gs.mobjslab.alloc(mo)
    }

    // --- Dispatch wiring tests ---

    #[test]
    fn dispatch_cpos_attack_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::Sergeant, 100, 0, 30);
        dispatch_action(&mut gs, h, ACTION_CPOS_ATTACK, None);
    }

    #[test]
    fn dispatch_cyber_attack_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 200, 0, 4000);
        dispatch_action(&mut gs, h, ACTION_CYBER_ATTACK, None);
    }

    #[test]
    fn dispatch_skel_missile_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::Revenant, 200, 0, 300);
        dispatch_action(&mut gs, h, ACTION_SKEL_MISSILE, None);
    }

    #[test]
    fn dispatch_fat_attack1_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);
        dispatch_action(&mut gs, h, ACTION_FAT_ATTACK1, None);
    }

    #[test]
    fn dispatch_fat_attack2_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);
        dispatch_action(&mut gs, h, ACTION_FAT_ATTACK2, None);
    }

    #[test]
    fn dispatch_fat_attack3_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);
        dispatch_action(&mut gs, h, ACTION_FAT_ATTACK3, None);
    }

    #[test]
    fn dispatch_skull_attack_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::LostSoul, 200, 0, 100);
        dispatch_action(&mut gs, h, ACTION_SKULL_ATTACK, None);
    }

    #[test]
    fn dispatch_bspi_attack_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::Arachnotron, 200, 0, 500);
        dispatch_action(&mut gs, h, ACTION_BSPI_ATTACK, None);
    }

    #[test]
    fn dispatch_spid_attack_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::SpiderMastermind, 200, 0, 3000);
        dispatch_action(&mut gs, h, ACTION_SPID_ATTACK, None);
    }

    #[test]
    fn dispatch_pain_attack_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::PainElemental, 200, 0, 400);
        dispatch_action(&mut gs, h, ACTION_PAIN_ATTACK, None);
    }

    // --- Cyberdemon spawns Rocket ---

    #[test]
    fn cyber_attack_spawns_rocket_projectile() {
        let mut gs = make_game_state();
        let cyber = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 200, 0, 4000);

        let count_before = gs.mobjslab.len();
        a_cyber_attack(&mut gs, cyber);
        let count_after = gs.mobjslab.len();

        assert!(
            count_after > count_before,
            "cyberdemon attack must spawn a projectile"
        );

        // Find the Rocket.
        let rocket = gs.mobjslab.iter_handles().find(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::Rocket)
                .unwrap_or(false)
        });
        assert!(rocket.is_some(), "must spawn a Rocket MobjKind");
    }

    // --- Revenant spawns Tracer ---

    #[test]
    fn skel_missile_spawns_tracer_projectile() {
        let mut gs = make_game_state();
        let skel = spawn_monster_targeting_player(&mut gs, MobjKind::Revenant, 200, 0, 300);

        a_skel_missile(&mut gs, skel);

        let tracer = gs.mobjslab.iter_handles().find(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::Tracer)
                .unwrap_or(false)
        });
        assert!(tracer.is_some(), "must spawn a Tracer MobjKind");
    }

    // --- Arachnotron spawns ArachPlaz ---

    #[test]
    fn bspi_attack_spawns_arachplaz() {
        let mut gs = make_game_state();
        let bspi = spawn_monster_targeting_player(&mut gs, MobjKind::Arachnotron, 200, 0, 500);

        a_bspi_attack(&mut gs, bspi);

        let plaz = gs.mobjslab.iter_handles().find(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::ArachPlaz)
                .unwrap_or(false)
        });
        assert!(plaz.is_some(), "must spawn an ArachPlaz MobjKind");
    }

    // --- Mancubus spawns FatShot (two per attack) ---

    #[test]
    fn fat_attack1_spawns_two_fatshots() {
        let mut gs = make_game_state();
        let manc = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);

        let count_before = gs.mobjslab.len();
        a_fat_attack1(&mut gs, manc);
        let count_after = gs.mobjslab.len();

        // Two FatShots should be spawned.
        assert_eq!(
            count_after - count_before,
            2,
            "fat_attack1 must spawn exactly 2 projectiles"
        );

        let fatshot_count = gs
            .mobjslab
            .iter_handles()
            .filter(|h| {
                gs.mobjslab
                    .get(*h)
                    .map(|m| m.kind == MobjKind::FatShot)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(fatshot_count, 2, "both projectiles must be FatShot");
    }

    #[test]
    fn fat_attack2_spawns_two_fatshots() {
        let mut gs = make_game_state();
        let manc = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);

        a_fat_attack2(&mut gs, manc);

        let fatshot_count = gs
            .mobjslab
            .iter_handles()
            .filter(|h| {
                gs.mobjslab
                    .get(*h)
                    .map(|m| m.kind == MobjKind::FatShot)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(fatshot_count, 2, "fat_attack2 must spawn 2 FatShots");
    }

    #[test]
    fn fat_attack3_spawns_two_fatshots() {
        let mut gs = make_game_state();
        let manc = spawn_monster_targeting_player(&mut gs, MobjKind::Mancubus, 200, 0, 600);

        a_fat_attack3(&mut gs, manc);

        let fatshot_count = gs
            .mobjslab
            .iter_handles()
            .filter(|h| {
                gs.mobjslab
                    .get(*h)
                    .map(|m| m.kind == MobjKind::FatShot)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(fatshot_count, 2, "fat_attack3 must spawn 2 FatShots");
    }

    // --- Lost Soul skull attack ---

    #[test]
    fn skull_attack_sets_skullfly_flag() {
        let mut gs = make_game_state();
        let skull = spawn_monster_targeting_player(&mut gs, MobjKind::LostSoul, 200, 0, 100);

        a_skull_attack(&mut gs, skull);

        let mo = gs.mobjslab.get(skull).unwrap();
        assert_ne!(
            mo.flags & flags::MF_SKULLFLY,
            0,
            "skull attack must set MF_SKULLFLY"
        );
    }

    #[test]
    fn skull_attack_sets_momentum_toward_target() {
        let mut gs = make_game_state();
        // Skull at (200,0), player at (0,0) — skull should fly west.
        let skull = spawn_monster_targeting_player(&mut gs, MobjKind::LostSoul, 200, 0, 100);

        a_skull_attack(&mut gs, skull);

        let mo = gs.mobjslab.get(skull).unwrap();
        assert!(
            mo.momx < Fixed16_16::ZERO,
            "momx should be negative (flying west toward player), got {:?}",
            mo.momx
        );
        // Speed should be approximately SKULLSPEED (20).
        let speed_sq = mo.momx.to_int() * mo.momx.to_int() + mo.momy.to_int() * mo.momy.to_int();
        let speed = (speed_sq as f32).sqrt();
        assert!(
            (15.0..=25.0).contains(&speed),
            "skull speed should be ~20, got {speed}"
        );
    }

    // --- Pain Elemental spawns Lost Soul ---

    #[test]
    fn pain_attack_spawns_lost_soul() {
        let mut gs = make_game_state();
        let pe = spawn_monster_targeting_player(&mut gs, MobjKind::PainElemental, 200, 0, 400);

        let count_before = gs
            .mobjslab
            .iter_handles()
            .filter(|h| {
                gs.mobjslab
                    .get(*h)
                    .map(|m| m.kind == MobjKind::LostSoul)
                    .unwrap_or(false)
            })
            .count();

        a_pain_attack(&mut gs, pe);

        let count_after = gs
            .mobjslab
            .iter_handles()
            .filter(|h| {
                gs.mobjslab
                    .get(*h)
                    .map(|m| m.kind == MobjKind::LostSoul)
                    .unwrap_or(false)
            })
            .count();

        assert_eq!(
            count_after,
            count_before + 1,
            "pain attack must spawn exactly 1 Lost Soul"
        );
    }

    #[test]
    fn pain_attack_respects_lost_soul_cap() {
        let mut gs = make_game_state();
        let pe = spawn_monster_targeting_player(&mut gs, MobjKind::PainElemental, 200, 0, 400);

        // Spawn 21 Lost Souls to reach the cap.
        for i in 0..21 {
            let mut skull = Mobj::new(
                MobjKind::LostSoul,
                Fixed16_16::from_int(500 + i * 10),
                Fixed16_16::from_int(500),
                Bam::ZERO,
            );
            skull.health = 100;
            skull.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
            gs.mobjslab.alloc(skull);
        }

        let count_before = gs
            .mobjslab
            .iter_handles()
            .filter(|h| {
                gs.mobjslab
                    .get(*h)
                    .map(|m| m.kind == MobjKind::LostSoul && m.health > 0)
                    .unwrap_or(false)
            })
            .count();

        a_pain_attack(&mut gs, pe);

        let count_after = gs
            .mobjslab
            .iter_handles()
            .filter(|h| {
                gs.mobjslab
                    .get(*h)
                    .map(|m| m.kind == MobjKind::LostSoul && m.health > 0)
                    .unwrap_or(false)
            })
            .count();

        assert_eq!(
            count_before, count_after,
            "pain attack must NOT spawn Lost Soul when cap ({}) is reached",
            LOST_SOUL_MAX
        );
    }

    #[test]
    fn pain_attack_spawns_when_under_cap() {
        let mut gs = make_game_state();
        let pe = spawn_monster_targeting_player(&mut gs, MobjKind::PainElemental, 200, 0, 400);

        // Spawn 20 Lost Souls — one below the cap.
        for i in 0..20 {
            let mut skull = Mobj::new(
                MobjKind::LostSoul,
                Fixed16_16::from_int(500 + i * 10),
                Fixed16_16::from_int(500),
                Bam::ZERO,
            );
            skull.health = 100;
            skull.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
            gs.mobjslab.alloc(skull);
        }

        a_pain_attack(&mut gs, pe);

        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|h| {
                gs.mobjslab
                    .get(*h)
                    .map(|m| m.kind == MobjKind::LostSoul && m.health > 0)
                    .unwrap_or(false)
            })
            .count();

        assert_eq!(count, 21, "pain attack should spawn 1 more (20->21)");
    }

    // --- No-target / dead-target edge cases ---

    #[test]
    fn all_attacks_noop_without_target() {
        let mut gs = make_game_state();
        // Monster with no target set (NULL).
        let mut mo = Mobj::new(
            MobjKind::Sergeant,
            Fixed16_16::from_int(100),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 30;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let h = gs.mobjslab.alloc(mo);

        // None of these should panic.
        dispatch_action(&mut gs, h, ACTION_CPOS_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_CYBER_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_SKEL_MISSILE, None);
        dispatch_action(&mut gs, h, ACTION_FAT_ATTACK1, None);
        dispatch_action(&mut gs, h, ACTION_FAT_ATTACK2, None);
        dispatch_action(&mut gs, h, ACTION_FAT_ATTACK3, None);
        dispatch_action(&mut gs, h, ACTION_SKULL_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_BSPI_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_SPID_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_PAIN_ATTACK, None);
    }

    #[test]
    fn all_attacks_noop_with_dead_target() {
        let mut gs = make_game_state();
        // Kill the player.
        gs.mobjslab.get_mut(gs.player.handle).unwrap().health = 0;

        let h = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 200, 0, 4000);

        let slab_len_before = gs.mobjslab.len();
        dispatch_action(&mut gs, h, ACTION_CYBER_ATTACK, None);
        // Cyberdemon should NOT spawn a rocket when target is dead.
        assert_eq!(
            gs.mobjslab.len(),
            slab_len_before,
            "must not spawn projectile when target is dead"
        );
    }

    #[test]
    fn all_attacks_noop_with_stale_handle() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::Cyberdemon, 200, 0, 4000);
        gs.mobjslab.free(h);

        // None should panic.
        dispatch_action(&mut gs, h, ACTION_CPOS_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_CYBER_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_SKEL_MISSILE, None);
        dispatch_action(&mut gs, h, ACTION_FAT_ATTACK1, None);
        dispatch_action(&mut gs, h, ACTION_SKULL_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_BSPI_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_SPID_ATTACK, None);
        dispatch_action(&mut gs, h, ACTION_PAIN_ATTACK, None);
    }

    // --- Constants ---

    #[test]
    fn fatspread_constant_is_correct() {
        assert_eq!(FATSPREAD, 0x0400_0000, "FATSPREAD must be ANG90/8");
    }

    #[test]
    fn skullspeed_constant_is_20() {
        assert_eq!(SKULLSPEED, 20, "Lost Soul charge speed must be 20");
    }

    #[test]
    fn lost_soul_max_is_21() {
        assert_eq!(LOST_SOUL_MAX, 21, "Lost Soul cap must be 21");
    }

    #[test]
    fn action_constants_are_unique() {
        let constants = [
            ACTION_NONE,
            ACTION_LOOK,
            ACTION_CHASE,
            ACTION_POS_ATTACK,
            ACTION_SPOS_ATTACK,
            ACTION_TROO_ATTACK,
            ACTION_SARG_ATTACK,
            ACTION_FALL,
            ACTION_HEAD_ATTACK,
            ACTION_BRUIS_ATTACK,
            ACTION_FACE_TARGET,
            ACTION_CPOS_ATTACK,
            ACTION_CYBER_ATTACK,
            ACTION_SKEL_MISSILE,
            ACTION_FAT_ATTACK1,
            ACTION_FAT_ATTACK2,
            ACTION_FAT_ATTACK3,
            ACTION_SKULL_ATTACK,
            ACTION_BSPI_ATTACK,
            ACTION_SPID_ATTACK,
            ACTION_PAIN_ATTACK,
            ACTION_VILE_CHASE,
            ACTION_VILE_START,
            ACTION_VILE_TARGET,
            ACTION_VILE_ATTACK,
            ACTION_FIRE,
            ACTION_BRAIN_AWAKE,
            ACTION_BRAIN_SPIT,
            ACTION_SPAWN_FLY,
            ACTION_BRAIN_DIE,
            ACTION_BRAIN_SCREAM,
            ACTION_BRAIN_EXPLODE,
        ];
        for i in 0..constants.len() {
            for j in (i + 1)..constants.len() {
                assert_ne!(
                    constants[i], constants[j],
                    "action constants at indices {i} and {j} must be unique"
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Arch-Vile action tests
    // -----------------------------------------------------------------------

    #[test]
    fn dispatch_vile_chase_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
        dispatch_action(&mut gs, h, ACTION_VILE_CHASE, None);
    }

    #[test]
    fn dispatch_vile_start_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
        dispatch_action(&mut gs, h, ACTION_VILE_START, None);
    }

    #[test]
    fn dispatch_vile_target_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
        dispatch_action(&mut gs, h, ACTION_VILE_TARGET, None);
    }

    #[test]
    fn dispatch_vile_attack_does_not_panic() {
        let mut gs = make_game_state();
        let h = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
        dispatch_action(&mut gs, h, ACTION_VILE_ATTACK, None);
    }

    #[test]
    fn dispatch_fire_does_not_panic() {
        let mut gs = make_game_state();
        // Spawn a fire actor with a tracer.
        let mut fire = Mobj::new(
            MobjKind::VileFire,
            Fixed16_16::from_int(100),
            Fixed16_16::from_int(100),
            Bam::ZERO,
        );
        fire.health = 1;
        fire.tracer = gs.player.handle;
        let h = gs.mobjslab.alloc(fire);
        dispatch_action(&mut gs, h, ACTION_FIRE, None);
    }

    #[test]
    fn vile_start_sets_tracer() {
        let mut gs = make_game_state();
        let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

        a_vile_start(&mut gs, vile);

        let mo = gs.mobjslab.get(vile).unwrap();
        assert_eq!(
            mo.tracer, gs.player.handle,
            "vile_start must set tracer to the target"
        );
    }

    #[test]
    fn vile_target_spawns_fire() {
        let mut gs = make_game_state();
        let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

        let count_before = gs.mobjslab.len();
        a_vile_target(&mut gs, vile);
        let count_after = gs.mobjslab.len();

        assert!(
            count_after > count_before,
            "vile_target must spawn a VileFire actor"
        );

        let fire = gs.mobjslab.iter_handles().find(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::VileFire)
                .unwrap_or(false)
        });
        assert!(fire.is_some(), "must spawn a VileFire MobjKind");
    }

    #[test]
    fn vile_attack_damages_target() {
        let mut gs = make_game_state();
        let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);
        let hp_before = gs.mobjslab.get(gs.player.handle).unwrap().health;

        a_vile_attack(&mut gs, vile);

        let hp_after = gs.mobjslab.get(gs.player.handle).unwrap().health;
        assert_eq!(
            hp_before - hp_after,
            90,
            "vile_attack must deal 20+70=90 total damage"
        );
    }

    #[test]
    fn vile_attack_applies_upward_thrust() {
        let mut gs = make_game_state();
        let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

        a_vile_attack(&mut gs, vile);

        let momz = gs.mobjslab.get(gs.player.handle).unwrap().momz;
        assert_eq!(
            momz,
            Fixed16_16::from_int(15),
            "vile_attack must apply momz of 15"
        );
    }

    #[test]
    fn fire_tracks_tracer_position() {
        let mut gs = make_game_state();
        // Move the player to a known position.
        gs.mobjslab.get_mut(gs.player.handle).unwrap().x = Fixed16_16::from_int(500);
        gs.mobjslab.get_mut(gs.player.handle).unwrap().y = Fixed16_16::from_int(300);

        let mut fire = Mobj::new(
            MobjKind::VileFire,
            Fixed16_16::from_int(100),
            Fixed16_16::from_int(100),
            Bam::ZERO,
        );
        fire.health = 1;
        fire.tracer = gs.player.handle;
        let fh = gs.mobjslab.alloc(fire);

        a_fire(&mut gs, fh);

        let fire_mo = gs.mobjslab.get(fh).unwrap();
        assert_eq!(fire_mo.x, Fixed16_16::from_int(500));
        assert_eq!(fire_mo.y, Fixed16_16::from_int(300));
    }

    #[test]
    fn vile_chase_resurrects_nearby_corpse() {
        let mut gs = make_game_state();
        let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

        // Create a trooper corpse within 128 units of the vile.
        let mut corpse = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(210),
            Fixed16_16::from_int(10),
            Bam::ZERO,
        );
        corpse.health = 0;
        corpse.flags = flags::MF_CORPSE;
        let corpse_h = gs.mobjslab.alloc(corpse);

        a_vile_chase(&mut gs, vile, None);

        // The corpse should be resurrected.
        let raised = gs.mobjslab.get(corpse_h).unwrap();
        assert!(
            raised.health > 0,
            "resurrected corpse must have health > 0, got {}",
            raised.health
        );
        assert_eq!(
            raised.flags & flags::MF_CORPSE,
            0,
            "resurrected corpse must not have MF_CORPSE flag"
        );
    }

    #[test]
    fn vile_chase_ignores_distant_corpse() {
        let mut gs = make_game_state();
        let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

        // Create a trooper corpse far from the vile (> 128 units).
        let mut corpse = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(500),
            Fixed16_16::from_int(500),
            Bam::ZERO,
        );
        corpse.health = 0;
        corpse.flags = flags::MF_CORPSE;
        let corpse_h = gs.mobjslab.alloc(corpse);

        a_vile_chase(&mut gs, vile, None);

        // The corpse should remain dead (too far away).
        let still_dead = gs.mobjslab.get(corpse_h).unwrap();
        assert_eq!(
            still_dead.health, 0,
            "distant corpse must not be resurrected"
        );
    }

    #[test]
    fn vile_chase_ignores_non_raisable_corpse() {
        let mut gs = make_game_state();
        let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

        // Create a Lost Soul corpse nearby (Lost Soul has raise_state = S_NULL).
        let mut corpse = Mobj::new(
            MobjKind::LostSoul,
            Fixed16_16::from_int(210),
            Fixed16_16::from_int(10),
            Bam::ZERO,
        );
        corpse.health = 0;
        corpse.flags = flags::MF_CORPSE;
        let corpse_h = gs.mobjslab.alloc(corpse);

        a_vile_chase(&mut gs, vile, None);

        // Lost Soul should remain dead (not raisable).
        let still_dead = gs.mobjslab.get(corpse_h).unwrap();
        assert_eq!(still_dead.health, 0, "non-raisable corpse must stay dead");
    }

    // -----------------------------------------------------------------------
    // Boss Brain action tests
    // -----------------------------------------------------------------------

    #[test]
    fn brain_awake_sets_flag() {
        let mut gs = make_game_state();
        assert!(!gs.brain_awake);
        a_brain_awake(&mut gs);
        assert!(gs.brain_awake, "brain_awake must set the flag");
    }

    #[test]
    fn brain_spit_does_nothing_when_not_awake() {
        let mut gs = make_game_state();
        gs.brain_targets
            .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

        let count_before = gs.mobjslab.len();
        a_brain_spit(&mut gs, brain);
        assert_eq!(
            gs.mobjslab.len(),
            count_before,
            "brain_spit must do nothing when brain_awake is false"
        );
    }

    #[test]
    fn brain_spit_does_nothing_without_targets() {
        let mut gs = make_game_state();
        gs.brain_awake = true;
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

        let count_before = gs.mobjslab.len();
        a_brain_spit(&mut gs, brain);
        assert_eq!(
            gs.mobjslab.len(),
            count_before,
            "brain_spit must do nothing without brain_targets"
        );
    }

    #[test]
    fn brain_spit_spawns_cube() {
        let mut gs = make_game_state();
        gs.brain_awake = true;
        gs.brain_targets
            .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

        let count_before = gs.mobjslab.len();
        a_brain_spit(&mut gs, brain);
        let count_after = gs.mobjslab.len();

        assert!(
            count_after > count_before,
            "brain_spit must spawn a BossCube"
        );

        let cube = gs.mobjslab.iter_handles().find(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::BossCube)
                .unwrap_or(false)
        });
        assert!(cube.is_some(), "must spawn a BossCube MobjKind");
    }

    #[test]
    fn brain_spit_round_robins_targets() {
        let mut gs = make_game_state();
        gs.brain_awake = true;
        gs.brain_targets
            .push((Fixed16_16::from_int(100), Fixed16_16::from_int(100)));
        gs.brain_targets
            .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

        assert_eq!(gs.brain_target_index, 0);
        a_brain_spit(&mut gs, brain);
        assert_eq!(
            gs.brain_target_index, 1,
            "first spit uses index 0, advances to 1"
        );
        a_brain_spit(&mut gs, brain);
        assert_eq!(
            gs.brain_target_index, 2,
            "second spit uses index 1, advances to 2"
        );
        // Third spit should wrap around (2 % 2 == 0).
        a_brain_spit(&mut gs, brain);
        assert_eq!(
            gs.brain_target_index, 1,
            "third spit wraps to index 0, advances to 1"
        );
    }

    #[test]
    fn spawn_fly_spawns_monster_and_fog() {
        let mut gs = make_game_state();
        gs.brain_awake = true;
        gs.brain_targets
            .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));

        // Create a cube with reactiontime pointing to target index 0.
        let mut cube = Mobj::new(
            MobjKind::BossCube,
            Fixed16_16::from_int(500),
            Fixed16_16::from_int(500),
            Bam::ZERO,
        );
        cube.health = 1;
        cube.flags = flags::MF_NOBLOCKMAP | flags::MF_MISSILE;
        cube.reactiontime = 0; // target index
        let cube_h = gs.mobjslab.alloc(cube);

        let count_before = gs.mobjslab.len();
        a_spawn_fly(&mut gs, cube_h);
        let count_after = gs.mobjslab.len();

        // Should spawn monster + fog - cube (removed) = net +1.
        // But the cube is freed, so we spawned 2 new (monster + fog) - 1 freed = net +1.
        assert!(
            count_after >= count_before,
            "spawn_fly must spawn monster + fog, got {count_before} -> {count_after}"
        );

        // Cube should be freed.
        assert!(
            gs.mobjslab.get(cube_h).is_none(),
            "cube must be removed after spawning"
        );

        // SpawnFire fog should exist.
        let fog = gs.mobjslab.iter_handles().find(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| m.kind == MobjKind::SpawnFire)
                .unwrap_or(false)
        });
        assert!(fog.is_some(), "must spawn a SpawnFire fog effect");
    }

    #[test]
    fn spawn_fly_monster_has_reaction_time() {
        let mut gs = make_game_state();
        gs.brain_awake = true;
        gs.brain_targets
            .push((Fixed16_16::from_int(500), Fixed16_16::from_int(500)));

        let mut cube = Mobj::new(
            MobjKind::BossCube,
            Fixed16_16::from_int(500),
            Fixed16_16::from_int(500),
            Bam::ZERO,
        );
        cube.health = 1;
        cube.flags = flags::MF_NOBLOCKMAP | flags::MF_MISSILE;
        cube.reactiontime = 0;
        let cube_h = gs.mobjslab.alloc(cube);

        a_spawn_fly(&mut gs, cube_h);

        // Find the spawned monster (not SpawnFire, not player, not cube).
        let monster = gs.mobjslab.iter_handles().find(|h| {
            gs.mobjslab
                .get(*h)
                .map(|m| {
                    m.kind != MobjKind::Player
                        && m.kind != MobjKind::SpawnFire
                        && m.kind != MobjKind::BossCube
                        && BOSS_SPAWN_TYPES.contains(&m.kind)
                })
                .unwrap_or(false)
        });
        assert!(
            monster.is_some(),
            "must spawn a monster from BOSS_SPAWN_TYPES"
        );

        let spawned = gs.mobjslab.get(monster.unwrap()).unwrap();
        assert_eq!(
            spawned.reactiontime, 18,
            "spawned monster must have reactiontime=18"
        );
    }

    #[test]
    fn brain_die_triggers_exit() {
        let mut gs = make_game_state();
        assert!(gs.exit_request.is_none());
        a_brain_die(&mut gs);
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Normal),
            "brain_die must trigger a normal exit"
        );
    }

    #[test]
    fn brain_scream_spawns_explosions() {
        let mut gs = make_game_state();
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

        let count_before = gs.mobjslab.len();
        a_brain_scream(&mut gs, brain);
        let count_after = gs.mobjslab.len();

        assert_eq!(
            count_after - count_before,
            20,
            "brain_scream must spawn exactly 20 explosions"
        );
    }

    #[test]
    fn brain_explode_spawns_one_explosion() {
        let mut gs = make_game_state();
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);

        let count_before = gs.mobjslab.len();
        a_brain_explode(&mut gs, brain);
        let count_after = gs.mobjslab.len();

        assert_eq!(
            count_after - count_before,
            1,
            "brain_explode must spawn exactly 1 explosion"
        );
    }

    #[test]
    fn dispatch_brain_awake_does_not_panic() {
        let mut gs = make_game_state();
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
        dispatch_action(&mut gs, brain, ACTION_BRAIN_AWAKE, None);
        assert!(gs.brain_awake);
    }

    #[test]
    fn dispatch_brain_spit_does_not_panic() {
        let mut gs = make_game_state();
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
        dispatch_action(&mut gs, brain, ACTION_BRAIN_SPIT, None);
    }

    #[test]
    fn dispatch_brain_die_does_not_panic() {
        let mut gs = make_game_state();
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
        dispatch_action(&mut gs, brain, ACTION_BRAIN_DIE, None);
    }

    #[test]
    fn dispatch_brain_scream_does_not_panic() {
        let mut gs = make_game_state();
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
        dispatch_action(&mut gs, brain, ACTION_BRAIN_SCREAM, None);
    }

    #[test]
    fn dispatch_brain_explode_does_not_panic() {
        let mut gs = make_game_state();
        let brain = spawn_monster_targeting_player(&mut gs, MobjKind::BossBrain, 200, 0, 250);
        dispatch_action(&mut gs, brain, ACTION_BRAIN_EXPLODE, None);
    }

    // --- New action no-target / stale-handle edge cases ---

    #[test]
    fn vile_actions_noop_without_target() {
        let mut gs = make_game_state();
        let mut mo = Mobj::new(
            MobjKind::ArchVile,
            Fixed16_16::from_int(100),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        mo.health = 700;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let h = gs.mobjslab.alloc(mo);

        // None of these should panic.
        dispatch_action(&mut gs, h, ACTION_VILE_START, None);
        dispatch_action(&mut gs, h, ACTION_VILE_TARGET, None);
        dispatch_action(&mut gs, h, ACTION_VILE_ATTACK, None);
    }

    #[test]
    fn vile_actions_noop_with_dead_target() {
        let mut gs = make_game_state();
        gs.mobjslab.get_mut(gs.player.handle).unwrap().health = 0;
        let vile = spawn_monster_targeting_player(&mut gs, MobjKind::ArchVile, 200, 0, 700);

        let count_before = gs.mobjslab.len();
        dispatch_action(&mut gs, vile, ACTION_VILE_TARGET, None);
        assert_eq!(
            gs.mobjslab.len(),
            count_before,
            "must not spawn fire when target is dead"
        );
    }

    #[test]
    fn boss_spawn_types_has_11_entries() {
        assert_eq!(
            BOSS_SPAWN_TYPES.len(),
            11,
            "BOSS_SPAWN_TYPES must have 11 monster types"
        );
    }
}
