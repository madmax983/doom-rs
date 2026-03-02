//! Sector specials and linedef triggers.
//!
//! Port of Doom's `p_spec.c` and `p_ceilng.c` / `p_doors.c` (simplified).
//!
//! # Implemented
//! - `tick_sector_specials`: damage floors (specials 5, 7, 16).
//! - `tick_doors`: advance active door/floor movers.
//! - `tick_lights`: advance light specials.
//! - `spawn_level_specials`: initialise light thinkers on level load.
//! - `p_use_lines`: player USE activation, dispatches to `activate_linedef`.
//! - `activate_linedef`: door toggle (types 1, 2, 26–29, 63, 64), exits (11, 51, 52, 124).

use doom_map::{Level, SIDEDEF_NONE};

use crate::mobj::MobjHandle;
use crate::state::{
    CeilingMover, DoorMover, ExitRequest, FloorMover, GameState, LightSpecial, MoveDirection,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Distance ahead the player can activate a linedef.
pub const USE_RANGE: i32 = 64;

/// Door open/close speed in map units per tic (Doom standard: 2 units/tic).
const DOOR_SPEED: i16 = 2;

/// Tics a door stays open before auto-closing (3.5 seconds at 35 Hz ≈ 120 tics).
const DOOR_WAIT: i32 = 120;

/// Period for fast blinking lights (tics).
const BLINK_FAST_PERIOD: i32 = 15;

/// Period for slow blinking lights (tics).
const BLINK_SLOW_PERIOD: i32 = 35;

// ---------------------------------------------------------------------------
// tick_sector_specials
// ---------------------------------------------------------------------------

/// Apply sector special damage to the actor each tic.
///
/// Simplified port of `P_PlayerInSpecialSector`.
///
/// Sector containment is approximated: the actor is considered to be "in" a
/// special sector if `actor.z.to_int() == sector.floor_height as i32`.
///
/// Damage is applied directly to `mobj.health` without routing through combat
/// to avoid circular dependencies at this stage.
pub fn tick_sector_specials(gs: &mut GameState, level: &Level, handle: MobjHandle) {
    // Read actor position.
    let (az, _ax, _ay) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.z.to_int(), mo.x.to_int(), mo.y.to_int()),
        None => return,
    };

    for sector in &level.sectors {
        if sector.special == 0 {
            continue;
        }

        // Only apply damage if actor is standing on this floor.
        if az != sector.floor_height as i32 {
            continue;
        }

        let dmg: i32 = match sector.special {
            5 => 10,  // lava
            7 => 5,   // nukage
            16 => 20, // acid
            _ => continue,
        };

        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.health -= dmg;
            if mo.health < 0 {
                mo.health = 0;
            }
        }

        // Only apply one sector's damage per tic (first match wins).
        return;
    }
}

// ---------------------------------------------------------------------------
// tick_doors
// ---------------------------------------------------------------------------

/// Advance all active door/floor movers by one tic.
///
/// Call this once per tic from `tick()`.
pub fn tick_doors(gs: &mut GameState, level: &mut Level) {
    let mut i = 0;
    while i < gs.active_doors.len() {
        // Borrow just the fields we need, then operate.
        let countdown = gs.active_doors[i].countdown;
        let speed_abs = gs.active_doors[i].speed.abs();

        // Waiting at open position?
        if countdown > 0 {
            gs.active_doors[i].countdown -= 1;
            i += 1;
            continue;
        }
        if countdown == 0 {
            // Start closing — negate speed so it moves downward.
            gs.active_doors[i].speed = -speed_abs;
            gs.active_doors[i].countdown = -1;
        }

        // Move toward target.
        let sector_idx = gs.active_doors[i].sector;
        let speed = gs.active_doors[i].speed;
        let target = gs.active_doors[i].target_height;
        let is_ceiling = gs.active_doors[i].is_ceiling;

        if sector_idx < level.sectors.len() {
            let sector = &mut level.sectors[sector_idx];
            let height = if is_ceiling {
                &mut sector.ceil_height
            } else {
                &mut sector.floor_height
            };
            *height += speed;

            let reached = if speed > 0 {
                *height >= target
            } else {
                *height <= target
            };

            if reached {
                *height = target;
                // Update current_height mirror.
                gs.active_doors[i].current_height = target;
                gs.active_doors.remove(i);
                // Do NOT increment i — item was removed.
                continue;
            }
            // Mirror current height.
            let new_h = if is_ceiling {
                level.sectors[sector_idx].ceil_height
            } else {
                level.sectors[sector_idx].floor_height
            };
            gs.active_doors[i].current_height = new_h;
        }

        i += 1;
    }
}

// ---------------------------------------------------------------------------
// tick_lights
// ---------------------------------------------------------------------------

/// Advance all light specials by one tic.
pub fn tick_lights(gs: &mut GameState, level: &mut Level) {
    for light in &mut gs.active_lights {
        light.timer -= 1;
        if light.timer <= 0 {
            light.is_bright = !light.is_bright;
            light.timer = light.period;
            if light.sector < level.sectors.len() {
                level.sectors[light.sector].light_level = if light.is_bright {
                    light.bright
                } else {
                    light.dark
                };
            }
        }
    }
}

// ---------------------------------------------------------------------------
// spawn_level_specials
// ---------------------------------------------------------------------------

/// Scan all sectors and spawn light specials based on `sector.special`.
///
/// Call this once after loading a level, before the first tic.
pub fn spawn_level_specials(gs: &mut GameState, level: &Level) {
    for (i, sector) in level.sectors.iter().enumerate() {
        match sector.special {
            1 => {
                // Random off: slow blink, goes dark.
                gs.active_lights.push(LightSpecial {
                    sector: i,
                    timer: BLINK_SLOW_PERIOD,
                    period: BLINK_SLOW_PERIOD,
                    bright: sector.light_level,
                    dark: 0,
                    is_bright: true,
                });
            }
            2 => {
                // Fast strobe.
                gs.active_lights.push(LightSpecial {
                    sector: i,
                    timer: BLINK_FAST_PERIOD,
                    period: BLINK_FAST_PERIOD,
                    bright: sector.light_level,
                    dark: 0,
                    is_bright: true,
                });
            }
            3 => {
                // Slow strobe: dim but not fully dark.
                gs.active_lights.push(LightSpecial {
                    sector: i,
                    timer: BLINK_SLOW_PERIOD,
                    period: BLINK_SLOW_PERIOD,
                    bright: sector.light_level,
                    dark: 35,
                    is_bright: true,
                });
            }
            _ => {} // Other specials handled by tick_sector_specials.
        }
    }
}

// ---------------------------------------------------------------------------
// Adjacent sector height helpers
// ---------------------------------------------------------------------------

/// Find the lowest floor height among all sectors adjacent to `sector_index`.
///
/// Adjacent means: the sector shares a two-sided linedef with the given sector.
/// If the sector has no adjacent sectors, returns the sector's own floor height.
pub fn lowest_adjacent_floor(level: &Level, sector_index: usize) -> i16 {
    let own_floor = level
        .sectors
        .get(sector_index)
        .map(|s| s.floor_height)
        .unwrap_or(0);
    let mut lowest = i16::MAX;
    let mut found = false;

    for ld in &level.linedefs {
        // Only two-sided linedefs connect sectors.
        if ld.left_sidedef == SIDEDEF_NONE {
            continue;
        }
        let right_sector = level
            .sidedefs
            .get(ld.right_sidedef as usize)
            .map(|s| s.sector as usize);
        let left_sector = level
            .sidedefs
            .get(ld.left_sidedef as usize)
            .map(|s| s.sector as usize);

        let other = if right_sector == Some(sector_index) {
            left_sector
        } else if left_sector == Some(sector_index) {
            right_sector
        } else {
            continue;
        };

        if let Some(other_idx) = other {
            if let Some(other_sec) = level.sectors.get(other_idx) {
                found = true;
                if other_sec.floor_height < lowest {
                    lowest = other_sec.floor_height;
                }
            }
        }
    }

    if found { lowest } else { own_floor }
}

/// Find the highest floor height among all sectors adjacent to `sector_index`.
///
/// Used for "lower to highest adjacent floor" specials.
/// If no adjacent sectors, returns the sector's own floor height.
pub fn highest_adjacent_floor(level: &Level, sector_index: usize) -> i16 {
    let own_floor = level
        .sectors
        .get(sector_index)
        .map(|s| s.floor_height)
        .unwrap_or(0);
    let mut highest = i16::MIN;
    let mut found = false;

    for ld in &level.linedefs {
        if ld.left_sidedef == SIDEDEF_NONE {
            continue;
        }
        let right_sector = level
            .sidedefs
            .get(ld.right_sidedef as usize)
            .map(|s| s.sector as usize);
        let left_sector = level
            .sidedefs
            .get(ld.left_sidedef as usize)
            .map(|s| s.sector as usize);

        let other = if right_sector == Some(sector_index) {
            left_sector
        } else if left_sector == Some(sector_index) {
            right_sector
        } else {
            continue;
        };

        if let Some(other_idx) = other {
            if let Some(other_sec) = level.sectors.get(other_idx) {
                found = true;
                if other_sec.floor_height > highest {
                    highest = other_sec.floor_height;
                }
            }
        }
    }

    if found { highest } else { own_floor }
}

/// Find the next floor height above the current sector's floor among adjacent sectors.
///
/// Scans all adjacent sector floors and returns the smallest one that is strictly
/// greater than the current sector's floor height. If none is found, returns the
/// sector's own floor height (no change).
pub fn next_highest_floor(level: &Level, sector_index: usize) -> i16 {
    let own_floor = level
        .sectors
        .get(sector_index)
        .map(|s| s.floor_height)
        .unwrap_or(0);
    let mut next = i16::MAX;
    let mut found = false;

    for ld in &level.linedefs {
        if ld.left_sidedef == SIDEDEF_NONE {
            continue;
        }
        let right_sector = level
            .sidedefs
            .get(ld.right_sidedef as usize)
            .map(|s| s.sector as usize);
        let left_sector = level
            .sidedefs
            .get(ld.left_sidedef as usize)
            .map(|s| s.sector as usize);

        let other = if right_sector == Some(sector_index) {
            left_sector
        } else if left_sector == Some(sector_index) {
            right_sector
        } else {
            continue;
        };

        if let Some(other_idx) = other {
            if let Some(other_sec) = level.sectors.get(other_idx) {
                if other_sec.floor_height > own_floor && other_sec.floor_height < next {
                    found = true;
                    next = other_sec.floor_height;
                }
            }
        }
    }

    if found { next } else { own_floor }
}

/// Find the lowest ceiling height among all sectors adjacent to `sector_index`.
///
/// Used for "raise floor to lowest adjacent ceiling" specials.
/// If no adjacent sectors, returns the sector's own ceiling height.
pub fn lowest_adjacent_ceiling(level: &Level, sector_index: usize) -> i16 {
    let own_ceil = level
        .sectors
        .get(sector_index)
        .map(|s| s.ceil_height)
        .unwrap_or(0);
    let mut lowest = i16::MAX;
    let mut found = false;

    for ld in &level.linedefs {
        if ld.left_sidedef == SIDEDEF_NONE {
            continue;
        }
        let right_sector = level
            .sidedefs
            .get(ld.right_sidedef as usize)
            .map(|s| s.sector as usize);
        let left_sector = level
            .sidedefs
            .get(ld.left_sidedef as usize)
            .map(|s| s.sector as usize);

        let other = if right_sector == Some(sector_index) {
            left_sector
        } else if left_sector == Some(sector_index) {
            right_sector
        } else {
            continue;
        };

        if let Some(other_idx) = other {
            if let Some(other_sec) = level.sectors.get(other_idx) {
                found = true;
                if other_sec.ceil_height < lowest {
                    lowest = other_sec.ceil_height;
                }
            }
        }
    }

    if found { lowest } else { own_ceil }
}

// ---------------------------------------------------------------------------
// tick_ceilings (crushers)
// ---------------------------------------------------------------------------

/// Advance all active ceiling movers / crushers by one tic.
///
/// Call this once per tic from `tick()`.
///
/// Crusher oscillation:
/// 1. Move ceiling by `speed` in `direction`.
/// 2. If moving Down and reaches `bottom_height`: reverse to Up.
///    If in crush range, apply `crush_damage` to the player (simplified).
/// 3. If moving Up and reaches `top_height`: reverse to Down (perpetual)
///    or remove (one-shot).
pub fn tick_ceilings(gs: &mut GameState, level: &mut Level) {
    let mut i = 0;
    while i < gs.active_ceilings.len() {
        let sector_idx = gs.active_ceilings[i].sector_index;
        let speed = gs.active_ceilings[i].speed;
        let direction = gs.active_ceilings[i].direction;
        let top = gs.active_ceilings[i].top_height;
        let bottom = gs.active_ceilings[i].bottom_height;
        let crush_dmg = gs.active_ceilings[i].crush_damage;
        let remove_when_done = gs.active_ceilings[i].remove_when_done;

        if sector_idx >= level.sectors.len() {
            gs.active_ceilings.remove(i);
            continue;
        }

        match direction {
            MoveDirection::Down => {
                level.sectors[sector_idx].ceil_height -= speed;
                let ceil = level.sectors[sector_idx].ceil_height;
                let floor = level.sectors[sector_idx].floor_height;

                // Crush damage: when ceiling is close to floor (within 8 units).
                if ceil <= floor + 8 && crush_dmg > 0 {
                    // Simplified: damage player if they are in this sector.
                    // A proper implementation would iterate all mobjs in the sector.
                    let player_handle = gs.player.handle;
                    if let Some(pmo) = gs.mobjslab.get(player_handle) {
                        if pmo.z.to_int() == floor as i32 {
                            // Very simplified sector check: just damage if z matches.
                            if let Some(pmo_mut) = gs.mobjslab.get_mut(player_handle) {
                                pmo_mut.health -= crush_dmg;
                                if pmo_mut.health < 0 {
                                    pmo_mut.health = 0;
                                }
                            }
                        }
                    }
                }

                if ceil <= bottom {
                    level.sectors[sector_idx].ceil_height = bottom;
                    gs.active_ceilings[i].direction = MoveDirection::Up;
                }
            }
            MoveDirection::Up => {
                level.sectors[sector_idx].ceil_height += speed;
                let ceil = level.sectors[sector_idx].ceil_height;

                if ceil >= top {
                    level.sectors[sector_idx].ceil_height = top;
                    if remove_when_done {
                        gs.active_ceilings.remove(i);
                        continue;
                    }
                    // Perpetual: reverse back to Down.
                    gs.active_ceilings[i].direction = MoveDirection::Down;
                }
            }
        }

        i += 1;
    }
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
    let mut i = 0;
    while i < gs.active_floors.len() {
        let sector_idx = gs.active_floors[i].sector_index;
        if sector_idx >= level.sectors.len() {
            gs.active_floors.remove(i);
            continue;
        }

        // Waiting phase.
        if gs.active_floors[i].waiting {
            gs.active_floors[i].wait_remaining -= 1;
            if gs.active_floors[i].wait_remaining <= 0 {
                // Wait over — reverse direction to return.
                gs.active_floors[i].waiting = false;
                gs.active_floors[i].direction = MoveDirection::Up;
                gs.active_floors[i].target_height = gs.active_floors[i].return_height;
            }
            i += 1;
            continue;
        }

        let speed = gs.active_floors[i].speed;
        let direction = gs.active_floors[i].direction;
        let target = gs.active_floors[i].target_height;
        let wait_tics = gs.active_floors[i].wait_tics;
        let crush = gs.active_floors[i].crush;
        let crush_dmg: i32 = if crush { 10 } else { 0 };

        match direction {
            MoveDirection::Down => {
                level.sectors[sector_idx].floor_height -= speed;
                let floor = level.sectors[sector_idx].floor_height;

                if floor <= target {
                    level.sectors[sector_idx].floor_height = target;
                    if wait_tics > 0 {
                        // Enter wait phase (e.g., lift at bottom).
                        gs.active_floors[i].waiting = true;
                        gs.active_floors[i].wait_remaining = wait_tics;
                    } else {
                        // One-shot: remove.
                        gs.active_floors.remove(i);
                        continue;
                    }
                }
            }
            MoveDirection::Up => {
                level.sectors[sector_idx].floor_height += speed;
                let floor = level.sectors[sector_idx].floor_height;

                // Crush damage when raising into something.
                if crush && crush_dmg > 0 {
                    let ceil = level.sectors[sector_idx].ceil_height;
                    if floor >= ceil - 8 {
                        let player_handle = gs.player.handle;
                        if let Some(pmo) = gs.mobjslab.get_mut(player_handle) {
                            if pmo.z.to_int() >= (floor - 8) as i32 {
                                pmo.health -= crush_dmg;
                                if pmo.health < 0 {
                                    pmo.health = 0;
                                }
                            }
                        }
                    }
                }

                if floor >= target {
                    level.sectors[sector_idx].floor_height = target;
                    if wait_tics > 0 && gs.active_floors[i].return_height != target {
                        // Returning phase complete — remove.
                        gs.active_floors.remove(i);
                        continue;
                    }
                    // One-shot raiser: remove.
                    gs.active_floors.remove(i);
                    continue;
                }
            }
        }

        i += 1;
    }
}

// ---------------------------------------------------------------------------
// Crusher / lift / floor activation helpers
// ---------------------------------------------------------------------------

/// Standard lift wait time: 3 seconds at 35 Hz = 105 tics.
const LIFT_WAIT: i32 = 105;

/// Activate a crusher on all sectors matching `tag`.
fn activate_crusher(
    gs: &mut GameState,
    level: &Level,
    tag: u16,
    speed: i16,
    crush_damage: i32,
    silent: bool,
    remove_when_done: bool,
) {
    let sector_indices: Vec<usize> = level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
        .collect();

    for idx in sector_indices {
        // Avoid duplicate crushers on the same sector.
        if gs.active_ceilings.iter().any(|c| c.sector_index == idx) {
            continue;
        }
        let sector = &level.sectors[idx];
        gs.active_ceilings.push(CeilingMover {
            sector_index: idx,
            top_height: sector.ceil_height,
            bottom_height: sector.floor_height + 8,
            speed,
            crush_damage,
            direction: MoveDirection::Down,
            silent,
            remove_when_done,
            tag,
        });
    }
}

/// Stop all crushers matching `tag` (line type 57).
fn stop_crushers(gs: &mut GameState, tag: u16) {
    gs.active_ceilings.retain(|c| c.tag != tag);
}

/// Activate a lift (lower-wait-raise) on all sectors matching `tag`.
fn activate_lift(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    let sector_indices: Vec<usize> = level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
        .collect();

    for idx in sector_indices {
        // Avoid duplicate floor movers on the same sector.
        if gs.active_floors.iter().any(|f| f.sector_index == idx) {
            continue;
        }
        let sector = &level.sectors[idx];
        let low = lowest_adjacent_floor(level, idx);
        gs.active_floors.push(FloorMover {
            sector_index: idx,
            target_height: low,
            speed,
            direction: MoveDirection::Down,
            wait_tics: LIFT_WAIT,
            return_height: sector.floor_height,
            waiting: false,
            wait_remaining: 0,
            crush: false,
            tag,
        });
    }
}

/// Activate a floor raiser (one-shot, no wait) on a single sector.
fn activate_floor_raise_single(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    tag: u16,
    target_height: i16,
    speed: i16,
    crush: bool,
) {
    if gs
        .active_floors
        .iter()
        .any(|f| f.sector_index == sector_idx)
    {
        return;
    }
    let Some(sector) = level.sectors.get(sector_idx) else {
        return;
    };
    gs.active_floors.push(FloorMover {
        sector_index: sector_idx,
        target_height,
        speed,
        direction: MoveDirection::Up,
        wait_tics: -1,
        return_height: sector.floor_height,
        waiting: false,
        wait_remaining: 0,
        crush,
        tag,
    });
}

/// Activate a floor lowerer (one-shot, no wait) on a single sector.
fn activate_floor_lower_single(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    tag: u16,
    target_height: i16,
    speed: i16,
) {
    if gs
        .active_floors
        .iter()
        .any(|f| f.sector_index == sector_idx)
    {
        return;
    }
    let Some(sector) = level.sectors.get(sector_idx) else {
        return;
    };
    gs.active_floors.push(FloorMover {
        sector_index: sector_idx,
        target_height,
        speed,
        direction: MoveDirection::Down,
        wait_tics: -1,
        return_height: sector.floor_height,
        waiting: false,
        wait_remaining: 0,
        crush: false,
        tag,
    });
}

// ---------------------------------------------------------------------------
// Door helpers
// ---------------------------------------------------------------------------

/// Enqueue a door mover that opens and optionally auto-closes.
fn open_door(gs: &mut GameState, level: &Level, sector_idx: usize, auto_close: bool) {
    let sector = match level.sectors.get(sector_idx) {
        Some(s) => s,
        None => return,
    };

    // Use sector's current ceiling as the open target (at least 128 above floor).
    let target = sector.ceil_height.max(sector.floor_height + 128);

    // Avoid duplicate movers for the same sector.
    if gs.active_doors.iter().any(|d| d.sector == sector_idx) {
        return;
    }

    gs.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: DOOR_SPEED,
        is_ceiling: true,
        wait_tics: if auto_close { DOOR_WAIT } else { -1 },
        countdown: if auto_close { DOOR_WAIT } else { -1 },
    });
}

/// Enqueue a door mover that closes a door.
fn close_door(gs: &mut GameState, level: &Level, sector_idx: usize) {
    let sector = match level.sectors.get(sector_idx) {
        Some(s) => s,
        None => return,
    };

    let target = sector.floor_height + 4;

    // Avoid duplicate movers for the same sector.
    if gs.active_doors.iter().any(|d| d.sector == sector_idx) {
        return;
    }

    gs.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: -DOOR_SPEED,
        is_ceiling: true,
        wait_tics: -1,
        countdown: -1,
    });
}

// ---------------------------------------------------------------------------
// p_use_lines
// ---------------------------------------------------------------------------

/// Check whether the player's USE action activates a linedef.
///
/// Port of `P_UseLines`. Casts a short ray from the actor's position toward
/// the direction they are facing and checks every linedef with a special for
/// intersection.
///
/// Because the trig tables may be uninitialized in tests (returning 0), the
/// function falls back to `ahead_x = ax + USE_RANGE, ahead_y = ay` when both
/// `cos` and `sin` are zero.
///
/// Only the first intersected linedef with a non-zero special is activated.
pub fn p_use_lines(gs: &mut GameState, level: &mut Level, handle: MobjHandle) {
    // Read actor position and angle.
    let (ax, ay, angle) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.angle),
        None => return,
    };

    let cos_int = angle.cos().to_int();
    let sin_int = angle.sin().to_int();

    // Fall back if trig tables are uninitialised (both return 0).
    let (ahead_x, ahead_y) = if cos_int == 0 && sin_int == 0 {
        (ax + USE_RANGE, ay)
    } else {
        (ax + USE_RANGE * cos_int, ay + USE_RANGE * sin_int)
    };

    // Find and activate the first linedef whose special segment the ray crosses.
    for ld_idx in 0..level.linedefs.len() {
        let ld = &level.linedefs[ld_idx];
        if ld.special == 0 {
            continue;
        }

        let v1 = &level.vertexes[ld.from_vertex as usize];
        let v2 = &level.vertexes[ld.to_vertex as usize];

        let lx1 = v1.x as i32;
        let ly1 = v1.y as i32;
        let lx2 = v2.x as i32;
        let ly2 = v2.y as i32;

        if segment_crosses_line(ax, ay, ahead_x, ahead_y, lx1, ly1, lx2, ly2) {
            activate_linedef(gs, level, ld_idx);
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// activate_linedef
// ---------------------------------------------------------------------------

/// Dispatch a linedef activation by its special number.
///
/// Handles:
/// - **1**: Door toggle — opens a closed door or closes an open one (immediate for compat).
/// - **2**: Open door, stays open (animated via DoorMover).
/// - **26**: Blue-key locked door.
/// - **27**: Yellow-key locked door.
/// - **28**: Red-key locked door.
/// - **29**: Close door (animated via DoorMover).
/// - **63**: Remote door open-stay (by tag).
/// - **64**: Remote door open-close (by tag).
/// - **11**: Exit — no-op stub.
/// - Other: no-op.
pub fn activate_linedef(gs: &mut GameState, level: &mut Level, linedef_idx: usize) {
    let Some(ld) = level.linedefs.get(linedef_idx) else {
        return;
    };

    let special = ld.special;

    // Find the sector behind the linedef (back sector for door triggers).
    let left_sidedef = ld.left_sidedef;
    if left_sidedef == SIDEDEF_NONE && special != 63 && special != 64 {
        // One-sided linedef — nothing to toggle for most specials.
        // Tag-based specials handle their own sector lookup.
    }

    match special {
        // --- Type 1: toggle door (immediate, for backward compatibility with existing tests) ---
        1 => {
            if left_sidedef == SIDEDEF_NONE {
                return;
            }
            let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                Some(sd) => sd.sector as usize,
                None => return,
            };

            let Some(sector) = level.sectors.get_mut(sector_idx) else {
                return;
            };

            if sector.ceil_height > sector.floor_height {
                // Door is open — close it.
                sector.ceil_height = sector.floor_height;
            } else {
                // Door is closed — open it.
                sector.ceil_height = sector.floor_height + 128;
            }
        }

        // --- Type 2: open door, stays open (animated) ---
        2 => {
            if left_sidedef == SIDEDEF_NONE {
                return;
            }
            let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                Some(sd) => sd.sector as usize,
                None => return,
            };
            open_door(gs, level, sector_idx, false);
        }

        // --- Type 29: close door (animated) ---
        29 => {
            if left_sidedef == SIDEDEF_NONE {
                return;
            }
            let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                Some(sd) => sd.sector as usize,
                None => return,
            };
            close_door(gs, level, sector_idx);
        }

        // --- Types 26/27/28: locked raise-and-close door ---
        26 => {
            // Blue card or skull required.
            if gs.player.has_key(crate::player::KEY_BLUE_CARD)
                || gs.player.has_key(crate::player::KEY_BLUE_SKULL)
            {
                if left_sidedef == SIDEDEF_NONE {
                    return;
                }
                let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                    Some(sd) => sd.sector as usize,
                    None => return,
                };
                open_door(gs, level, sector_idx, true);
            }
        }
        27 => {
            // Yellow key required.
            if gs.player.has_key(crate::player::KEY_YELLOW_CARD)
                || gs.player.has_key(crate::player::KEY_YELLOW_SKULL)
            {
                if left_sidedef == SIDEDEF_NONE {
                    return;
                }
                let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                    Some(sd) => sd.sector as usize,
                    None => return,
                };
                open_door(gs, level, sector_idx, true);
            }
        }
        28 => {
            // Red key required.
            if gs.player.has_key(crate::player::KEY_RED_CARD)
                || gs.player.has_key(crate::player::KEY_RED_SKULL)
            {
                if left_sidedef == SIDEDEF_NONE {
                    return;
                }
                let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                    Some(sd) => sd.sector as usize,
                    None => return,
                };
                open_door(gs, level, sector_idx, true);
            }
        }

        // --- Type 63/64: remote tag-based door ---
        63 | 64 => {
            let tag = level.linedefs[linedef_idx].tag;
            let sector_indices: Vec<usize> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
                .collect();
            for idx in sector_indices {
                open_door(gs, level, idx, special == 64);
            }
        }

        // --- Type 11: S1 Exit (normal) ---
        11 => {
            gs.exit_request = Some(ExitRequest::Normal);
        }

        // --- Type 51: S1 Secret Exit ---
        51 => {
            gs.exit_request = Some(ExitRequest::Secret);
        }

        // --- Type 52: W1 Exit (walk trigger, normal) ---
        52 => {
            gs.exit_request = Some(ExitRequest::Normal);
        }

        // --- Type 124: W1 Secret Exit (walk trigger) ---
        124 => {
            gs.exit_request = Some(ExitRequest::Secret);
        }

        // -----------------------------------------------------------------
        // Crushers
        // -----------------------------------------------------------------

        // Type 6: Fast crusher ceiling (perpetual, speed=2).
        6 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_crusher(gs, level, tag, 2, 10, false, false);
        }

        // Type 25: Slow crusher ceiling (perpetual, speed=1).
        25 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_crusher(gs, level, tag, 1, 10, false, false);
        }

        // Type 44: Ceiling lower to 8 above floor (one-shot, no crush damage).
        44 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_crusher(gs, level, tag, 2, 0, true, true);
        }

        // Type 57: Stop crusher (remove all crushers matching tag).
        57 => {
            let tag = level.linedefs[linedef_idx].tag;
            stop_crushers(gs, tag);
        }

        // -----------------------------------------------------------------
        // Lifts (lower-wait-raise)
        // -----------------------------------------------------------------

        // Type 62: Plat lower-wait-raise (speed 4).
        62 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 66: Plat lower-wait-raise (speed 4, repeatable).
        66 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 10: Plat down-wait-up-stay (door-like lift).
        10 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 21: Plat down-wait-up-stay (switch).
        21 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 88: Plat down-wait-up-stay-monster (walk trigger).
        88 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 121: Plat lower-wait-raise (turbo speed 8).
        121 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 8);
        }

        // -----------------------------------------------------------------
        // Floor raisers
        // -----------------------------------------------------------------

        // Type 5: Floor raise to lowest adjacent ceiling.
        5 => {
            let tag = level.linedefs[linedef_idx].tag;
            let per_sector: Vec<(usize, i16)> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, lowest_adjacent_ceiling(level, i)))
                .collect();
            for (idx, target) in per_sector {
                activate_floor_raise_single(gs, level, idx, tag, target, 1, true);
            }
        }

        // Type 18: Floor raise to next highest adjacent floor.
        18 => {
            let tag = level.linedefs[linedef_idx].tag;
            let per_sector: Vec<(usize, i16)> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, next_highest_floor(level, i)))
                .collect();
            for (idx, target) in per_sector {
                activate_floor_raise_single(gs, level, idx, tag, target, 1, false);
            }
        }

        // Type 22: Floor raise to next highest adjacent floor (switch variant).
        22 => {
            let tag = level.linedefs[linedef_idx].tag;
            let per_sector: Vec<(usize, i16)> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, next_highest_floor(level, i)))
                .collect();
            for (idx, target) in per_sector {
                activate_floor_raise_single(gs, level, idx, tag, target, 1, false);
            }
        }

        // -----------------------------------------------------------------
        // Floor lowerers
        // -----------------------------------------------------------------

        // Type 23: Floor lower to lowest adjacent floor.
        23 => {
            let tag = level.linedefs[linedef_idx].tag;
            let per_sector: Vec<(usize, i16)> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, lowest_adjacent_floor(level, i)))
                .collect();
            for (idx, target) in per_sector {
                activate_floor_lower_single(gs, level, idx, tag, target, 1);
            }
        }

        // Type 19: Floor lower to highest adjacent floor.
        19 => {
            let tag = level.linedefs[linedef_idx].tag;
            let per_sector: Vec<(usize, i16)> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, highest_adjacent_floor(level, i)))
                .collect();
            for (idx, target) in per_sector {
                activate_floor_lower_single(gs, level, idx, tag, target, 1);
            }
        }

        // Type 38: Floor lower to lowest adjacent floor (walk trigger).
        38 => {
            let tag = level.linedefs[linedef_idx].tag;
            let per_sector: Vec<(usize, i16)> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, lowest_adjacent_floor(level, i)))
                .collect();
            for (idx, target) in per_sector {
                activate_floor_lower_single(gs, level, idx, tag, target, 1);
            }
        }

        // Type 36: Floor lower to 8 above highest adjacent floor.
        36 => {
            let tag = level.linedefs[linedef_idx].tag;
            let per_sector: Vec<(usize, i16)> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, highest_adjacent_floor(level, i) + 8))
                .collect();
            for (idx, target) in per_sector {
                activate_floor_lower_single(gs, level, idx, tag, target, 1);
            }
        }

        // Type 56: Floor raise to 8 below lowest adjacent ceiling (crush).
        56 => {
            let tag = level.linedefs[linedef_idx].tag;
            let per_sector: Vec<(usize, i16)> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| (i, lowest_adjacent_ceiling(level, i) - 8))
                .collect();
            for (idx, target) in per_sector {
                activate_floor_raise_single(gs, level, idx, tag, target, 1, true);
            }
        }

        _ => {
            // Unknown special — silently ignored.
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the segment from `(ax, ay)` to `(bx, by)` crosses the
/// infinite line defined by `(lx1, ly1)` → `(lx2, ly2)`.
///
/// Uses the cross-product (sign) test: the segment crosses the line when the
/// two endpoints lie on opposite sides.
fn segment_crosses_line(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    lx1: i32,
    ly1: i32,
    lx2: i32,
    ly2: i32,
) -> bool {
    // Direction of the linedef.
    let ldx = (lx2 - lx1) as i64;
    let ldy = (ly2 - ly1) as i64;

    // Cross products of linedef direction with each segment endpoint.
    let c1 = ldx * (ay - ly1) as i64 - ldy * (ax - lx1) as i64;
    let c2 = ldx * (by - ly1) as i64 - ldy * (bx - lx1) as i64;

    // Different signs (XOR of sign bits < 0) means the segment straddles the line.
    (c1 ^ c2) < 0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind};
    use crate::state::GameState;
    use doom_types::{Bam, Fixed16_16};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Build a minimal 1×1 blockmap identical to the one in `movement.rs` tests.
    fn make_minimal_blockmap() -> doom_map::Blockmap {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count
        // offset table: block 0 is at word-offset 5 from start of lump.
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes()); // sentinel
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
        doom_map::Blockmap::parse_lump(&bm_data).unwrap()
    }

    /// Build a level with one sector that has the given special, no linedefs.
    fn make_damage_level(floor_height: i16, special: u16) -> doom_map::Level {
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
                special,
                tag: 0,
            }],
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build an actor with the given floor height as z coordinate.
    fn make_actor_at_z(gs: &mut GameState, floor_height: i32) -> MobjHandle {
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.z = Fixed16_16::from_int(floor_height);
        gs.mobjslab.alloc(mo)
    }

    /// Build a two-sided linedef + supporting geometry for door tests.
    ///
    /// Layout:
    /// - Sector 0: front sector (actor is here), floor=0 ceil=128.
    /// - Sector 1: back sector (the door cavity), floor=0 ceil=`door_ceil`.
    /// - Vertex 0: (0, -10) — linedef from-vertex.
    /// - Vertex 1: (0, 10)  — linedef to-vertex (vertical wall at x=0).
    /// - Sidedef 0: right side → sector 0.
    /// - Sidedef 1: left side  → sector 1 (the door).
    /// - Linedef 0: two-sided (FLAG_TWO_SIDED=4), special=`special`, right=0, left=1.
    fn make_door_level_with_special(door_ceil: i16, special: u16) -> doom_map::Level {
        // Reject for 2 sectors: ceil(4/8) = 1 byte.
        let reject = doom_map::Reject::parse_lump(&[0u8], 2).unwrap();

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
                ceil_height: door_ceil,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: -10 }, // v0
            doom_map::Vertex { x: 0, y: 10 },  // v1
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

        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004, // FLAG_TWO_SIDED
            special,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Convenience: door level with special=1 (the original helper).
    fn make_door_level(door_ceil: i16) -> doom_map::Level {
        make_door_level_with_special(door_ceil, 1)
    }

    // -----------------------------------------------------------------------
    // Tests: tick_sector_specials
    // -----------------------------------------------------------------------

    #[test]
    fn damage_floor_hurts_actor_standing_on_it() {
        let mut gs = GameState::new("TEST");
        let level = make_damage_level(0, 5); // special 5 = lava, floor=0
        let handle = make_actor_at_z(&mut gs, 0); // z matches floor_height

        tick_sector_specials(&mut gs, &level, handle);

        let health = gs.mobjslab.get(handle).unwrap().health;
        assert_eq!(health, 90, "lava (special 5) must deal 10 damage per tic");
    }

    #[test]
    fn damage_floor_ignores_actor_above_it() {
        let mut gs = GameState::new("TEST");
        let level = make_damage_level(0, 5); // lava at floor=0
        let handle = make_actor_at_z(&mut gs, 10); // actor z=10, not on the floor

        tick_sector_specials(&mut gs, &level, handle);

        let health = gs.mobjslab.get(handle).unwrap().health;
        assert_eq!(health, 100, "actor above lava floor must take no damage");
    }

    // -----------------------------------------------------------------------
    // Tests: p_use_lines
    // -----------------------------------------------------------------------

    #[test]
    fn p_use_lines_activates_nearest_linedef() {
        let mut gs = GameState::new("TEST");
        // Closed door: ceil == floor (0).
        let mut level = make_door_level(0);

        // Place actor at (-32, 0) facing East (Bam::ZERO → trig fallback, ray = east).
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(-32),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        let handle = gs.mobjslab.alloc(mo);

        // Before: door is closed (ceil == floor == 0).
        assert_eq!(level.sectors[1].ceil_height, 0);

        p_use_lines(&mut gs, &mut level, handle);

        // After: door opened (ceil = floor + 128 = 128).
        assert_eq!(
            level.sectors[1].ceil_height, 128,
            "p_use_lines must open the door sector"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: activate_linedef (door toggle — type 1)
    // -----------------------------------------------------------------------

    #[test]
    fn door_toggle_opens_closed_door() {
        let mut gs = GameState::new("TEST");
        // Door sector has ceil == floor (closed).
        let mut level = make_door_level(0);

        assert_eq!(level.sectors[1].ceil_height, 0, "precondition: door closed");

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            level.sectors[1].ceil_height, 128,
            "activate_linedef must open a closed door to floor + 128"
        );
    }

    #[test]
    fn door_toggle_closes_open_door() {
        let mut gs = GameState::new("TEST");
        // Door sector has ceil == floor + 128 (open).
        let mut level = make_door_level(128);

        assert_eq!(level.sectors[1].ceil_height, 128, "precondition: door open");

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            level.sectors[1].ceil_height, 0,
            "activate_linedef must close an open door to floor height"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: animated doors (type 2)
    // -----------------------------------------------------------------------

    #[test]
    fn animated_door_opens_over_time() {
        let mut gs = GameState::new("TEST");
        // Type 2: open door, stays open. Start fully closed (ceil == floor == 0).
        let mut level = make_door_level_with_special(0, 2);

        assert_eq!(level.sectors[1].ceil_height, 0, "precondition: door closed");
        assert!(gs.active_doors.is_empty());

        // Activate the linedef — enqueues a DoorMover.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_doors.len(), 1, "DoorMover should be enqueued");

        // Tick doors several times — ceiling should rise.
        let initial_ceil = level.sectors[1].ceil_height;
        for _ in 0..10 {
            tick_doors(&mut gs, &mut level);
        }
        assert!(
            level.sectors[1].ceil_height > initial_ceil,
            "ceiling must rise after ticking doors"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: locked doors
    // -----------------------------------------------------------------------

    #[test]
    fn locked_door_blocked_without_key() {
        let mut gs = GameState::new("TEST");
        // Type 26: blue key required.
        let mut level = make_door_level_with_special(0, 26);

        // Player has no blue key.
        assert!(!gs.player.has_key(crate::player::KEY_BLUE_CARD));
        assert!(!gs.player.has_key(crate::player::KEY_BLUE_SKULL));

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            gs.active_doors.is_empty(),
            "door must not open without blue key"
        );
    }

    #[test]
    fn locked_door_opens_with_blue_card() {
        let mut gs = GameState::new("TEST");
        // Type 26: blue key required.
        let mut level = make_door_level_with_special(0, 26);

        // Give player the blue card.
        gs.player.give_key(crate::player::KEY_BLUE_CARD);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            gs.active_doors.len(),
            1,
            "door must open when player has blue card"
        );
    }

    #[test]
    fn locked_door_opens_with_blue_skull() {
        let mut gs = GameState::new("TEST");
        // Type 26: blue key required (skull is equivalent).
        let mut level = make_door_level_with_special(0, 26);

        // Give player the blue skull.
        gs.player.give_key(crate::player::KEY_BLUE_SKULL);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            gs.active_doors.len(),
            1,
            "door must open when player has blue skull"
        );
    }

    #[test]
    fn yellow_locked_door_blocked_without_key() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 27);

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            gs.active_doors.is_empty(),
            "yellow door must not open without yellow key"
        );
    }

    #[test]
    fn red_locked_door_blocked_without_key() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 28);

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            gs.active_doors.is_empty(),
            "red door must not open without red key"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: spawn_level_specials / tick_lights
    // -----------------------------------------------------------------------

    #[test]
    fn spawn_level_specials_creates_light_thinker() {
        let mut gs = GameState::new("TEST");
        // One sector with special=1 (random off blinking light).
        let level = make_damage_level(0, 1);

        spawn_level_specials(&mut gs, &level);

        assert_eq!(
            gs.active_lights.len(),
            1,
            "spawn_level_specials must create one light thinker for special=1"
        );
    }

    #[test]
    fn light_toggles_after_period() {
        let mut gs = GameState::new("TEST");
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).unwrap();
        let mut level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 2, // fast strobe
                tag: 0,
            }],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        spawn_level_specials(&mut gs, &level);
        assert_eq!(gs.active_lights.len(), 1);

        // Tick past the period — light should toggle.
        let initial_light = level.sectors[0].light_level;
        for _ in 0..BLINK_FAST_PERIOD {
            tick_lights(&mut gs, &mut level);
        }
        // After one full period, light should have toggled to dark.
        assert_ne!(
            level.sectors[0].light_level, initial_light,
            "light must toggle after one period"
        );
    }

    // -----------------------------------------------------------------------
    // Helpers for crusher / lift / floor tests
    // -----------------------------------------------------------------------

    /// Build a level with multiple sectors connected by two-sided linedefs.
    ///
    /// Layout: 3 sectors, 2 two-sided linedefs connecting them in a chain.
    /// - Sector 0: floor=`floors[0]`, ceil=`ceils[0]`, tag=`tags[0]`
    /// - Sector 1: floor=`floors[1]`, ceil=`ceils[1]`, tag=`tags[1]`
    /// - Sector 2: floor=`floors[2]`, ceil=`ceils[2]`, tag=`tags[2]`
    /// - Linedef 0 connects sector 0 and sector 1 (special=`ld_special`, tag=`ld_tag`)
    /// - Linedef 1 connects sector 1 and sector 2 (special=0, tag=0)
    fn make_multi_sector_level(
        floors: [i16; 3],
        ceils: [i16; 3],
        tags: [u16; 3],
        ld_special: u16,
        ld_tag: u16,
    ) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8; 2], 3).unwrap();

        let sectors = vec![
            doom_map::Sector {
                floor_height: floors[0],
                ceil_height: ceils[0],
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: tags[0],
            },
            doom_map::Sector {
                floor_height: floors[1],
                ceil_height: ceils[1],
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: tags[1],
            },
            doom_map::Sector {
                floor_height: floors[2],
                ceil_height: ceils[2],
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: tags[2],
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
            doom_map::Vertex { x: 128, y: 0 },
            doom_map::Vertex { x: 192, y: 0 },
        ];

        // Sidedef pairs for two linedefs.
        let sidedefs = vec![
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
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 2,
            },
        ];

        let linedefs = vec![
            doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004, // FLAG_TWO_SIDED
                special: ld_special,
                tag: ld_tag,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            doom_map::Linedef {
                from_vertex: 1,
                to_vertex: 2,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 2,
                left_sidedef: 3,
            },
        ];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build a simple level with one sector for crusher/floor tests.
    /// The sector has the given tag, and a linedef triggers it.
    fn make_tagged_sector_level(
        floor: i16,
        ceil: i16,
        tag: u16,
        ld_special: u16,
    ) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8; 1], 2).unwrap();

        let sectors = vec![
            // Sector 0: front sector (where the player stands).
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            // Sector 1: the target sector being moved.
            doom_map::Sector {
                floor_height: floor,
                ceil_height: ceil,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag,
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: -10 },
            doom_map::Vertex { x: 0, y: 10 },
        ];

        let sidedefs = vec![
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
        ];

        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: ld_special,
            tag,
            right_sidedef: 0,
            left_sidedef: 1,
        }];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    // -----------------------------------------------------------------------
    // Tests: adjacent sector height helpers
    // -----------------------------------------------------------------------

    #[test]
    fn lowest_adjacent_floor_finds_minimum() {
        // Sector 0: floor=0, Sector 1: floor=64, Sector 2: floor=32
        // Sector 1 is adjacent to both 0 and 2.
        let level = make_multi_sector_level([0, 64, 32], [128, 128, 128], [0, 0, 0], 0, 0);

        // Adjacent to sector 1: sectors 0 (floor=0) and 2 (floor=32).
        let result = lowest_adjacent_floor(&level, 1);
        assert_eq!(
            result, 0,
            "lowest adjacent floor to sector 1 should be 0 (sector 0)"
        );
    }

    #[test]
    fn highest_adjacent_floor_finds_maximum() {
        let level = make_multi_sector_level([10, 64, 50], [128, 128, 128], [0, 0, 0], 0, 0);

        // Adjacent to sector 1: sectors 0 (floor=10) and 2 (floor=50).
        let result = highest_adjacent_floor(&level, 1);
        assert_eq!(
            result, 50,
            "highest adjacent floor to sector 1 should be 50 (sector 2)"
        );
    }

    #[test]
    fn next_highest_floor_finds_next_step() {
        // Sector 1: floor=0, adjacent to sector 0 (floor=32) and sector 2 (floor=64).
        let level = make_multi_sector_level([32, 0, 64], [128, 128, 128], [0, 0, 0], 0, 0);

        let result = next_highest_floor(&level, 1);
        assert_eq!(result, 32, "next highest floor above 0 should be 32");
    }

    #[test]
    fn next_highest_floor_no_higher_returns_own() {
        // Sector 1: floor=100, adjacent floors are 20 and 50 (both lower).
        let level = make_multi_sector_level([20, 100, 50], [200, 200, 200], [0, 0, 0], 0, 0);

        let result = next_highest_floor(&level, 1);
        assert_eq!(result, 100, "no higher floor => returns own floor");
    }

    #[test]
    fn lowest_adjacent_ceiling_finds_minimum() {
        // Sector 1 adjacent to sector 0 (ceil=128) and sector 2 (ceil=96).
        let level = make_multi_sector_level([0, 0, 0], [128, 200, 96], [0, 0, 0], 0, 0);

        let result = lowest_adjacent_ceiling(&level, 1);
        assert_eq!(
            result, 96,
            "lowest adjacent ceiling to sector 1 should be 96 (sector 2)"
        );
    }

    #[test]
    fn lowest_adjacent_floor_no_neighbors_returns_own() {
        // A sector with no linedefs connecting to others.
        let level = make_damage_level(42, 0);
        let result = lowest_adjacent_floor(&level, 0);
        assert_eq!(result, 42, "no adjacent sectors => returns own floor");
    }

    // -----------------------------------------------------------------------
    // Tests: CeilingMover (crushers)
    // -----------------------------------------------------------------------

    #[test]
    fn crusher_oscillates_between_heights() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 6);

        // Activate line type 6 (fast crusher, perpetual).
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_ceilings.len(), 1, "one crusher must be created");

        let initial_ceil = level.sectors[1].ceil_height;
        assert_eq!(initial_ceil, 128);

        // Tick until ceiling descends: 128 to 8 = 120 units / speed 2 = 60 tics.
        for _ in 0..60 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(
            level.sectors[1].ceil_height, 8,
            "ceiling must reach bottom_height"
        );

        // Should have reversed to Up.
        assert_eq!(
            gs.active_ceilings[0].direction,
            MoveDirection::Up,
            "crusher must reverse to Up after hitting bottom"
        );

        // Tick until it returns to the top: 120 units / speed 2 = 60 tics.
        for _ in 0..60 {
            tick_ceilings(&mut gs, &mut level);
        }

        // Should have reached top and reversed back to Down (perpetual).
        assert_eq!(
            gs.active_ceilings[0].direction,
            MoveDirection::Down,
            "perpetual crusher must reverse to Down after reaching top"
        );
        assert_eq!(
            level.sectors[1].ceil_height, 128,
            "crusher must return to top_height"
        );

        // Verify perpetual: tick a few more — should descend again.
        for _ in 0..5 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(
            level.sectors[1].ceil_height, 118,
            "perpetual crusher must continue oscillating"
        );
    }

    #[test]
    fn one_shot_crusher_removes_itself() {
        let mut gs = GameState::new("TEST");
        // Line type 44: one-shot ceiling lower (remove_when_done=true).
        let mut level = make_tagged_sector_level(0, 128, 1, 44);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_ceilings.len(), 1);

        // Tick down to bottom_height (8). 128 -> 8 = 120 units / speed 2 = 60 tics.
        for _ in 0..60 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].ceil_height, 8);

        // Reverses to Up; tick back to 128. 120 / 2 = 60 tics.
        for _ in 0..60 {
            tick_ceilings(&mut gs, &mut level);
        }

        // One-shot crusher should be removed when reaching top.
        assert!(
            gs.active_ceilings.is_empty(),
            "one-shot crusher must remove itself after returning to top"
        );
        assert_eq!(level.sectors[1].ceil_height, 128);
    }

    #[test]
    fn crusher_stops_on_line_type_57() {
        let mut gs = GameState::new("TEST");
        // Start a crusher with tag=5.
        let mut level = make_tagged_sector_level(0, 128, 5, 6);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_ceilings.len(), 1);

        // Tick a few times.
        for _ in 0..5 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert!(
            !gs.active_ceilings.is_empty(),
            "crusher should still be running"
        );

        // Now add a linedef with special 57 and the same tag.
        level.linedefs.push(doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 57,
            tag: 5,
            right_sidedef: 0,
            left_sidedef: 1,
        });

        // Activate line type 57 (stop crusher).
        let stop_idx = level.linedefs.len() - 1;
        activate_linedef(&mut gs, &mut level, stop_idx);

        assert!(
            gs.active_ceilings.is_empty(),
            "line type 57 must stop all crushers with matching tag"
        );
    }

    #[test]
    fn slow_crusher_type_25_speed_1() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 25);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_ceilings.len(), 1);
        assert_eq!(
            gs.active_ceilings[0].speed, 1,
            "type 25 must use speed 1 (slow)"
        );

        // Tick once — should move by 1.
        tick_ceilings(&mut gs, &mut level);
        assert_eq!(level.sectors[1].ceil_height, 127);
    }

    // -----------------------------------------------------------------------
    // Tests: FloorMover (lifts)
    // -----------------------------------------------------------------------

    #[test]
    fn lift_lower_wait_raise() {
        let mut gs = GameState::new("TEST");

        // Sector 0: floor=0 (front), Sector 1: floor=64 (target, tag=1).
        // Sector 0 is adjacent to sector 1 with floor=0 → lowest adjacent = 0.
        let mut level = make_tagged_sector_level(64, 128, 1, 62);

        assert_eq!(level.sectors[1].floor_height, 64, "precondition: floor=64");

        // Activate line type 62 (lift lower-wait-raise, speed 4).
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1, "one floor mover must be created");

        // Lowest adjacent floor is sector 0's floor = 0.
        assert_eq!(
            gs.active_floors[0].target_height, 0,
            "lift target = lowest adjacent = 0"
        );
        assert_eq!(
            gs.active_floors[0].return_height, 64,
            "return height = original floor"
        );

        // Tick until floor lowers to 0: 64 units / speed 4 = 16 tics.
        for _ in 0..16 {
            tick_floors(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].floor_height, 0, "floor must lower to 0");

        // Should now be in wait phase.
        assert!(gs.active_floors[0].waiting, "lift must enter wait phase");
        assert_eq!(
            gs.active_floors[0].wait_remaining, LIFT_WAIT,
            "wait_remaining must be set to LIFT_WAIT"
        );

        // Tick through the wait phase (105 tics).
        for _ in 0..LIFT_WAIT {
            tick_floors(&mut gs, &mut level);
        }
        assert!(!gs.active_floors[0].waiting, "wait phase must end");

        // Should now be heading back up to return_height (64).
        // 64 units / speed 4 = 16 tics.
        for _ in 0..16 {
            tick_floors(&mut gs, &mut level);
        }
        assert_eq!(
            level.sectors[1].floor_height, 64,
            "floor must raise back to 64"
        );

        // Lift should be removed after returning.
        assert!(
            gs.active_floors.is_empty(),
            "lift must remove itself after return"
        );
    }

    #[test]
    fn turbo_lift_type_121_speed_8() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(64, 128, 1, 121);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1);
        assert_eq!(
            gs.active_floors[0].speed, 8,
            "type 121 must use speed 8 (turbo)"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: FloorMover (floor raisers / lowerers)
    // -----------------------------------------------------------------------

    #[test]
    fn floor_raise_to_next_highest() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=32, Sector 1: floor=0 (target, tag=1), Sector 2: floor=64.
        // Sector 1 is adjacent to 0 (floor=32) and 2 (floor=64).
        // next_highest_floor above 0 = 32.
        let mut level = make_multi_sector_level(
            [32, 0, 64],
            [128, 128, 128],
            [0, 1, 0],
            18, // type 18: floor raise to next highest adjacent floor
            1,  // tag 1 targets sector 1
        );

        assert_eq!(level.sectors[1].floor_height, 0);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1);
        assert_eq!(
            gs.active_floors[0].target_height, 32,
            "target = next highest floor = 32"
        );

        // Tick until floor reaches 32: 32 units / speed 1 = 32 tics.
        for _ in 0..32 {
            tick_floors(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].floor_height, 32, "floor must reach 32");

        // One-shot raiser should be removed.
        assert!(
            gs.active_floors.is_empty(),
            "one-shot floor raiser must remove itself"
        );
    }

    #[test]
    fn floor_lower_to_lowest_adjacent() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=0, Sector 1: floor=64 (target, tag=1), Sector 2: floor=32.
        // Sector 1 adjacent to 0 (floor=0) and 2 (floor=32).
        // lowest_adjacent_floor = 0.
        let mut level = make_multi_sector_level(
            [0, 64, 32],
            [128, 128, 128],
            [0, 1, 0],
            23, // type 23: floor lower to lowest adjacent
            1,
        );

        assert_eq!(level.sectors[1].floor_height, 64);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1);
        assert_eq!(
            gs.active_floors[0].target_height, 0,
            "target = lowest adjacent = 0"
        );

        // Tick until floor reaches 0: 64 units / speed 1 = 64 tics.
        for _ in 0..64 {
            tick_floors(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].floor_height, 0, "floor must lower to 0");

        assert!(
            gs.active_floors.is_empty(),
            "one-shot floor lowerer must remove itself"
        );
    }

    #[test]
    fn floor_raise_to_lowest_ceiling_type_5() {
        let mut gs = GameState::new("TEST");
        // Sector 0: ceil=128, Sector 1: floor=0 ceil=200 tag=1, Sector 2: ceil=96.
        // Sector 1 adjacent to 0 (ceil=128) and 2 (ceil=96).
        // lowest_adjacent_ceiling = 96.
        let mut level = make_multi_sector_level(
            [0, 0, 0],
            [128, 200, 96],
            [0, 1, 0],
            5, // type 5: floor raise to lowest adjacent ceiling
            1,
        );

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1);
        assert_eq!(
            gs.active_floors[0].target_height, 96,
            "target = lowest adjacent ceiling = 96"
        );
        assert!(gs.active_floors[0].crush, "type 5 must have crush=true");
    }

    #[test]
    fn floor_lower_to_highest_adjacent_type_19() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=10, Sector 1: floor=64 tag=1, Sector 2: floor=48.
        // highest_adjacent_floor = 48.
        let mut level = make_multi_sector_level(
            [10, 64, 48],
            [128, 128, 128],
            [0, 1, 0],
            19, // type 19: floor lower to highest adjacent floor
            1,
        );

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1);
        assert_eq!(
            gs.active_floors[0].target_height, 48,
            "target = highest adjacent = 48"
        );
    }

    #[test]
    fn floor_lower_type_36_to_8_above_highest() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=10, Sector 1: floor=64 tag=1, Sector 2: floor=30.
        // highest_adjacent_floor = 30, target = 30 + 8 = 38.
        let mut level = make_multi_sector_level(
            [10, 64, 30],
            [128, 128, 128],
            [0, 1, 0],
            36, // type 36
            1,
        );

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1);
        assert_eq!(
            gs.active_floors[0].target_height, 38,
            "target = highest_adj(30) + 8 = 38"
        );
    }

    #[test]
    fn floor_raise_type_56_to_8_below_lowest_ceiling() {
        let mut gs = GameState::new("TEST");
        // Sector 0: ceil=128, Sector 1: floor=0 ceil=200 tag=1, Sector 2: ceil=100.
        // lowest_adjacent_ceiling = 100, target = 100 - 8 = 92.
        let mut level = make_multi_sector_level([0, 0, 0], [128, 200, 100], [0, 1, 0], 56, 1);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1);
        assert_eq!(
            gs.active_floors[0].target_height, 92,
            "target = lowest_adj_ceil(100) - 8 = 92"
        );
        assert!(gs.active_floors[0].crush, "type 56 must have crush=true");
    }

    // -----------------------------------------------------------------------
    // Tests: duplicate mover prevention
    // -----------------------------------------------------------------------

    #[test]
    fn duplicate_crusher_prevented() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 6);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_ceilings.len(), 1);

        // Try to activate again — should not add a duplicate.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.active_ceilings.len(),
            1,
            "must not create duplicate crushers"
        );
    }

    #[test]
    fn duplicate_lift_prevented() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(64, 128, 1, 62);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_floors.len(), 1, "must not create duplicate lifts");
    }

    // -----------------------------------------------------------------------
    // Tests: GameState clone includes new fields
    // -----------------------------------------------------------------------

    #[test]
    fn game_state_clone_includes_ceilings_and_floors() {
        let mut gs = GameState::new("TEST");
        gs.active_ceilings.push(CeilingMover {
            sector_index: 0,
            top_height: 128,
            bottom_height: 8,
            speed: 2,
            crush_damage: 10,
            direction: MoveDirection::Down,
            silent: false,
            remove_when_done: false,
            tag: 1,
        });
        gs.active_floors.push(FloorMover {
            sector_index: 0,
            target_height: 0,
            speed: 4,
            direction: MoveDirection::Down,
            wait_tics: 105,
            return_height: 64,
            waiting: false,
            wait_remaining: 0,
            crush: false,
            tag: 1,
        });

        let gs2 = gs.clone();
        assert_eq!(gs2.active_ceilings.len(), 1, "clone must include ceilings");
        assert_eq!(gs2.active_floors.len(), 1, "clone must include floors");
        assert_eq!(gs2.active_ceilings[0].top_height, 128);
        assert_eq!(gs2.active_floors[0].target_height, 0);
    }

    // -----------------------------------------------------------------------
    // Tests: exit line types (11, 51, 52, 124)
    // -----------------------------------------------------------------------

    #[test]
    fn exit_type_11_sets_normal_exit() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 11);

        assert_eq!(gs.exit_request, None, "precondition: no exit request");
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Normal),
            "type 11 must set ExitRequest::Normal"
        );
    }

    #[test]
    fn exit_type_51_sets_secret_exit() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 51);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Secret),
            "type 51 must set ExitRequest::Secret"
        );
    }

    #[test]
    fn exit_type_52_walk_sets_normal_exit() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 52);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Normal),
            "type 52 (walk trigger) must set ExitRequest::Normal"
        );
    }

    #[test]
    fn exit_type_124_walk_sets_secret_exit() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 124);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Secret),
            "type 124 (walk trigger) must set ExitRequest::Secret"
        );
    }

    #[test]
    fn exit_request_is_none_by_default() {
        let gs = GameState::new("TEST");
        assert_eq!(
            gs.exit_request, None,
            "exit_request must be None on creation"
        );
    }
}
