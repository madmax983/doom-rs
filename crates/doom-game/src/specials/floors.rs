#![allow(unused_imports)]
#![allow(dead_code)]
use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

use super::*;
use crate::mobj::MobjHandle;
use crate::movers::*;
use crate::state::*;

// Floor activation functions (public API)

/// Lower floor to lowest adjacent floor on all sectors matching `tag`.
///
/// Creates one `FloorMover` per matching sector.
pub fn ev_floor_lower_to_lowest(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| (i, lowest_adjacent_floor(level, i)))
    {
        activate_floor_lower_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            FloorType::LowerToLowest,
        );
    }
}

/// Lower floor to highest adjacent floor on all sectors matching `tag`.
pub fn ev_floor_lower_to_highest(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| (i, highest_adjacent_floor(level, i)))
    {
        activate_floor_lower_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            FloorType::LowerToHighest,
        );
    }
}

/// Lower floor to next lowest adjacent floor on all sectors matching `tag`.
///
/// "Next lowest" means: find the highest adjacent floor that is still below
/// the sector's current floor. If none, no mover is created.
/// ⚡ Bolt Optimization:
/// Avoids intermediate `.collect::<Vec<_>>()` by processing sectors inline.
pub fn ev_floor_lower_to_nearest(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, s) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
    {
        // Find the highest adjacent floor strictly below our floor.
        let own_floor = s.floor_height;
        let target = adjacent_sectors(level, idx)
            .map(|(_, adj_s)| adj_s.floor_height)
            .filter(|&h| h < own_floor)
            .max()
            .unwrap_or(own_floor);
        activate_floor_lower_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            FloorType::LowerToNearest,
        );
    }
}

/// Raise floor to lowest adjacent ceiling on all sectors matching `tag`.
pub fn ev_floor_raise_to_lowest_ceiling(
    gs: &mut GameState,
    level: &Level,
    tag: u16,
    speed: i16,
    crush: crate::state::CrushBehavior,
) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| (i, lowest_adjacent_ceiling(level, i)))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crush,
            FloorType::RaiseCrush,
        );
    }
}

/// Raise floor to next highest adjacent floor on all sectors matching `tag`.
pub fn ev_floor_raise_to_nearest(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| (i, next_highest_floor(level, i)))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crate::state::CrushBehavior::NoCrush,
            FloorType::RaiseToNearest,
        );
    }
}

/// Raise floor by shortest lower texture height on all sectors matching `tag`.
pub fn ev_floor_raise_by_texture(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, s)| (i, s.floor_height + shortest_lower_texture(level, i)))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crate::state::CrushBehavior::NoCrush,
            FloorType::RaiseByTexture,
        );
    }
}

/// Raise floor by exactly 24 units on all sectors matching `tag`.
pub fn ev_floor_raise_24(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, s)| (i, s.floor_height + 24))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crate::state::CrushBehavior::NoCrush,
            FloorType::Raise24,
        );
    }
}

/// Raise floor by exactly 32 units on all sectors matching `tag`.
pub fn ev_floor_raise_32(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, s)| (i, s.floor_height + 32))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crate::state::CrushBehavior::NoCrush,
            FloorType::Raise32,
        );
    }
}

/// Raise floor to the sector's own ceiling on all sectors matching `tag`.
pub fn ev_floor_raise_to_ceiling(
    gs: &mut GameState,
    level: &Level,
    tag: u16,
    speed: i16,
    crush: crate::state::CrushBehavior,
) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, s)| (i, s.ceil_height))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crush,
            FloorType::RaiseToCeiling,
        );
    }
}

// tick_floors (lifts / floor raisers)

/// Advance all active floor movers by one tic.
///
/// Call this once per tic from `tick()`.
///
/// Lift behavior (wait_tics > 0):
/// 1. Floor lowers to target_height.
/// 2. Enters wait phase for wait_tics.
/// 3. Floor raises back to return_height.
/// 4. Removed when return_height reached.
///
/// Floor raiser/lowerer behavior (wait_tics == -1):
/// 1. Floor moves to target_height.
/// 2. Removed when target reached.
pub fn tick_floors(gs: &mut GameState, level: &mut Level) {
    let mut active_floors = std::mem::take(&mut gs.movers.active_floors);

    active_floors.retain_mut(|floor_mover| {
        let sector_idx = floor_mover.sector_index;
        if sector_idx >= level.sectors.len() {
            return false;
        }

        // Waiting phase.
        if floor_mover.waiting {
            floor_mover.wait_remaining -= 1;
            if floor_mover.wait_remaining <= 0 {
                // Wait over — reverse direction to return.
                floor_mover.waiting = false;
                floor_mover.direction = MoveDirection::Up;
                floor_mover.target_height = floor_mover.return_height;
            }
            return true;
        }

        let speed = floor_mover.speed;
        let direction = floor_mover.direction;
        let target = floor_mover.target_height;
        let wait_tics = floor_mover.wait_tics;
        let crush = floor_mover.crush;
        let crush_dmg: i32 = if crush == crate::state::CrushBehavior::Crush {
            10
        } else {
            0
        };

        match direction {
            MoveDirection::Down => {
                level.sectors[sector_idx].floor_height -= speed;
                let floor = level.sectors[sector_idx].floor_height;

                if floor <= target {
                    level.sectors[sector_idx].floor_height = target;
                    if wait_tics > 0 {
                        // Enter wait phase (e.g., lift at bottom).
                        floor_mover.waiting = true;
                        floor_mover.wait_remaining = wait_tics;
                    } else {
                        // One-shot: remove.
                        return false;
                    }
                }
            }
            MoveDirection::Up => {
                level.sectors[sector_idx].floor_height += speed;
                let floor = level.sectors[sector_idx].floor_height;

                // Crush damage when raising into something.
                if crush == crate::state::CrushBehavior::Crush && crush_dmg > 0 {
                    let ceil = level.sectors[sector_idx].ceil_height;
                    if floor >= ceil - 8 {
                        let player_handle = gs.player.handle;
                        if let Some(pmo) = gs.mobjslab.get_mut(player_handle) {
                            if pmo.z.to_int() >= (floor - 8) as i32 {
                                gs.damage_player(crush_dmg);
                            }
                        }
                    }
                }

                if floor >= target {
                    level.sectors[sector_idx].floor_height = target;
                    if wait_tics > 0 && floor_mover.return_height != target {
                        // Returning phase complete — remove.
                        return false;
                    }
                    // One-shot raiser: remove.
                    return false;
                }
            }
        }

        true
    });

    gs.movers.active_floors = active_floors;
}
