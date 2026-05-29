#![allow(unused_imports)]
#![allow(dead_code)]
use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

use super::*;
use crate::mobj::MobjHandle;
use crate::movers::*;
use crate::state::*;

// Donut special

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

// Perpetual platforms

/// Standard platform wait time: 3 seconds at 35 Hz ≈ 105 tics.
const PLATFORM_WAIT: i32 = 105;

/// Activate a perpetual platform on all sectors matching `tag`.
///
/// The platform oscillates between the lowest adjacent floor and the sector's
/// current floor height.
///
/// Returns the number of platforms created.
///
/// # Arguments
///
/// * `gs` - Game state tracking platforms.
/// * `level` - Map containing tagged sectors.
/// * `tag` - Identify the sector to activate.
/// * `speed` - Rate of platform movement.
///
/// # Examples
///
/// ```
/// use doom_game::specials::ev_perpetual_platform;
/// // ev_perpetual_platform(&mut gs, &level, 1, 8);
/// ```
pub fn ev_perpetual_platform(gs: &mut GameState, level: &Level, tag: u16, speed: i16) -> usize {
    let mut count = 0;
    for idx in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
    {
        // Avoid duplicate platforms on the same sector.
        if gs
            .movers
            .active_platforms
            .iter()
            .any(|p| p.sector_index == idx)
        {
            continue;
        }
        let sector = &level.sectors[idx];
        let low = lowest_adjacent_floor(level, idx);
        let high = sector.floor_height;

        gs.movers.active_platforms.push(PerpetualPlatform {
            sector_index: idx,
            low_height: low,
            high_height: high,
            speed,
            wait_tics: PLATFORM_WAIT,
            wait_remaining: 0,
            status: PlatformStatus::Down,
            tag,
        });
        count += 1;
    }
    count
}

/// Advance all active perpetual platforms by one tic.
///
/// Platforms oscillate:
/// 1. Move floor down by `speed` until `low_height` is reached.
/// 2. Enter wait phase for `wait_tics`.
/// 3. Move floor up by `speed` until `high_height` is reached.
/// 4. Enter wait phase for `wait_tics`.
/// 5. Repeat.
///
/// # Arguments
///
/// * `gs` - Game state containing active platforms.
/// * `level` - Level to mutate sector floors in.
///
/// # Examples
///
/// ```
/// use doom_game::specials::tick_platforms;
/// // tick_platforms(&mut gs, &mut level);
/// ```
pub fn tick_platforms(gs: &mut GameState, level: &mut Level) {
    for plat in &mut gs.movers.active_platforms {
        let sector_idx = plat.sector_index;
        if sector_idx >= level.sectors.len() {
            continue;
        }

        match plat.status {
            PlatformStatus::Waiting => {
                plat.wait_remaining -= 1;
                if plat.wait_remaining <= 0 {
                    // Determine which direction to go next.
                    let floor = level.sectors[sector_idx].floor_height;
                    if floor <= plat.low_height {
                        plat.status = PlatformStatus::Up;
                    } else {
                        plat.status = PlatformStatus::Down;
                    }
                }
            }
            PlatformStatus::Down => {
                level.sectors[sector_idx].floor_height -= plat.speed;
                let floor = level.sectors[sector_idx].floor_height;
                if floor <= plat.low_height {
                    level.sectors[sector_idx].floor_height = plat.low_height;
                    plat.status = PlatformStatus::Waiting;
                    plat.wait_remaining = plat.wait_tics;
                }
            }
            PlatformStatus::Up => {
                level.sectors[sector_idx].floor_height += plat.speed;
                let floor = level.sectors[sector_idx].floor_height;
                if floor >= plat.high_height {
                    level.sectors[sector_idx].floor_height = plat.high_height;
                    plat.status = PlatformStatus::Waiting;
                    plat.wait_remaining = plat.wait_tics;
                }
            }
        }
    }
}
