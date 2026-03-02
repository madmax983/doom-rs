//! Monster action functions — dispatched from the thinker loop.
//!
//! Action functions fire when an actor *enters* a new state (when `tics`
//! reaches zero and the state machine transitions to `next_state`).
//!
//! # Implemented (Batch 2)
//! - `A_Look` (index 1): scan for the player; transition to `see_state`.
//! - `A_Chase` (index 2): move toward target; use 8-direction grid movement.
//! - `A_FaceTarget` (index 3): snap angle to face the current target.
//!
//! # Implemented (Batch 5)
//! - `A_PosAttack`  (index 3): Trooper hitscan attack.
//! - `A_SPosAttack` (index 4): Sergeant 3-pellet shotgun burst.
//! - `A_TroopAttack`(index 5): Imp — melee if close, else hitscan.
//! - `A_SargAttack` (index 6): Demon melee-only attack.
//! - `A_Fall`       (index 7): clear MF_SOLID/MF_COUNTKILL on death.
//!
//! # Implemented (Batch 3)
//! - `p_new_chase_dir`: full 4-candidate direction selection with fallback.
//! - `p_check_sight`: REJECT-table-based coarse line-of-sight check.

use doom_map::Level;
use doom_types::{Bam, Fixed16_16};

use crate::mobj::MobjHandle;
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

// ---------------------------------------------------------------------------
// Public dispatcher
// ---------------------------------------------------------------------------

/// Dispatch the action function with index `action` for actor `handle`.
///
/// Called by `GameState::advance_mobj_state` each time an actor enters a
/// new state.
pub fn dispatch_action(
    gs: &mut GameState,
    handle: MobjHandle,
    action: u8,
    level: Option<&Level>,
) {
    match action {
        ACTION_NONE         => {}
        ACTION_LOOK         => a_look(gs, handle),
        ACTION_CHASE        => a_chase(gs, handle, level),
        ACTION_POS_ATTACK   => a_pos_attack(gs, handle, level),
        ACTION_SPOS_ATTACK  => a_spos_attack(gs, handle, level),
        ACTION_TROO_ATTACK  => a_troo_attack(gs, handle, level),
        ACTION_SARG_ATTACK  => a_sarg_attack(gs, handle),
        ACTION_FALL         => a_fall(gs, handle),
        _                   => {}
    }
}

// ---------------------------------------------------------------------------
// 8-direction constants and movement table
// ---------------------------------------------------------------------------

/// Direction constants matching Doom's `dirtype_t`.
pub const DI_EAST:      u8 = 0;
pub const DI_NORTHEAST: u8 = 1;
pub const DI_NORTH:     u8 = 2;
pub const DI_NORTHWEST: u8 = 3;
pub const DI_WEST:      u8 = 4;
pub const DI_SOUTHWEST: u8 = 5;
pub const DI_SOUTH:     u8 = 6;
pub const DI_SOUTHEAST: u8 = 7;
pub const DI_NODIR:     u8 = 8;

/// Unit movement vectors for the 8-way grid (Doom `DI_*` directions).
///
/// Index: 0=East, 1=NE, 2=North, 3=NW, 4=West, 5=SW, 6=South, 7=SE.
/// Values ≈ `FRACUNIT * cos/sin(n * 45°)`.  Diagonal uses Doom's historical
/// constant 47000 ≈ 65536 * sin(45°).
const XMOVE: [Fixed16_16; 9] = [
    Fixed16_16( 65536), // East
    Fixed16_16( 47000), // NE
    Fixed16_16(     0), // North
    Fixed16_16(-47000), // NW
    Fixed16_16(-65536), // West
    Fixed16_16(-47000), // SW
    Fixed16_16(     0), // South
    Fixed16_16( 47000), // SE
    Fixed16_16(     0), // NODIR
];

const YMOVE: [Fixed16_16; 9] = [
    Fixed16_16(     0), // East
    Fixed16_16( 47000), // NE
    Fixed16_16( 65536), // North
    Fixed16_16( 47000), // NW
    Fixed16_16(     0), // West
    Fixed16_16(-47000), // SW
    Fixed16_16(-65536), // South
    Fixed16_16(-47000), // SE
    Fixed16_16(     0), // NODIR
];

/// Choose the best 8-way direction given a `(dx, dy)` displacement.
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
            (true,  true)  => DI_NORTHEAST,
            (false, true)  => DI_NORTHWEST,
            (false, false) => DI_SOUTHWEST,
            (true,  false) => DI_SOUTHEAST,
        }
    }
}

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
fn p_check_sight(
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

    // REJECT-table culling: look up the sector indices from subsectors.
    if let Some(lv) = level {
        let src_sector = sector_from_subsector(lv, src_subsector);
        let tgt_sector = sector_from_subsector(lv, tgt_subsector);
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

/// Resolve the sector index for the given subsector index.
///
/// Path: ssectors[sub] → first seg → linedef → right sidedef → sector.
/// Returns `None` if any index is out of range.
fn sector_from_subsector(level: &Level, subsector: usize) -> Option<usize> {
    let ss = level.ssectors.get(subsector)?;
    let seg = level.segs.get(ss.first_seg as usize)?;
    let ld = level.linedefs.get(seg.linedef as usize)?;
    let sd_idx = if seg.direction == 0 {
        ld.right_sidedef
    } else {
        ld.left_sidedef
    };
    let sd = level.sidedefs.get(sd_idx as usize)?;
    Some(sd.sector as usize)
}

// ---------------------------------------------------------------------------
// A_Look
// ---------------------------------------------------------------------------

/// Port of `A_Look` from Doom's `p_enemy.c`.
///
/// If the monster is not in its threshold period and the player is within
/// line-of-sight (P_CheckSight), transition to `see_state`.
fn a_look(gs: &mut GameState, handle: MobjHandle) {
    let player_handle = gs.player.handle;

    // Decrement threshold counter while the monster is still "alerted."
    {
        let Some(mo) = gs.mobjslab.get_mut(handle) else { return };
        if mo.threshold > 0 {
            mo.threshold -= 1;
            return;
        }
    }

    // Grab the monster's kind.
    let mo_kind = match gs.mobjslab.get(handle) {
        Some(mo) => mo.kind,
        None => return,
    };

    // Check the player exists and is alive.
    let player_alive = match gs.mobjslab.get(player_handle) {
        Some(p) => !p.is_dead(),
        _ => false,
    };
    if !player_alive {
        return;
    }

    // Line-of-sight check (uses REJECT table when level is available).
    // A_Look is called without a level reference (see dispatch_action).
    // We pass None here; the REJECT path is used by A_Chase which has level.
    if !p_check_sight(gs, handle, player_handle, None) {
        return;
    }

    // Resolve see_state from mobjinfo.
    let see_sn = match mobjinfo::MOBJINFO.get(mo_kind as usize) {
        Some(info) if info.see_state.0 != 0 => info.see_state,
        _ => return,
    };
    let new_tics = match states::STATES.get(see_sn.0 as usize) {
        Some(e) => e.tics,
        None => return,
    };

    // Transition monster to see_state.
    let Some(mo) = gs.mobjslab.get_mut(handle) else { return };
    mo.target    = player_handle;
    mo.threshold = 60; // stay alerted for 60 tics
    mo.state     = see_sn;
    mo.tics      = new_tics;
    // The action for see_state fires naturally when tics next reaches zero.
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
    let new_x  = mo_x + step_x;
    let new_y  = mo_y + step_y;

    let can_move = match level {
        Some(lv) => crate::movement::p_try_move(&gs.mobjslab, handle, new_x, new_y, lv),
        None     => true,
    };

    if can_move {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.x       = new_x;
            mo.y       = new_y;
            mo.momx    = step_x;
            mo.momy    = step_y;
            mo.movedir = dir;
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
pub fn p_new_chase_dir(
    gs: &mut GameState,
    handle: MobjHandle,
    level: Option<&Level>,
) {
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
    let d_x: u8 = if dx > 0 { DI_EAST  } else if dx < 0 { DI_WEST  } else { DI_NODIR };
    let d_y: u8 = if dy > 0 { DI_NORTH } else if dy < 0 { DI_SOUTH } else { DI_NODIR };

    // Primary candidate: diagonal combining both directions.
    let diag: u8 = if d_x == DI_NODIR || d_y == DI_NODIR {
        // No diagonal possible — use whichever pure direction exists.
        if d_x != DI_NODIR { d_x } else { d_y }
    } else {
        // Choose the diagonal that matches (d_x, d_y).
        match (d_x, d_y) {
            (DI_EAST,  DI_NORTH) => DI_NORTHEAST,
            (DI_EAST,  DI_SOUTH) => DI_SOUTHEAST,
            (DI_WEST,  DI_NORTH) => DI_NORTHWEST,
            (DI_WEST,  DI_SOUTH) => DI_SOUTHWEST,
            _ => DI_NODIR,
        }
    };

    // Secondary/tertiary: prefer the axis with larger absolute displacement.
    let (sec, tert) = if dx.abs() > dy.abs() {
        (d_x, d_y)   // larger x → prefer x-dir, then y-dir
    } else {
        (d_y, d_x)   // larger y → prefer y-dir, then x-dir
    };

    // Try candidates in order: primary → secondary → tertiary → nodir.
    let candidates = [diag, sec, tert, DI_NODIR];
    for &cand in &candidates {
        if cand == DI_NODIR {
            break;
        }
        if try_move_in_dir(gs, handle, cand, speed, level) {
            // Reset movecount so monster won't re-evaluate direction for a while.
            let rng_val = gs.rng.next() as i32;
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.movecount = 8 + (rng_val & 7);
            }
            return;
        }
    }

    // All preferred directions blocked: try any direction round-robin.
    // Cycle through all 8 directions starting from a random offset.
    let start_dir = gs.rng.next() % 8;
    for i in 0u8..8 {
        let dir = (start_dir + i) % 8;
        if try_move_in_dir(gs, handle, dir, speed, level) {
            let rng_val = gs.rng.next() as i32;
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.movecount = 4 + (rng_val & 3);
            }
            return;
        }
    }

    // Truly stuck: set NODIR.
    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.movedir   = DI_NODIR;
        mo.movecount = 0;
    }
}

// ---------------------------------------------------------------------------
// A_Chase
// ---------------------------------------------------------------------------

/// Port of `A_Chase` from Doom's `p_enemy.c`.
///
/// Moves the monster one step toward its target using full `P_NewChaseDir`
/// 4-candidate direction selection with fallback.  `movecount` tracks how
/// long the monster continues in its current direction before re-evaluating.
fn a_chase(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    // Grab everything we need from the monster before any borrow conflict.
    let (target_handle, mo_kind, movecount) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.target, mo.kind, mo.movecount),
        None => return,
    };

    // Check target still exists and is alive.
    let target_alive = match gs.mobjslab.get(target_handle) {
        Some(t) => !t.is_dead(),
        None => false,
    };

    if !target_alive {
        // Revert to idle spawn state.
        let spawn_sn = mobjinfo::MOBJINFO
            .get(mo_kind as usize)
            .map(|i| i.spawn_state)
            .unwrap_or_default();
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.target = MobjHandle::NULL;
            mo.state  = spawn_sn;
            if let Some(e) = states::STATES.get(spawn_sn.0 as usize) {
                mo.tics = e.tics;
            }
        }
        return;
    }

    // Decrement movecount each tic.
    {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.movecount -= 1;
        }
    }

    // Re-choose direction if movecount expired or we need to recompute.
    let need_new_dir = movecount <= 0;
    if need_new_dir {
        p_new_chase_dir(gs, handle, level);
    } else {
        // Try to keep moving in the current direction.
        let (mo_x, mo_y, cur_dir, speed) = match gs.mobjslab.get(handle) {
            Some(mo) => {
                let spd = mobjinfo::MOBJINFO
                    .get(mo.kind as usize)
                    .map(|i| i.speed)
                    .unwrap_or(Fixed16_16::ZERO);
                (mo.x, mo.y, mo.movedir, spd)
            }
            None => return,
        };

        if cur_dir != DI_NODIR {
            let step_x = XMOVE[cur_dir as usize].fixed_mul(speed);
            let step_y = YMOVE[cur_dir as usize].fixed_mul(speed);
            let new_x  = mo_x + step_x;
            let new_y  = mo_y + step_y;

            let can_move = match level {
                Some(lv) => crate::movement::p_try_move(&gs.mobjslab, handle, new_x, new_y, lv),
                None     => true,
            };

            if can_move {
                if let Some(mo) = gs.mobjslab.get_mut(handle) {
                    mo.x    = new_x;
                    mo.y    = new_y;
                    mo.momx = step_x;
                    mo.momy = step_y;
                }
            } else {
                // Blocked: immediately choose a new direction.
                p_new_chase_dir(gs, handle, level);
            }
        }
    }

    // Face the target.
    a_face_target(gs, handle);

    // Check if we should transition to an attack state.
    let target_handle2 = match gs.mobjslab.get(handle) {
        Some(mo) => mo.target,
        None => return,
    };
    let (melee_sn, missile_sn) = {
        let Some(mo) = gs.mobjslab.get(handle) else { return };
        let info = &mobjinfo::MOBJINFO[mo.kind as usize];
        (info.melee_state, info.missile_state)
    };
    let (mo_x2, mo_y2) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x, mo.y),
        None => return,
    };
    let (tx2, ty2) = match gs.mobjslab.get(target_handle2) {
        Some(t) => (t.x, t.y),
        None => return,
    };
    let dist = (tx2 - mo_x2).to_int().abs() + (ty2 - mo_y2).to_int().abs();

    // Melee: enter melee state if within MELEERANGE and melee_state != S_NULL.
    if melee_sn != crate::mobj::StateNum::NULL
        && dist <= crate::combat::MELEERANGE.to_int()
    {
        if let Some(entry) = states::STATES.get(melee_sn.0 as usize) {
            let new_tics = entry.tics;
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.state = melee_sn;
                mo.tics  = new_tics;
            }
        }
        return;
    }

    // Ranged: enter missile state if within MISSILERANGE and missile_state != S_NULL.
    if missile_sn != crate::mobj::StateNum::NULL
        && dist <= crate::combat::MISSILERANGE.to_int()
    {
        if let Some(entry) = states::STATES.get(missile_sn.0 as usize) {
            let new_tics = entry.tics;
            if let Some(mo) = gs.mobjslab.get_mut(handle) {
                mo.state = missile_sn;
                mo.tics  = new_tics;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A_FaceTarget
// ---------------------------------------------------------------------------

/// Snap the monster's angle to face its current target.
///
/// Uses the 8-way direction grid for deterministic, float-free computation.
/// Full smooth turn interpolation is Batch 3.
fn a_face_target(gs: &mut GameState, handle: MobjHandle) {
    let (target_handle, mo_x, mo_y) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.target, mo.x, mo.y),
        None => return,
    };
    let (tx, ty) = match gs.mobjslab.get(target_handle) {
        Some(t) => (t.x, t.y),
        None => return,
    };

    let dx  = (tx - mo_x).to_int();
    let dy  = (ty - mo_y).to_int();
    let dir = dir_to_target(dx, dy);

    // Convert 8-way direction index to Bam: each step is 45° = ANG45.
    let angle = Bam(dir as u32 * 0x2000_0000);

    if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.angle = angle;
    }
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
// A_PosAttack (Trooper hitscan)
// ---------------------------------------------------------------------------

/// Port of `A_PosAttack` from Doom's `p_enemy.c`.
///
/// Fires a single hitscan bolt at the current target.
fn a_pos_attack(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    // Check target exists and is alive.
    let target = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => mo.target,
        _ => return,
    };
    if gs.mobjslab.get(target).map(|t| t.is_dead()).unwrap_or(true) {
        return;
    }

    // Face the target, then read the resulting angle.
    a_face_target(gs, handle);
    let angle = match gs.mobjslab.get(handle) {
        Some(mo) => mo.angle,
        None => return,
    };

    let damage = ((gs.tic_num % 8) + 1) as i32 * 3;
    crate::combat::p_line_attack(gs, handle, angle, crate::combat::MISSILERANGE, damage, level);
}

// ---------------------------------------------------------------------------
// A_SPosAttack (Sergeant — 3-pellet shotgun burst)
// ---------------------------------------------------------------------------

/// Port of `A_SPosAttack` from Doom's `p_enemy.c`.
///
/// Fires 3 hitscan pellets with a small angular spread centered on the target.
fn a_spos_attack(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    let target = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => mo.target,
        _ => return,
    };
    if gs.mobjslab.get(target).map(|t| t.is_dead()).unwrap_or(true) {
        return;
    }

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
            gs, handle, shot_angle, crate::combat::MISSILERANGE, damage, level,
        );
    }
}

// ---------------------------------------------------------------------------
// A_TroopAttack (Imp — melee if in range, else hitscan)
// ---------------------------------------------------------------------------

/// Port of `A_TroopAttack` from Doom's `p_enemy.c`.
///
/// Uses melee if the target is within `MELEERANGE`, otherwise hitscan.
fn a_troo_attack(gs: &mut GameState, handle: MobjHandle, level: Option<&Level>) {
    let (target, mo_x, mo_y) = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => (mo.target, mo.x, mo.y),
        _ => return,
    };
    if gs.mobjslab.get(target).map(|t| t.is_dead()).unwrap_or(true) {
        return;
    }

    a_face_target(gs, handle);
    let angle = match gs.mobjslab.get(handle) {
        Some(mo) => mo.angle,
        None => return,
    };

    let (tx, ty) = match gs.mobjslab.get(target) {
        Some(t) => (t.x, t.y),
        None => return,
    };

    let dist = (tx - mo_x).to_int().abs() + (ty - mo_y).to_int().abs();
    let damage = ((gs.tic_num % 8) + 1) as i32 * 3;

    if dist <= crate::combat::MELEERANGE.to_int() {
        crate::combat::damage_mobj(gs, target, handle, damage);
    } else {
        crate::combat::p_line_attack(
            gs, handle, angle, crate::combat::MISSILERANGE, damage, level,
        );
    }
}

// ---------------------------------------------------------------------------
// A_SargAttack (Demon — melee only)
// ---------------------------------------------------------------------------

/// Port of `A_SargAttack` from Doom's `p_enemy.c`.
///
/// Deals melee damage only if the target is within `MELEERANGE`.
fn a_sarg_attack(gs: &mut GameState, handle: MobjHandle) {
    let (target, mo_x, mo_y) = match gs.mobjslab.get(handle) {
        Some(mo) if mo.target != MobjHandle::NULL => (mo.target, mo.x, mo.y),
        _ => return,
    };
    if gs.mobjslab.get(target).map(|t| t.is_dead()).unwrap_or(true) {
        return;
    }

    let (tx, ty) = match gs.mobjslab.get(target) {
        Some(t) => (t.x, t.y),
        None => return,
    };

    let dist = (tx - mo_x).to_int().abs() + (ty - mo_y).to_int().abs();
    if dist <= crate::combat::MELEERANGE.to_int() {
        let damage = ((gs.tic_num % 3) + 1) as i32 * 4;
        crate::combat::damage_mobj(gs, target, handle, damage);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind, flags};
    use crate::player::PlayerState;
    use crate::state::GameState;
    use crate::tic::TicCmd;
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
        mo.flags  = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let handle = gs.mobjslab.alloc(mo);
        gs.player  = PlayerState::pistol_start(handle);
        gs
    }

    fn spawn_trooper(gs: &mut GameState, x: i32, y: i32) -> MobjHandle {
        use crate::mobjinfo::MOBJINFO;
        use crate::states::STATES;
        use crate::mobj::MobjKind;
        let kind = MobjKind::Trooper;
        let spawn_sn = MOBJINFO[kind as usize].spawn_state;
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 20;
        mo.flags  = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state  = spawn_sn;
        mo.tics   = STATES[spawn_sn.0 as usize].tics;
        gs.mobjslab.alloc(mo)
    }

    #[test]
    fn dir_to_target_east() {
        assert_eq!(dir_to_target(100, 0), 0); // East
    }

    #[test]
    fn dir_to_target_north() {
        assert_eq!(dir_to_target(0, 100), 2); // North
    }

    #[test]
    fn dir_to_target_ne_diagonal() {
        assert_eq!(dir_to_target(50, 50), 1); // NE
    }

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
        // Should still be in spawn state (idle).
        assert_eq!(mo.state, spawn_sn, "trooper must stay idle when player is out of range");
    }

    #[test]
    fn a_chase_moves_monster_toward_player() {
        let mut gs = make_game_state();
        // Player at (0,0), trooper at (100,0).
        let trooper = spawn_trooper(&mut gs, 100, 0);

        // Tick enough times to trigger A_Look (10 tics) then A_Chase (4 tics).
        for _ in 0..15 {
            gs.tick(TicCmd::default(), None);
        }

        let mo = gs.mobjslab.get(trooper).unwrap();
        // Trooper should have moved west (toward x=0).
        assert!(
            mo.x < Fixed16_16::from_int(100),
            "trooper x={:?} must decrease toward player", mo.x
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

    // -----------------------------------------------------------------------
    // Batch 5 attack tests
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
        use crate::states::{ids, STATES};
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
        mo.health  = health;
        mo.flags   = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state   = fallback_sn;
        mo.tics    = STATES[fallback_sn.0 as usize].tics;
        mo.target  = gs.player.handle;
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
        assert!(found_atk, "trooper must enter an ATK state when player is in range");
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
        assert_eq!(
            mo.flags & flags::MF_SOLID,
            0,
            "a_fall must clear MF_SOLID"
        );
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
    fn spawn_monster_chasing_at(
        gs: &mut GameState,
        x: i32,
        y: i32,
    ) -> MobjHandle {
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
        mo.health    = 20;
        mo.flags     = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        mo.state     = see_sn;
        mo.tics      = STATES[see_sn.0 as usize].tics;
        mo.target    = MobjHandle::NULL;
        mo.threshold = 60;
        mo.movedir   = super::DI_NODIR;
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
        assert_ne!(mo.movedir, super::DI_NODIR, "movedir must be set after p_new_chase_dir");
        assert!(mo.movecount > 0, "movecount must be positive after p_new_chase_dir");
        // The chosen direction should be westward or southward.
        // DI_SOUTHWEST=5, DI_WEST=4, DI_SOUTH=6, DI_SOUTHEAST=7, DI_EAST=0, etc.
        // The primary candidate is SW (5); secondary is W or S.
        let dir = mo.movedir;
        assert!(
            dir == super::DI_SOUTHWEST || dir == super::DI_WEST || dir == super::DI_SOUTH
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
        assert_eq!(mo.movedir, super::DI_NODIR, "monster with no target must stay at NODIR");
    }

    #[test]
    fn p_check_sight_returns_true_without_level() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().target = gs.player.handle;

        // No level → falls through to Manhattan distance check (≤ 4096).
        let can_see = super::p_check_sight(&gs, trooper, gs.player.handle, None);
        assert!(can_see, "monster 100 units away must be visible without level");
    }

    #[test]
    fn p_check_sight_returns_false_for_reject_blocked() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().target = gs.player.handle;

        // Build a minimal level with 2 sectors where the REJECT marks them
        // as mutually invisible (all-ones reject data = all blocked).
        // 2 sectors → REJECT needs ceil(4/8) = 1 byte of all 0xFF.
        let reject = doom_map::Reject::parse_lump(&[0xFFu8], 2).unwrap();

        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).unwrap();

        // Build geometry so sector_from_subsector can resolve sector indices.
        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
            doom_map::Vertex { x: 0, y: 64 },
            doom_map::Vertex { x: 64, y: 64 },
        ];
        let sidedefs = vec![
            doom_map::Sidedef {
                x_offset: 0, y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: *b"\0\0\0\0\0\0\0\0",
                sector: 0,
            },
            doom_map::Sidedef {
                x_offset: 0, y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: *b"\0\0\0\0\0\0\0\0",
                sector: 1,
            },
        ];
        let linedefs = vec![
            doom_map::Linedef {
                from_vertex: 0, to_vertex: 1,
                flags: 0, special: 0, tag: 0,
                right_sidedef: 0, left_sidedef: 0xFFFF,
            },
            doom_map::Linedef {
                from_vertex: 2, to_vertex: 3,
                flags: 0, special: 0, tag: 0,
                right_sidedef: 1, left_sidedef: 0xFFFF,
            },
        ];
        let segs = vec![
            doom_map::Seg {
                from_vertex: 0, to_vertex: 1,
                angle: 0, linedef: 0, direction: 0, offset: 0,
            },
            doom_map::Seg {
                from_vertex: 2, to_vertex: 3,
                angle: 0, linedef: 1, direction: 0, offset: 0,
            },
        ];
        let ssectors = vec![
            doom_map::Ssector { seg_count: 1, first_seg: 0 }, // sector 0
            doom_map::Ssector { seg_count: 1, first_seg: 1 }, // sector 1
        ];
        let sectors = vec![
            doom_map::Sector {
                floor_height: 0, ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192, special: 0, tag: 0,
            },
            doom_map::Sector {
                floor_height: 0, ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192, special: 0, tag: 0,
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

        // Both player and monster have subsector=0 by default (from Mobj::new).
        // REJECT sector 0 → sector 0 is bit 0 in the all-0xFF table → blocked.
        let can_see = super::p_check_sight(&gs, trooper, gs.player.handle, Some(&level));
        assert!(!can_see, "all-ones reject must block LOS");
    }

    #[test]
    fn p_check_sight_returns_true_for_all_zero_reject() {
        let mut gs = make_game_state();
        let trooper = spawn_trooper(&mut gs, 100, 0);
        gs.mobjslab.get_mut(trooper).unwrap().target = gs.player.handle;

        // All-zero reject: everything is potentially visible.
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
            // Single ssector with 0 segs — sector_from_subsector returns None,
            // which means the reject is NOT consulted → falls through to distance check.
            ssectors: vec![doom_map::Ssector { seg_count: 0, first_seg: 0 }],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height: 0, ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0", ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192, special: 0, tag: 0,
            }],
            reject,
            blockmap,
        };

        // All-zero reject + distance 100 → should be visible.
        let can_see = super::p_check_sight(&gs, trooper, gs.player.handle, Some(&level));
        assert!(can_see, "all-zero reject + close distance must report visible");
    }
}
