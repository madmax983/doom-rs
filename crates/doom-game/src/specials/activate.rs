use super::constants::*;
use super::*;
use crate::mobj::MobjHandle;
use crate::state::*;
use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

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
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let (ax, ay, angle) = (mo.x.to_int(), mo.y.to_int(), mo.angle);

    let cos_raw = i64::from(angle.cos().raw());
    let sin_raw = i64::from(angle.sin().raw());

    // Fall back if trig tables are uninitialised (both return 0).
    let (ahead_x, ahead_y) = if cos_raw == 0 && sin_raw == 0 {
        (ax + USE_RANGE, ay)
    } else {
        (
            ax + ((i64::from(USE_RANGE) * cos_raw) / i64::from(FIXED_ONE.raw())) as i32,
            ay + ((i64::from(USE_RANGE) * sin_raw) / i64::from(FIXED_ONE.raw())) as i32,
        )
    };

    // Find crossed linedefs in front-to-back order. Vanilla Doom traverses all
    // intercepts here: a closed ordinary wall in front of a special must block
    // use instead of letting the player "reach through" it.
    let mut intercepts: smallvec::SmallVec<[(i64, i64, usize); 16]> = smallvec::SmallVec::new();
    for ld_idx in 0..level.linedefs.len() {
        // Collect data while the borrow is immutable; drop before mutable dispatch.
        let (lx1, ly1, lx2, ly2) = {
            let ld = &level.linedefs[ld_idx];
            let v1 = &level.vertexes[ld.from_vertex as usize];
            let v2 = &level.vertexes[ld.to_vertex as usize];
            (v1.x as i32, v1.y as i32, v2.x as i32, v2.y as i32)
        };

        let Some((num, denom)) =
            segment_intersection_frac(ax, ay, ahead_x, ahead_y, lx1, ly1, lx2, ly2)
        else {
            continue;
        };
        intercepts.push((num, denom, ld_idx));
    }

    intercepts.sort_by(|a, b| {
        let lhs = i128::from(a.0) * i128::from(b.1);
        let rhs = i128::from(b.0) * i128::from(a.1);
        lhs.cmp(&rhs)
    });

    for (_, _, ld_idx) in intercepts {
        let (special, blocks_use, use_side) = {
            let linedef = &level.linedefs[ld_idx];
            let blocks_use = match crate::trace::line_opening(level, linedef) {
                None => true,
                Some((open_bottom, open_top)) => open_top <= open_bottom,
            };
            let use_side = crate::sight::point_on_side(
                Fixed16_16::from_int(ax),
                Fixed16_16::from_int(ay),
                ld_idx,
                level,
            ) as u8;
            (linedef.special, blocks_use, use_side)
        };

        // USE key only activates switch-type (S1/SR) triggers.
        use crate::linedef_dispatch::{TriggerType, classify_trigger, dispatch_linedef};
        if let Some(trigger) = classify_trigger(special) {
            if matches!(trigger, TriggerType::SwitchOnce | TriggerType::SwitchRepeat) {
                let activated =
                    dispatch_linedef(gs, level, ld_idx, special, trigger, handle, use_side);
                if activated {
                    crate::switch::toggle_switch_texture(level, ld_idx);
                }
                return;
            }
        }

        if special == 0 && blocks_use {
            gs.sound
                .sound_queue
                .push(crate::state::SoundRequest::PlayerUseFail);
            return;
        }

        if blocks_use {
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
        // Doors
        1 | 2 | 29 | 16 | 76 | 26 | 27 | 28 | 63 | 105 | 106 | 107 | 108 | 109 | 110 | 99 | 133
        | 134 | 135 | 136 | 137 => {
            activate_doors(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Exits
        11 | 51 | 52 | 124 => activate_exits(gs, level, special, left_sidedef as i16, linedef_idx),
        // Ceilings
        6 | 25 | 44 | 49 | 57 | 72 | 73 | 74 | 141 => {
            activate_ceilings(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Lifts
        62 | 66 | 10 | 21 | 88 | 121 | 120 | 122 | 123 => {
            activate_lifts(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Floors
        5 | 14 | 15 | 18 | 20 | 22 | 24 | 30 | 56 | 58 | 59 | 64 | 65 | 67 | 68 | 91 | 92 | 93
        | 94 | 95 | 96 | 19 | 23 | 36 | 37 | 38 | 45 | 60 | 69 | 70 | 71 | 82 | 83 | 84 | 98
        | 102 => activate_floors(gs, level, special, left_sidedef as i16, linedef_idx),
        // Stairs
        7 | 8 | 100 | 127 => activate_stairs(gs, level, special, left_sidedef as i16, linedef_idx),
        // Platforms
        53 | 54 | 87 | 89 => {
            activate_platforms(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Teleports
        39 | 97 | 125 | 126 => {
            activate_teleports(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Misc
        9 | 146 => activate_misc(gs, level, special, left_sidedef as i16, linedef_idx),
        _ => {
            // Unknown special — silently ignored.
        }
    }
}

#[allow(unused_variables)]
pub fn activate_doors(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // --- Type 1: toggle door (immediate, for backward compatibility with existing tests) ---
        1 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
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
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            open_door(
                gs,
                level,
                sector_idx,
                crate::linedef_dispatch::DoorBehavior::OpenStay,
            );
        }

        // --- Type 29: close door (animated) ---
        29 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            close_door(gs, level, sector_idx);
        }

        // --- Types 16, 76: close door, wait 30s, reopen ---
        16 | 76 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            close_wait_open_door(gs, level, sector_idx);
        }

        // --- Types 26/27/28: locked raise-and-close door ---
        26 => {
            // Blue card or skull required.
            if gs.player.has_key(crate::player::KEY_BLUE_CARD)
                || gs.player.has_key(crate::player::KEY_BLUE_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
                );
            }
        }
        27 => {
            // Yellow key required.
            if gs.player.has_key(crate::player::KEY_YELLOW_CARD)
                || gs.player.has_key(crate::player::KEY_YELLOW_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
                );
            }
        }
        28 => {
            // Red key required.
            if gs.player.has_key(crate::player::KEY_RED_CARD)
                || gs.player.has_key(crate::player::KEY_RED_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
                );
            }
        }

        // --- Type 63: remote tag-based door (open stay) ---
        63 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                open_door(
                    gs,
                    level,
                    idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // -----------------------------------------------------------------
        // Blazing doors (fast doors, speed=8)
        // -----------------------------------------------------------------

        // Type 105: WR Blazing door open-close.
        105 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                open_blazing_door(
                    gs,
                    level,
                    idx,
                    crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
                );
            }
        }

        // Type 106: WR Blazing door open-stay.
        106 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                open_blazing_door(
                    gs,
                    level,
                    idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 107: WR Blazing door close.
        107 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                close_blazing_door(gs, level, idx);
            }
        }

        // Type 108: W1 Blazing door open-close.
        108 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            open_blazing_door(
                gs,
                level,
                sector_idx,
                crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
            );
        }

        // Type 109: W1 Blazing door open-stay.
        109 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            open_blazing_door(
                gs,
                level,
                sector_idx,
                crate::linedef_dispatch::DoorBehavior::OpenStay,
            );
        }

        // Type 110: W1 Blazing door close.
        110 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            close_blazing_door(gs, level, sector_idx);
        }

        // -----------------------------------------------------------------
        // Additional keyed door line types
        // -----------------------------------------------------------------

        // Type 99: SR Blue key door open-stay.
        99 => {
            if gs.player.has_key(crate::player::KEY_BLUE_CARD)
                || gs.player.has_key(crate::player::KEY_BLUE_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 133: S1 Blue key door open-stay (blazing).
        133 => {
            if gs.player.has_key(crate::player::KEY_BLUE_CARD)
                || gs.player.has_key(crate::player::KEY_BLUE_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_blazing_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 134: SR Red key door open-stay.
        134 => {
            if gs.player.has_key(crate::player::KEY_RED_CARD)
                || gs.player.has_key(crate::player::KEY_RED_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 135: S1 Red key door open-stay (blazing).
        135 => {
            if gs.player.has_key(crate::player::KEY_RED_CARD)
                || gs.player.has_key(crate::player::KEY_RED_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_blazing_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 136: SR Yellow key door open-stay.
        136 => {
            if gs.player.has_key(crate::player::KEY_YELLOW_CARD)
                || gs.player.has_key(crate::player::KEY_YELLOW_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 137: S1 Yellow key door open-stay (blazing).
        137 => {
            if gs.player.has_key(crate::player::KEY_YELLOW_CARD)
                || gs.player.has_key(crate::player::KEY_YELLOW_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_blazing_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }
        _ => {}
    }
}

#[allow(unused_variables)]
pub fn activate_exits(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
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
        _ => {}
    }
}

#[allow(unused_variables)]
pub fn activate_ceilings(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Crushers
        // -----------------------------------------------------------------

        // Type 6: W1 Fast crusher ceiling (perpetual, speed=2).
        6 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_raise_fast(gs, level, tag, 2);
        }

        // Type 25: W1 Slow crusher ceiling (perpetual, speed=1).
        25 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_and_raise(gs, level, tag, 1);
        }

        // Type 44: W1 Ceiling lower to 8 above floor (one-shot, no crush damage).
        44 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_lower_and_crush(gs, level, tag, 2);
        }

        // Type 49: S1 Ceiling lower to 8 above floor + crush damage.
        49 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_crusher(
                gs,
                level,
                tag,
                CrusherParams {
                    speed: 2,
                    crush_damage: 10,
                    silent: false,
                    remove_when_done: true,
                    ceiling_type: CeilingType::LowerAndCrush,
                },
            );
        }

        // Type 57: W1 Stop ceiling crusher (remove all crushers matching tag).
        57 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_stop(gs, tag);
        }

        // Type 72: WR Ceiling lower to 8 above floor.
        72 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_lower_and_crush(gs, level, tag, 2);
        }

        // Type 73: WR Ceiling crush and raise (slow, perpetual).
        73 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_and_raise(gs, level, tag, 1);
        }

        // Type 74: WR Stop ceiling crusher.
        74 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_stop(gs, tag);
        }

        // Type 141: W1 Ceiling crush and raise (silent, perpetual).
        141 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_crusher(
                gs,
                level,
                tag,
                CrusherParams {
                    speed: 2,
                    crush_damage: 10,
                    silent: true,
                    remove_when_done: false,
                    ceiling_type: CeilingType::SilentCrush,
                },
            );
        }
        _ => {}
    }
}

#[allow(unused_variables)]
pub fn activate_lifts(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Lifts (lower-wait-raise)
        // -----------------------------------------------------------------

        // Type 62: Plat lower-wait-raise (speed 4).
        62 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 66: SR Raise floor 24 + change.
        66 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
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
        // Additional lift line types (using LiftMover)
        // -----------------------------------------------------------------

        // Type 120: WR Lift blazing (speed 8, wait 105).
        120 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_do_lift(gs, level, tag, 8, LIFT_WAIT);
        }

        // Type 122: S1 Lift blazing (speed 8, wait 105).
        122 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_do_lift(gs, level, tag, 8, LIFT_WAIT);
        }

        // Type 123: SR Lift blazing (speed 8, wait 105).
        123 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_do_lift(gs, level, tag, 8, LIFT_WAIT);
        }
        _ => {}
    }
}

#[allow(unused_variables)]
pub fn activate_floors(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Floor raisers
        // -----------------------------------------------------------------

        // Type 5: W1 Floor raise to lowest adjacent ceiling (crush).
        5 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_lowest_ceiling(gs, level, tag, 1, crate::state::CrushBehavior::Crush);
        }

        // Type 14: S1 Raise floor 32 + change texture/type.
        14 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_32(gs, level, tag, 1);
        }

        // Type 15: S1 Raise floor 24 + change texture/type.
        15 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 18: S1 Floor raise to next highest adjacent floor.
        18 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 20: S1 Raise floor to next highest + change texture.
        20 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 22: W1 Floor raise to next highest adjacent floor + change texture.
        22 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 24: G1 Raise floor to lowest adjacent ceiling.
        24 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_lowest_ceiling(
                gs,
                level,
                tag,
                1,
                crate::state::CrushBehavior::NoCrush,
            );
        }

        // Type 30: W1 Raise floor by shortest lower texture.
        30 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_by_texture(gs, level, tag, 1);
        }

        // Type 56: W1 Floor raise to 8 below lowest adjacent ceiling (crush).
        56 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = lowest_adjacent_ceiling(level, idx) - 8;
                    activate_floor_raise_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        crate::state::CrushBehavior::Crush,
                        FloorType::RaiseCrush,
                    );
                }
            }
        }

        // Type 58: W1 Raise floor 24.
        58 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 59: W1 Raise floor 24 + change texture/type.
        59 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 64: SR Raise floor to lowest adjacent ceiling.
        64 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_lowest_ceiling(
                gs,
                level,
                tag,
                1,
                crate::state::CrushBehavior::NoCrush,
            );
        }

        // Type 65: SR Raise floor to 8 below lowest ceiling + crush.
        65 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = lowest_adjacent_ceiling(level, idx) - 8;
                    activate_floor_raise_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        crate::state::CrushBehavior::Crush,
                        FloorType::RaiseCrush,
                    );
                }
            }
        }

        // Type 67: SR Raise floor 32 + change.
        67 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_32(gs, level, tag, 1);
        }

        // Type 68: SR Raise floor to next highest + change texture.
        68 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 91: WR Raise floor to lowest adjacent ceiling.
        91 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_lowest_ceiling(
                gs,
                level,
                tag,
                1,
                crate::state::CrushBehavior::NoCrush,
            );
        }

        // Type 92: WR Raise floor 24.
        92 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 93: WR Raise floor 24 + change.
        93 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 94: WR Raise floor to 8 below lowest ceiling + crush.
        94 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = lowest_adjacent_ceiling(level, idx) - 8;
                    activate_floor_raise_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        crate::state::CrushBehavior::Crush,
                        FloorType::RaiseCrush,
                    );
                }
            }
        }

        // Type 95: WR Raise floor to next highest + change texture.
        95 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 96: WR Raise floor by shortest lower texture.
        96 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_by_texture(gs, level, tag, 1);
        }

        // -----------------------------------------------------------------
        // Floor lowerers
        // -----------------------------------------------------------------

        // Type 19: W1 Lower floor to highest adjacent floor.
        19 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_highest(gs, level, tag, 1);
        }

        // Type 23: S1 Lower floor to lowest adjacent floor.
        23 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 36: W1 Lower floor to highest adjacent - 8 (turbo).
        36 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 37: W1 Lower floor to lowest adjacent + change texture/type.
        37 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 38: W1 Lower floor to lowest adjacent floor.
        38 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 45: SR Lower floor to highest adjacent floor.
        45 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_highest(gs, level, tag, 1);
        }

        // Type 60: SR Lower floor to lowest adjacent floor.
        60 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 69: SR Lower floor to highest adjacent - 8.
        69 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 70: SR Lower floor to highest adjacent - 8 (turbo).
        70 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 71: S1 Lower floor to highest adjacent - 8 (turbo).
        71 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 82: WR Lower floor to lowest adjacent floor.
        82 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 83: WR Lower floor to highest adjacent floor.
        83 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_highest(gs, level, tag, 1);
        }

        // Type 84: WR Lower floor to lowest adjacent + change.
        84 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 98: WR Lower floor to highest adjacent - 8 (turbo).
        98 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 102: S1 Lower floor to highest adjacent floor.
        102 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_highest(gs, level, tag, 1);
        }
        _ => {}
    }
}

#[allow(unused_variables)]
pub fn activate_stairs(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Teleporters
        // -----------------------------------------------------------------

        // -----------------------------------------------------------------
        // Stairs
        // -----------------------------------------------------------------

        // Type 7: S1 Build stairs 8 units.
        7 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_build_stairs(
                    gs,
                    level,
                    idx,
                    StairType::Build8,
                    crate::state::CrushBehavior::NoCrush,
                );
            }
        }

        // Type 8: W1 Build stairs turbo 16 units.
        8 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_build_stairs(
                    gs,
                    level,
                    idx,
                    StairType::Turbo16,
                    crate::state::CrushBehavior::NoCrush,
                );
            }
        }

        // Type 100: W1 Build stairs turbo 16 + crush.
        100 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_build_stairs(
                    gs,
                    level,
                    idx,
                    StairType::Turbo16,
                    crate::state::CrushBehavior::Crush,
                );
            }
        }

        // Type 127: S1 Build stairs turbo 16 units.
        127 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_build_stairs(
                    gs,
                    level,
                    idx,
                    StairType::Turbo16,
                    crate::state::CrushBehavior::NoCrush,
                );
            }
        }
        _ => {}
    }
}

#[allow(unused_variables)]
pub fn activate_platforms(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Perpetual platforms
        // -----------------------------------------------------------------

        // Type 53: S1 Perpetual platform (speed 1).
        53 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_perpetual_platform(gs, level, tag, 1);
        }

        // Type 54: W1 Stop platform (by tag).
        54 => {
            let tag = level.linedefs[linedef_idx].tag;
            gs.movers.active_platforms.retain(|p| p.tag != tag);
        }

        // Type 87: WR Perpetual platform (speed 1).
        87 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_perpetual_platform(gs, level, tag, 1);
        }

        // Type 89: WR Stop platform (by tag).
        89 => {
            let tag = level.linedefs[linedef_idx].tag;
            gs.movers.active_platforms.retain(|p| p.tag != tag);
        }
        _ => {}
    }
}

#[allow(unused_variables)]
pub fn activate_teleports(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Teleporters
        // -----------------------------------------------------------------

        // Type 39: W1 Teleport (walk trigger, one-shot).
        39 => {
            let tag = level.linedefs[linedef_idx].tag;
            let handle = gs.player.handle;
            ev_teleport(gs, level, tag, handle);
        }

        // Type 97: WR Teleport (walk trigger, repeatable).
        97 => {
            let tag = level.linedefs[linedef_idx].tag;
            let handle = gs.player.handle;
            ev_teleport(gs, level, tag, handle);
        }

        // Type 125: W1 Teleport Monsters Only.
        125 => {
            // Monsters-only teleport — no-op for player activation.
            // In a full implementation, this would only teleport monster actors.
        }

        // Type 126: WR Teleport Monsters Only (repeatable).
        126 => {
            // Monsters-only teleport — no-op for player activation.
        }
        _ => {}
    }
}

#[allow(unused_variables)]
pub fn activate_misc(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Donut specials
        // -----------------------------------------------------------------

        // Type 9: S1 Donut.
        9 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_do_donut(gs, level, idx);
            }
        }

        // Type 146: W1 Donut.
        146 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_do_donut(gs, level, idx);
            }
        }
        _ => {}
    }
}

/// Return the parametric fraction `t` along the segment from `(ax, ay)` to
/// `(bx, by)` where it intersects the linedef segment `(lx1, ly1)` → `(lx2, ly2)`.
///
/// The fraction is returned as `(numerator, denominator)` with
/// `0 <= numerator <= denominator` and `denominator > 0`.
pub fn segment_intersection_frac(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    lx1: i32,
    ly1: i32,
    lx2: i32,
    ly2: i32,
) -> Option<(i64, i64)> {
    let rdx = i64::from(bx - ax);
    let rdy = i64::from(by - ay);
    let sdx = i64::from(lx2 - lx1);
    let sdy = i64::from(ly2 - ly1);
    let qpx = i64::from(lx1 - ax);
    let qpy = i64::from(ly1 - ay);

    let denom = rdx * sdy - rdy * sdx;
    if denom == 0 {
        return None;
    }

    let t_num = qpx * sdy - qpy * sdx;
    let u_num = qpx * rdy - qpy * rdx;
    let (t_num, u_num, denom) = if denom < 0 {
        (-t_num, -u_num, -denom)
    } else {
        (t_num, u_num, denom)
    };

    if !(0..=denom).contains(&t_num) || !(0..=denom).contains(&u_num) {
        return None;
    }

    Some((t_num, denom))
}
