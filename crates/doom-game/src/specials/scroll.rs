#![allow(unused_imports)]
#![allow(dead_code)]
use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

use super::*;
use crate::mobj::MobjHandle;
use crate::movers::*;
use crate::state::*;

// Scrolling walls

/// Scan all linedefs in the level for scrolling wall specials and register
/// them in `gs.movers.scrolling_walls`.
///
/// Supported line types:
/// - **48**: Scroll texture left (speed_x = 1, speed_y = 0). The most common
///   scrolling wall in vanilla Doom (used for animated conveyor belts, water
///   textures on walls, etc.).
/// - **85**: Scroll texture right (speed_x = -1, speed_y = 0). Boom extension
///   but widely used in modern WADs.
pub fn init_scrolling_walls(gs: &mut GameState, level: &Level) {
    for (i, ld) in level.linedefs.iter().enumerate() {
        let (sx, sy) = match ld.special {
            48 => (1i16, 0i16),  // scroll left
            85 => (-1i16, 0i16), // scroll right
            _ => continue,
        };
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: i,
            speed_x: sx,
            speed_y: sy,
            accumulated_x: 0,
            accumulated_y: 0,
        });
    }
}

/// Advance all scrolling wall accumulators by one tic.
///
/// Called once per tic from `GameState::tick`. The accumulated offsets are
/// read by the renderer (via `GameState::get_scroll_offset`) and added to
/// the sidedef's `x_offset` / `y_offset` when drawing.
pub fn tick_scrollers(gs: &mut GameState) {
    for sw in &mut gs.movers.scrolling_walls {
        sw.accumulated_x = sw.accumulated_x.wrapping_add(sw.speed_x as i32);
        sw.accumulated_y = sw.accumulated_y.wrapping_add(sw.speed_y as i32);
    }
}

// Conveyor belts

/// Scan all linedefs in the level for conveyor belt specials and register
/// them in `gs.movers.conveyors`.
///
/// Supported line types:
/// - **253**: Scroll floor + push things (conveyor belt).
/// - **254**: Scroll floor + push things + scroll wall.
/// - **255**: Scroll wall using linedef offsets (generalized scroller).
///
/// The push direction and magnitude are derived from the linedef's sidedef
/// texture offsets (`x_offset` and `y_offset` of the right sidedef).
/// The sector affected is the one on the front side of the linedef (the
/// sector referenced by the right sidedef).
pub fn init_conveyors(gs: &mut GameState, level: &Level) {
    for ld in level.linedefs.iter() {
        match ld.special {
            253..=255 => {}
            _ => continue,
        }

        // The right sidedef must exist for the conveyor to function.
        let sd_idx = ld.right_sidedef;
        if sd_idx == SIDEDEF_NONE || (sd_idx as usize) >= level.sidedefs.len() {
            continue;
        }
        let sd = &level.sidedefs[sd_idx as usize];
        let sector_idx = sd.sector as usize;
        if sector_idx >= level.sectors.len() {
            continue;
        }

        // Derive push force from sidedef offsets: x_offset = horizontal push,
        // y_offset = vertical push. This matches the Boom convention.
        let push_x = sd.x_offset as i32;
        let push_y = sd.y_offset as i32;

        // Direction and speed are derived for informational purposes.
        // Direction: atan2(push_y, push_x) in degrees. For simplicity, store 0.
        // Speed: magnitude of push vector, simplified as max of abs values.
        let speed = (push_x.abs().max(push_y.abs())) as i16;

        gs.movers.conveyors.push(ConveyorBelt {
            sector_index: sector_idx,
            push_x,
            push_y,
            direction: 0,
            speed,
        });
    }
}

/// Apply conveyor belt push forces to all actors standing in conveyor sectors.
///
/// Called once per tic from `GameState::tick`. For each conveyor belt, any
/// actor whose `floor_z` (approximated as `z`) matches the sector floor is
/// pushed by the conveyor's force vector.
///
/// This is a simplified implementation — real Doom uses momentum-based push
/// rather than direct position adjustment.
///
/// **Performance:** Avoids 2 internal Vec allocations per game tic by iterating
/// over the components of the game state directly instead of performing `.collect::<Vec<_>>()`. NLL
/// provides the compiler proof necessary to drop mutability constraints correctly.
pub fn tick_conveyors(gs: &mut GameState, level: Option<&Level>) {
    if gs.movers.conveyors.is_empty() {
        return;
    }

    let level = match level {
        Some(lv) => lv,
        None => return, // Cannot determine sector membership without level geometry.
    };

    // Iterate all live actors and apply push if standing in a conveyor sector.
    // Use index iteration to avoid allocating a vector of handles while satisfying the borrow checker,
    // ensuring determinism by capturing the initial bounds.
    let initial_slot_count = gs.mobjslab.slot_count();
    let initial_generation = gs.mobjslab.next_generation();

    for i in 0..initial_slot_count {
        let Some(handle) = gs.mobjslab.handle_at(i) else {
            continue;
        };
        // Skip mobjs spawned during this iteration.
        if handle.generation >= initial_generation {
            continue;
        }

        let Some(mo) = gs.mobjslab.get(handle) else {
            continue;
        };
        let mz = mo.z.to_int();

        for conveyor in &gs.movers.conveyors {
            let sector_idx = conveyor.sector_index;
            if sector_idx >= level.sectors.len() {
                continue;
            }
            let floor_h = level.sectors[sector_idx].floor_height as i32;

            // Simple containment check: actor z matches sector floor.
            if mz == floor_h {
                if let Some(mo) = gs.mobjslab.get_mut(handle) {
                    mo.x += Fixed16_16::from_raw(conveyor.push_x);
                    mo.y += Fixed16_16::from_raw(conveyor.push_y);
                }
                break; // Only apply one conveyor per actor per tic.
            }
        }
    }
}
