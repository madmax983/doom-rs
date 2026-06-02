use super::*;
use crate::state::*;
use doom_map::{Level, SIDEDEF_NONE};

// ---------------------------------------------------------------------------
// Floor activation functions (public API)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Stair builders
// ---------------------------------------------------------------------------

/// Stair type determines step size and speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StairType {
    /// 8-unit steps, speed 2 (line type 7).
    Build8,
    /// 16-unit turbo steps, speed 4 (line types 8, 100, 127).
    Turbo16,
}

/// Build stairs starting from `start_sector`, walking adjacent sectors via
/// two-sided linedefs. Each successive sector's floor is raised by `step_size`.
///
/// Returns the number of floor movers created.
///
/// # Algorithm
/// Starting from the trigger sector, find an adjacent sector (via a two-sided
/// linedef whose front side is the current sector) that has the same floor
/// flat texture. That becomes the next stair step. Repeat until no more
/// matching adjacent sectors are found.
pub fn ev_build_stairs(
    gs: &mut GameState,
    level: &Level,
    start_sector: usize,
    stair_type: StairType,
    crush: crate::state::CrushBehavior,
) -> usize {
    let (step_size, speed): (i16, i16) = match stair_type {
        StairType::Build8 => (8, 2),
        StairType::Turbo16 => (16, 4),
    };

    let mut count = 0;
    let mut current_sector = start_sector;
    let Some(s) = level.sectors.get(start_sector) else {
        return 0;
    };
    let mut target_height = s.floor_height + step_size;

    // Raise the starting sector first.
    if !gs
        .movers
        .active_floors
        .iter()
        .any(|f| f.sector_index == start_sector)
    {
        gs.movers.active_floors.push(FloorMover {
            sector_index: start_sector,
            target_height,
            speed,
            direction: MoveDirection::Up,
            wait_tics: -1,
            return_height: level.sectors[start_sector].floor_height,
            waiting: false,
            wait_remaining: 0,
            crush,
            tag: 0,
            floor_type: FloorType::RaiseToNearest,
        });
        count += 1;
    }

    // Walk adjacent sectors with matching floor texture.
    loop {
        let cur_flat = level.sectors[current_sector].floor_flat;
        let ld_indices = sector_linedefs(level, current_sector);

        let mut found_next = false;

        for ld_idx in ld_indices {
            let ld = &level.linedefs[ld_idx];
            // Must be two-sided.
            if ld.left_sidedef == SIDEDEF_NONE {
                continue;
            }
            // The "other" sector is on the left side of the linedef.
            let Some(sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
                continue;
            };
            let other_sector = sd.sector as usize;
            // Skip if it's the same sector.
            if other_sector == current_sector {
                continue;
            }
            // Check matching floor texture.
            let Some(s) = level.sectors.get(other_sector) else {
                continue;
            };
            let other_sec = s;
            if other_sec.floor_flat != cur_flat {
                continue;
            }
            // Skip if already has a floor mover.
            if gs
                .movers
                .active_floors
                .iter()
                .any(|f| f.sector_index == other_sector)
            {
                continue;
            }

            target_height += step_size;

            gs.movers.active_floors.push(FloorMover {
                sector_index: other_sector,
                target_height,
                speed,
                direction: MoveDirection::Up,
                wait_tics: -1,
                return_height: other_sec.floor_height,
                waiting: false,
                wait_remaining: 0,
                crush,
                tag: 0,
                floor_type: FloorType::RaiseToNearest,
            });
            count += 1;
            current_sector = other_sector;
            found_next = true;
            break;
        }

        if !found_next {
            break;
        }
    }

    count
}

// ---------------------------------------------------------------------------
// Donut special
// ---------------------------------------------------------------------------

/// Execute a donut special: raise the "donut hole" (inner sector) floor to
/// match the surrounding ring sector's floor height.
///
/// The donut hole is the sector enclosed by the trigger sector. We find it by
/// looking at the back side of linedefs fronting the trigger sector.
///
/// Returns the number of floor movers created.
///
/// # Arguments
///
/// * `gs` - Game state for tracking movers.
/// * `level` - Level containing the donut.
/// * `trigger_sector` - The sector activating the special.
///
/// # Examples
///
/// ```
/// use doom_game::specials::ev_do_donut;
/// // ev_do_donut(&mut gs, &level, sector_idx);
/// ```
pub fn ev_do_donut(gs: &mut GameState, level: &Level, trigger_sector: usize) -> usize {
    let ld_indices = sector_linedefs(level, trigger_sector);

    let mut count = 0;

    for ld_idx in ld_indices {
        let ld = &level.linedefs[ld_idx];
        // Must be two-sided.
        if ld.left_sidedef == SIDEDEF_NONE {
            continue;
        }
        // The "donut hole" is on the back side.
        let Some(sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
            continue;
        };
        let hole_sector = sd.sector as usize;

        if hole_sector == trigger_sector {
            continue;
        }

        // The ring sector provides the target height.
        // Find it by looking at linedefs fronting the hole sector — the ring
        // is the other sector that isn't the trigger sector.
        let hole_ld_indices = sector_linedefs(level, hole_sector);
        let mut ring_floor: Option<i16> = None;

        for hole_ld in hole_ld_indices {
            let hld = &level.linedefs[hole_ld];
            if hld.left_sidedef == SIDEDEF_NONE {
                continue;
            }
            let Some(sd) = level.sidedefs.get(hld.left_sidedef as usize) else {
                continue;
            };
            let ring_sector = sd.sector as usize;
            if ring_sector != hole_sector && ring_sector != trigger_sector {
                if let Some(s) = level.sectors.get(ring_sector) {
                    ring_floor = Some(s.floor_height);
                    break;
                }
            }
        }

        let target = match ring_floor {
            Some(h) => h,
            None => {
                // Fall back: use trigger sector floor as ring floor.
                match level.sectors.get(trigger_sector) {
                    Some(s) => s.floor_height,
                    None => continue,
                }
            }
        };

        // Skip if already has a mover.
        if gs
            .movers
            .active_floors
            .iter()
            .any(|f| f.sector_index == hole_sector)
        {
            continue;
        }

        let Some(s) = level.sectors.get(hole_sector) else {
            continue;
        };
        let hole_sec = s;

        let direction = if target >= hole_sec.floor_height {
            MoveDirection::Up
        } else {
            MoveDirection::Down
        };

        gs.movers.active_floors.push(FloorMover {
            sector_index: hole_sector,
            target_height: target,
            speed: 1,
            direction,
            wait_tics: -1,
            return_height: hole_sec.floor_height,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::NoCrush,
            tag: 0,
            floor_type: FloorType::LowerToLowest,
        });
        count += 1;

        // Only create one mover per donut activation.
        break;
    }

    count
}

// ---------------------------------------------------------------------------
// tick_floors (lifts / floor raisers)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Crusher / lift / floor activation helpers
// ---------------------------------------------------------------------------

/// Standard lift wait time: 3 seconds at 35 Hz = 105 tics.
pub const LIFT_WAIT: i32 = 105;
