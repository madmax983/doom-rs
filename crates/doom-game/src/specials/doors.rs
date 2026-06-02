use super::constants::*;
use super::*;
use crate::state::*;
use doom_map::{Level, SIDEDEF_NONE};

// ---------------------------------------------------------------------------
// tick_doors
// ---------------------------------------------------------------------------

/// Advance all active door/floor movers by one tic.
///
/// Call this once per tic from `tick()`.
pub fn tick_doors(gs: &mut GameState, level: &mut Level) {
    const CLOSE_WAIT_OPEN_DELAY: i32 = 1050; // 30 s at 35 Hz

    let mut active_doors = std::mem::take(&mut gs.movers.active_doors);

    active_doors.retain_mut(|door| {
        let sector_idx = door.sector;

        // ── Close-wait-open: waiting at closed position before reopening ──
        if door.reopen_countdown > 0 {
            door.reopen_countdown -= 1;
            return true;
        }
        if door.reopen_countdown == 0 {
            // Delay expired — start reopening.
            let rh = door.reopen_height;
            door.target_height = rh;
            door.speed = DOOR_SPEED; // positive = opening
            door.reopen_height = 0;
            door.reopen_countdown = -1;
            // Fall through to movement logic.
        }

        // ── Standard open-wait-close countdown ──
        let countdown = door.countdown;
        let speed_abs = door.speed.abs();

        if countdown > 0 {
            door.countdown -= 1;
            return true;
        }
        if countdown == 0 {
            // Start closing — negate speed so it moves downward.
            door.speed = -speed_abs;
            if let Some(sector) = level.sectors.get(sector_idx) {
                door.target_height = sector.floor_height;
            }
            door.countdown = -1;
        }

        // ── Move toward target ──
        let speed = door.speed;
        let target = door.target_height;
        let is_ceiling = door.is_ceiling;
        let reopen_height = door.reopen_height;
        let wait_tics = door.wait_tics;

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
                door.current_height = target;

                if speed > 0 && wait_tics > 0 {
                    door.countdown = wait_tics;
                    return true;
                }

                // Close-wait-open: start the reopen delay instead of removing.
                if speed < 0 && reopen_height != 0 {
                    door.reopen_countdown = CLOSE_WAIT_OPEN_DELAY;
                    return true;
                }

                return false;
            }
            let new_h = if is_ceiling {
                level.sectors[sector_idx].ceil_height
            } else {
                level.sectors[sector_idx].floor_height
            };
            door.current_height = new_h;
        }

        true
    });

    gs.movers.active_doors = active_doors;
}

// ---------------------------------------------------------------------------
// Door helpers
// ---------------------------------------------------------------------------

/// Enqueue a door mover that opens and optionally auto-closes.
pub fn open_door(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    behavior: crate::linedef_dispatch::DoorBehavior,
) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;

    let target = lowest_adjacent_ceiling(level, sector_idx) - 4;

    // Avoid duplicate movers for the same sector.
    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }

    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: DOOR_SPEED,
        is_ceiling: true,
        wait_tics: if behavior == crate::linedef_dispatch::DoorBehavior::OpenWaitClose {
            DOOR_WAIT
        } else {
            -1
        },
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}

/// Allow monsters to open ordinary door linedefs without mutating the map.
pub fn monster_activate_door_linedef(
    gs: &mut GameState,
    level: &Level,
    linedef_idx: usize,
) -> bool {
    let Some(ld) = level.linedefs.get(linedef_idx) else {
        return false;
    };

    let auto_close = match ld.special {
        1 | 117 => true,
        31 | 118 => false,
        _ => return false,
    };

    if ld.left_sidedef == SIDEDEF_NONE {
        return false;
    }

    let Some(sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
        return false;
    };
    let sector_idx = sd.sector as usize;
    if sector_idx >= level.sectors.len() {
        return false;
    }

    open_door(
        gs,
        level,
        sector_idx,
        if auto_close {
            crate::linedef_dispatch::DoorBehavior::OpenWaitClose
        } else {
            crate::linedef_dispatch::DoorBehavior::OpenStay
        },
    );
    true
}

/// Enqueue a door mover that closes a door.
pub fn close_door(gs: &mut GameState, level: &Level, sector_idx: usize) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;

    let target = sector.floor_height;

    // Avoid duplicate movers for the same sector.
    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }

    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: -DOOR_SPEED,
        is_ceiling: true,
        wait_tics: -1,
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}

/// Enqueue a door that closes, waits 30 s (1050 tics), then reopens.
///
/// Used by linedef types 16 (W1) and 76 (WR).
pub fn close_wait_open_door(gs: &mut GameState, level: &Level, sector_idx: usize) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;
    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }
    let reopen_h = lowest_adjacent_ceiling(level, sector_idx) - 4;
    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: sector.floor_height,
        current_height: sector.ceil_height,
        speed: -DOOR_SPEED,
        is_ceiling: true,
        wait_tics: -1,
        countdown: -1,
        reopen_height: reopen_h,
        reopen_countdown: -1,
    });
}

/// Enqueue a blazing (fast) door mover that opens and optionally auto-closes.
///
/// Same as `open_door` but with `BLAZING_DOOR_SPEED` (8 units/tic).
pub fn open_blazing_door(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    behavior: crate::linedef_dispatch::DoorBehavior,
) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;

    let target = lowest_adjacent_ceiling(level, sector_idx) - 4;

    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }

    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: BLAZING_DOOR_SPEED,
        is_ceiling: true,
        wait_tics: if behavior == crate::linedef_dispatch::DoorBehavior::OpenWaitClose {
            DOOR_WAIT
        } else {
            -1
        },
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}

/// Enqueue a blazing (fast) door mover that closes a door.
pub fn close_blazing_door(gs: &mut GameState, level: &Level, sector_idx: usize) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;

    let target = sector.floor_height;

    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }

    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: -BLAZING_DOOR_SPEED,
        is_ceiling: true,
        wait_tics: -1,
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}
