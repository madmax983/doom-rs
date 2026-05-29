#![allow(unused_imports)]
#![allow(dead_code)]
use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

use super::*;
use crate::mobj::MobjHandle;
use crate::movers::*;
use crate::state::*;

// tick_ceilings (crushers)

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
///
/// # Arguments
///
/// * `gs` - Game state containing active ceilings.
/// * `level` - Level to mutate sector ceilings in.
///
/// # Examples
///
/// ```
/// use doom_game::specials::tick_ceilings;
/// // tick_ceilings(&mut gs, &mut level);
/// ```
pub fn tick_ceilings(gs: &mut GameState, level: &mut Level) {
    let mut active_ceilings = std::mem::take(&mut gs.movers.active_ceilings);

    active_ceilings.retain_mut(|ceiling| {
        let sector_idx = ceiling.sector_index;
        let speed = ceiling.speed;
        let direction = ceiling.direction;
        let top = ceiling.top_height;
        let bottom = ceiling.bottom_height;
        let crush_dmg = ceiling.crush_damage;
        let remove_when_done = ceiling.remove_when_done;
        let normal_speed = ceiling.normal_speed;
        let ceiling_type = ceiling.ceiling_type;

        if sector_idx >= level.sectors.len() {
            return false;
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
                            gs.damage_player(crush_dmg);
                        }
                    }

                    // Slow down to speed 1 when crushing (CrushAndRaise / SilentCrush).
                    match ceiling_type {
                        CeilingType::CrushAndRaise | CeilingType::SilentCrush => {
                            ceiling.speed = 1;
                        }
                        _ => {}
                    }
                }

                if ceil <= bottom {
                    level.sectors[sector_idx].ceil_height = bottom;
                    match ceiling_type {
                        CeilingType::LowerToFloor | CeilingType::LowerAndCrush => {
                            // One-shot types: remove when done.
                            return false;
                        }
                        _ => {
                            // Perpetual types: reverse to Up.
                            ceiling.direction = MoveDirection::Up;
                        }
                    }
                }
            }
            MoveDirection::Up => {
                // Resume normal speed when going up.
                ceiling.speed = normal_speed;

                level.sectors[sector_idx].ceil_height += normal_speed;
                let ceil = level.sectors[sector_idx].ceil_height;

                if ceil >= top {
                    level.sectors[sector_idx].ceil_height = top;
                    if remove_when_done {
                        return false;
                    }
                    // Perpetual: reverse back to Down.
                    ceiling.direction = MoveDirection::Down;
                }
            }
        }

        true
    });

    gs.movers.active_ceilings = active_ceilings;
}

// Public ceiling activation functions

/// Activate a CrushAndRaise ceiling on all sectors matching `tag`.
///
/// Perpetual crusher: lowers to floor+8, reverses, raises to top, reverses, repeat.
/// Deals 10 damage per tic when crushing.
pub fn ev_ceiling_crush_and_raise(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    activate_crusher(
        gs,
        level,
        tag,
        CrusherParams {
            speed,
            crush_damage: 10,
            silent: false,
            remove_when_done: false,
            ceiling_type: CeilingType::CrushAndRaise,
        },
    );
}

/// Activate a LowerAndCrush ceiling on all sectors matching `tag`.
///
/// One-shot: lowers to floor+8 then stops. No crush damage.
pub fn ev_ceiling_lower_and_crush(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    activate_crusher(
        gs,
        level,
        tag,
        CrusherParams {
            speed,
            crush_damage: 0,
            silent: false,
            remove_when_done: true,
            ceiling_type: CeilingType::LowerAndCrush,
        },
    );
}

/// Activate a LowerToFloor ceiling on all sectors matching `tag`.
///
/// One-shot: lowers to floor height then stops. No crush damage.
pub fn ev_ceiling_lower_to_floor(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    activate_crusher(
        gs,
        level,
        tag,
        CrusherParams {
            speed,
            crush_damage: 0,
            silent: false,
            remove_when_done: true,
            ceiling_type: CeilingType::LowerToFloor,
        },
    );
}

/// Stop all crushers with matching `tag` by removing them.
///
/// Used by line types 57 and 74.
pub fn ev_ceiling_crush_stop(gs: &mut GameState, tag: u16) {
    stop_crushers(gs, tag);
}

/// Activate a FastCrushAndRaise ceiling on all sectors matching `tag`.
///
/// Like CrushAndRaise but typically with higher speed. Deals 10 damage per tic.
pub fn ev_ceiling_crush_raise_fast(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    activate_crusher(
        gs,
        level,
        tag,
        CrusherParams {
            speed,
            crush_damage: 10,
            silent: false,
            remove_when_done: false,
            ceiling_type: CeilingType::FastCrushAndRaise,
        },
    );
}

/// Raise ceiling on all sectors matching `tag` to the highest adjacent ceiling.
///
/// Creates a one-shot `CeilingMover` with `CeilingType::RaiseToHighest`.
/// Linedef type 40 (W1 Raise ceiling to highest adjacent ceiling).
pub fn ev_ceiling_raise_to_highest(gs: &mut GameState, level: &Level, tag: u16) {
    for sector_idx in 0..level.sectors.len() {
        let sector = &level.sectors[sector_idx];
        if sector.tag != tag {
            continue;
        }
        let top = highest_adjacent_ceiling(level, sector_idx);
        if sector.ceil_height >= top {
            continue;
        }
        if gs
            .movers
            .active_ceilings
            .iter()
            .any(|c| c.sector_index == sector_idx)
        {
            continue;
        }
        gs.movers.active_ceilings.push(CeilingMover {
            sector_index: sector_idx,
            top_height: top,
            bottom_height: sector.ceil_height,
            speed: 2,
            normal_speed: 2,
            crush_damage: 0,
            direction: MoveDirection::Up,
            silent: false,
            remove_when_done: true,
            tag,
            ceiling_type: CeilingType::RaiseToHighest,
        });
    }
}

/// Parameters to spawn a ceiling crusher.
///
/// ## Examples
/// ```
/// # use doom_game::specials::CrusherParams;
/// # use doom_game::state::CeilingType;
/// let params = CrusherParams {
///     speed: 2,
///     crush_damage: 10,
///     silent: false,
///     remove_when_done: false,
///     ceiling_type: CeilingType::CrushAndRaise,
/// };
/// ```
pub struct CrusherParams {
    /// Vertical speed of the ceiling when crushing downward (map units per tic).
    pub speed: i16,
    /// Damage dealt to actors caught under the ceiling when it bottoms out.
    pub crush_damage: i32,
    /// If true, the crusher does not play standard movement sounds.
    pub silent: bool,
    /// If true, the crusher is removed from the active mover list after one cycle.
    pub remove_when_done: bool,
    /// The state-machine type controlling the oscillation/raising pattern.
    pub ceiling_type: CeilingType,
}

pub fn activate_crusher(gs: &mut GameState, level: &Level, tag: u16, params: CrusherParams) {
    for idx in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
    {
        // Avoid duplicate crushers on the same sector.
        if gs
            .movers
            .active_ceilings
            .iter()
            .any(|c| c.sector_index == idx)
        {
            continue;
        }
        let sector = &level.sectors[idx];
        let bottom = match params.ceiling_type {
            CeilingType::LowerToFloor => sector.floor_height,
            _ => sector.floor_height + 8,
        };
        gs.movers.active_ceilings.push(CeilingMover {
            sector_index: idx,
            top_height: sector.ceil_height,
            bottom_height: bottom,
            speed: params.speed,
            normal_speed: params.speed,
            crush_damage: params.crush_damage,
            direction: MoveDirection::Down,
            silent: params.silent,
            remove_when_done: params.remove_when_done,
            tag,
            ceiling_type: params.ceiling_type,
        });
    }
}

/// Stop all crushers matching `tag` (line type 57).
fn stop_crushers(gs: &mut GameState, tag: u16) {
    gs.movers.active_ceilings.retain(|c| c.tag != tag);
}

/// Activate a lift (lower-wait-raise) on all sectors matching `tag`.
pub fn activate_lift(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for idx in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
    {
        // Avoid duplicate floor movers on the same sector.
        if gs
            .movers
            .active_floors
            .iter()
            .any(|f| f.sector_index == idx)
        {
            continue;
        }
        let sector = &level.sectors[idx];
        let low = lowest_adjacent_floor(level, idx);
        gs.movers.active_floors.push(FloorMover {
            sector_index: idx,
            target_height: low,
            speed,
            direction: MoveDirection::Down,
            wait_tics: LIFT_WAIT,
            return_height: sector.floor_height,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::NoCrush,
            tag,
            floor_type: FloorType::LowerToLowest,
        });
    }
}
