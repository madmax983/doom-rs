#![allow(unused_imports)]
#![allow(dead_code)]
use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

use super::*;
use crate::mobj::MobjHandle;
use crate::movers::*;
use crate::state::*;

// Stair builders

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
