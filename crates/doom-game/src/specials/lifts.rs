#![allow(unused_imports)]
#![allow(dead_code)]
use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

use super::*;
use crate::mobj::MobjHandle;
use crate::movers::*;
use crate::state::*;

// LiftMover activation and tick

/// Activate a lift (lower-wait-raise) on all sectors matching `tag` using the
/// dedicated `LiftMover` system.
///
/// For each matching sector, computes the lowest adjacent floor height as
/// `low_height`, stores the current floor as `high_height`, and creates a
/// `LiftMover` starting in `Lowering` status.
///
/// Returns the number of lifts created.
pub fn ev_do_lift(
    gs: &mut GameState,
    level: &Level,
    tag: u16,
    speed: i16,
    wait_tics: i32,
) -> usize {
    let mut count = 0;
    for idx in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
    {
        // Avoid duplicate lifts on the same sector.
        if gs.movers.lifts.iter().any(|l| l.sector_index == idx) {
            continue;
        }
        let sector = &level.sectors[idx];
        let low = lowest_adjacent_floor(level, idx);
        gs.movers.lifts.push(LiftMover {
            sector_index: idx,
            low_height: low,
            high_height: sector.floor_height,
            speed,
            wait_tics,
            wait_remaining: 0,
            status: LiftStatus::Lowering,
        });
        count += 1;
    }
    count
}

/// Advance all active lifts by one tic.
///
/// Call this once per tic from `tick()`.
///
/// Lift cycle:
/// 1. `Lowering`: move floor down by `speed`. When `floor <= low_height`,
///    snap to `low_height`, transition to `Waiting`, set `wait_remaining`.
/// 2. `Waiting`: decrement `wait_remaining`. When 0, transition to `Raising`.
/// 3. `Raising`: move floor up by `speed`. When `floor >= high_height`,
///    snap to `high_height`, transition to `Done`.
/// 4. `Done`: remove from active list.
pub fn tick_lifts(gs: &mut GameState, level: &mut Level) {
    let mut lifts = std::mem::take(&mut gs.movers.lifts);

    lifts.retain_mut(|lift| {
        let sector_idx = lift.sector_index;
        if sector_idx >= level.sectors.len() {
            return false;
        }

        match lift.status {
            LiftStatus::Lowering => {
                let speed = lift.speed;
                let low = lift.low_height;
                level.sectors[sector_idx].floor_height -= speed;
                if level.sectors[sector_idx].floor_height <= low {
                    level.sectors[sector_idx].floor_height = low;
                    lift.status = LiftStatus::Waiting;
                    lift.wait_remaining = lift.wait_tics;
                }
            }
            LiftStatus::Waiting => {
                lift.wait_remaining -= 1;
                if lift.wait_remaining <= 0 {
                    lift.status = LiftStatus::Raising;
                }
            }
            LiftStatus::Raising => {
                let speed = lift.speed;
                let high = lift.high_height;
                level.sectors[sector_idx].floor_height += speed;
                if level.sectors[sector_idx].floor_height >= high {
                    level.sectors[sector_idx].floor_height = high;
                    lift.status = LiftStatus::Done;
                }
            }
            LiftStatus::Done => {
                return false;
            }
        }

        true
    });

    gs.movers.lifts = lifts;
}

/// Activate a floor raiser (one-shot, no wait) on a single sector with an
/// explicit `FloorType`.
pub fn activate_floor_raise_single_typed(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    tag: u16,
    target_height: i16,
    speed: i16,
    crush: crate::state::CrushBehavior,
    floor_type: FloorType,
) {
    if gs
        .movers
        .active_floors
        .iter()
        .any(|f| f.sector_index == sector_idx)
    {
        return;
    }
    let Some(sector) = level.sectors.get(sector_idx) else {
        return;
    };
    gs.movers.active_floors.push(FloorMover {
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
        floor_type,
    });
}

/// Activate a floor lowerer (one-shot, no wait) on a single sector with an
/// explicit `FloorType`.
pub fn activate_floor_lower_single_typed(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    tag: u16,
    target_height: i16,
    speed: i16,
    floor_type: FloorType,
) {
    if gs
        .movers
        .active_floors
        .iter()
        .any(|f| f.sector_index == sector_idx)
    {
        return;
    }
    let Some(sector) = level.sectors.get(sector_idx) else {
        return;
    };
    gs.movers.active_floors.push(FloorMover {
        sector_index: sector_idx,
        target_height,
        speed,
        direction: MoveDirection::Down,
        wait_tics: -1,
        return_height: sector.floor_height,
        waiting: false,
        wait_remaining: 0,
        crush: crate::state::CrushBehavior::NoCrush,
        tag,
        floor_type,
    });
}
