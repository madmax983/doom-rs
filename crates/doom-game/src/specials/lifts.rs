use super::*;
use crate::state::*;
use doom_map::Level;

// ---------------------------------------------------------------------------
// Perpetual platforms
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// LiftMover activation and tick
// ---------------------------------------------------------------------------

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
