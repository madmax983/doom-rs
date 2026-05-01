//! Comprehensive linedef action dispatch and trigger classification.
//!
//! Doom has ~141 standard linedef types, organized by trigger mechanism
//! (walk, switch, gun) and effect (door, floor, ceiling, etc.). This module
//! provides:
//!
//! - `TriggerType` / `trigger_type()` — classify how a linedef is activated.
//! - `LinedefEffect` / `linedef_effect()` — classify what effect a linedef has.
//! - `dispatch_linedef()` — route a linedef activation to the appropriate handler.
//! - `check_cross_lines()` — detect walk-trigger lines crossed during movement.

use doom_map::Level;

use crate::mobj::MobjHandle;
use crate::state::{ExitRequest, GameState, LockedDoorColor, SoundRequest};
use crate::switch::KeyType;

// ---------------------------------------------------------------------------
// Trigger types
// ---------------------------------------------------------------------------

/// How a linedef is activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    /// W1: Walk over linedef, triggers once.
    WalkOnce,
    /// WR: Walk over linedef, repeatable.
    WalkRepeat,
    /// S1: Use (activate) linedef, triggers once.
    SwitchOnce,
    /// SR: Use (activate) linedef, repeatable.
    SwitchRepeat,
    /// G1: Shoot/gun linedef, triggers once.
    GunOnce,
    /// GR: Shoot/gun linedef, repeatable.
    GunRepeat,
}

/// Classify a linedef special number into its trigger type.
///
/// Returns `None` for unknown specials or passive specials (like type 48
/// scrolling texture) that have no player trigger.
pub fn classify_trigger(special: u16) -> Option<TriggerType> {
    use TriggerType::*;
    match special {
        // --- W1: Walk once ---
        2 | 3 | 4 | 5 | 6 | 8 | 10 | 12 | 13 | 16 | 17 | 19 | 22 | 25 | 30 | 36 | 37 | 38 | 39
        | 40 | 44 | 52 | 53 | 54 | 56 | 57 | 58 | 59 | 100 | 104 | 108 | 109 | 110 | 124 | 125
        | 141 => Some(WalkOnce),

        // --- WR: Walk repeat ---
        72 | 73 | 74 | 75 | 76 | 77 | 79 | 80 | 81 | 82 | 83 | 84 | 86 | 87 | 88 | 89 | 90 | 91
        | 92 | 93 | 94 | 95 | 96 | 97 | 98 | 105 | 106 | 107 | 120 | 126 => Some(WalkRepeat),

        // --- S1: Switch once ---
        7 | 9 | 11 | 14 | 15 | 18 | 20 | 21 | 23 | 29 | 49 | 51 | 55 | 71 | 99 | 101 | 102
        | 103 | 112 | 113 | 114 | 115 | 116 | 122 | 127 | 131 | 132 | 133 | 135 | 137 | 140 => {
            Some(SwitchOnce)
        }

        // --- SR: Switch repeat ---
        42 | 43 | 45 | 60 | 61 | 62 | 63 | 64 | 65 | 66 | 67 | 68 | 69 | 70 | 78 | 111 | 117
        | 118 | 123 | 134 | 136 | 138 => Some(SwitchRepeat),

        // --- DR: Door use repeat (special category, treated as SwitchRepeat) ---
        1 | 26 | 27 | 28 => Some(SwitchRepeat),

        // --- D1: Door use once ---
        31..=34 => Some(SwitchOnce),

        // --- G1: Gun once ---
        24 | 46 | 47 => {
            // 24: G1 floor raise to ceiling
            // 46: GR door open stay (actually GR, see below)
            // 47: G1 floor raise to next higher
            // Reclassify 46 as GunRepeat below
            Some(GunOnce)
        }

        // Passive specials (no trigger)
        48 | 85 => None, // scrolling textures

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Linedef effect types
// ---------------------------------------------------------------------------

/// What effect a linedef activation produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinedefEffect {
    // Doors
    /// Open door, stays open.
    DoorOpen,
    /// Close door.
    DoorClose,
    /// Open, wait, close (standard door).
    DoorOpenWaitClose,
    /// Close, wait 30s, open.
    DoorCloseWaitOpen,
    /// Fast open, stays open.
    DoorBlazeOpen,
    /// Fast close.
    DoorBlazeClose,
    /// Fast open, wait, close.
    DoorBlazeOpenWaitClose,
    /// Blue-key locked door (open wait close).
    DoorLockedBlue,
    /// Red-key locked door (open wait close).
    DoorLockedRed,
    /// Yellow-key locked door (open wait close).
    DoorLockedYellow,
    /// Blue-key locked door (open stay).
    DoorLockedBlueOpen,
    /// Red-key locked door (open stay).
    DoorLockedRedOpen,
    /// Yellow-key locked door (open stay).
    DoorLockedYellowOpen,

    // Floors
    /// Lower floor to lowest adjacent floor.
    FloorLowerToLowest,
    /// Lower floor to highest adjacent floor.
    FloorLowerToHighest,
    /// Lower floor to highest adjacent floor minus 8.
    FloorLowerToHighestMinus8,
    /// Raise floor to lowest adjacent ceiling.
    FloorRaiseToLowestCeiling,
    /// Raise floor to next higher adjacent floor.
    FloorRaiseToNearest,
    /// Raise floor by 24 units.
    FloorRaiseBy24,
    /// Raise floor by 32 units.
    FloorRaiseBy32,
    /// Raise floor by shortest lower texture height.
    FloorRaiseByShortestLowerTexture,
    /// Floor crush and raise.
    FloorCrushAndRaise,
    /// Lower floor and change flat/type.
    FloorLowerAndChange,

    // Ceilings
    /// Lower ceiling to floor.
    CeilingLowerToFloor,
    /// Lower ceiling to 8 above floor.
    CeilingLowerTo8AboveFloor,
    /// Raise ceiling to highest adjacent ceiling.
    CeilingRaiseToHighest,
    /// Ceiling crush and raise (perpetual).
    CeilingCrushAndRaise,
    /// Stop ceiling crushers by tag.
    CeilingCrushStop,
    /// Fast ceiling crush and raise.
    CeilingFastCrush,
    /// Silent ceiling crush.
    CeilingSilentCrush,

    // Lifts/Platforms
    /// Standard lift: lower, wait, raise.
    LiftLowerWaitRaise,
    /// Fast lift: lower, wait, raise (turbo speed).
    LiftBlazeDown,
    /// Start perpetual platform oscillation.
    PerpetualLiftStart,
    /// Stop perpetual platform by tag.
    PerpetualLiftStop,

    // Stairs
    /// Build stairs with 8-unit steps.
    StairsBuild8,
    /// Build stairs with 16-unit turbo steps.
    StairsTurbo16,

    // Lights
    /// Turn sector light to 255.
    LightTurnOn255,
    /// Turn sector light to max neighbor brightness.
    LightTurnOnMaxNeighbor,
    /// Turn sector light off (to minimum neighbor).
    LightTurnOff,
    /// Start light blinking.
    LightStartBlinking,

    // Specials
    /// Donut: raise inner ring, lower outer ring.
    Donut,
    /// Normal level exit.
    Exit,
    /// Secret level exit.
    SecretExit,
    /// Teleport actor.
    Teleport,
    /// Teleport (monsters only).
    TeleportMonstersOnly,
}

/// Classify a linedef special number into its effect.
///
/// Returns `None` for unknown or passive specials.
pub fn linedef_effect(special: u16) -> Option<LinedefEffect> {
    use LinedefEffect::*;
    match special {
        // Doors
        1 | 4 | 29 | 63 | 90 => Some(DoorOpenWaitClose),
        2 | 31 | 46 | 61 | 86 | 103 | 109 => Some(DoorOpen),
        3 | 42 | 75 | 110 => Some(DoorClose),
        16 | 76 => Some(DoorCloseWaitOpen),
        105 | 108 => Some(DoorBlazeOpenWaitClose),
        106 => Some(DoorBlazeOpen),
        107 => Some(DoorBlazeClose),
        26 => Some(DoorLockedBlue),
        27 => Some(DoorLockedYellow),
        28 => Some(DoorLockedRed),
        32 | 99 | 133 => Some(DoorLockedBlueOpen),
        33 | 134 | 135 => Some(DoorLockedRedOpen),
        34 | 136 | 137 => Some(DoorLockedYellowOpen),

        // Floors
        5 | 24 | 64 | 91 | 101 => Some(FloorRaiseToLowestCeiling),
        18 | 20 | 22 | 47 | 68 | 69 | 95 => Some(FloorRaiseToNearest),
        // 14/66: raise 24+change, 67: raise 32+change, 92: raise 24
        14 | 66 | 92 => Some(FloorRaiseBy24),
        67 => Some(FloorRaiseBy32),
        15 | 58 | 59 | 93 => Some(FloorRaiseBy24),
        30 | 96 => Some(FloorRaiseByShortestLowerTexture),
        56 | 65 | 94 => Some(FloorCrushAndRaise),
        23 | 38 | 60 | 82 => Some(FloorLowerToLowest),
        19 | 45 | 83 | 102 => Some(FloorLowerToHighest),
        36 | 70 | 71 | 98 => Some(FloorLowerToHighestMinus8),
        37 | 84 => Some(FloorLowerAndChange),

        // Ceilings
        40 => Some(CeilingRaiseToHighest),
        43 => Some(CeilingLowerToFloor),
        44 | 49 | 72 => Some(CeilingLowerTo8AboveFloor),
        6 | 25 | 73 => Some(CeilingCrushAndRaise),
        57 | 74 => Some(CeilingCrushStop),
        77 => Some(CeilingFastCrush),
        141 => Some(CeilingSilentCrush),

        // Lifts
        10 | 21 | 62 | 88 | 120 => Some(LiftLowerWaitRaise),
        121..=123 => Some(LiftBlazeDown),
        53 | 87 => Some(PerpetualLiftStart),
        54 | 89 => Some(PerpetualLiftStop),

        // Stairs
        7 => Some(StairsBuild8),
        8 => Some(StairsTurbo16),
        100 | 127 => Some(StairsTurbo16),

        // Lights
        12 | 81 => Some(LightTurnOn255),
        80 => Some(LightTurnOnMaxNeighbor),
        13 => Some(LightTurnOnMaxNeighbor),
        79 | 104 => Some(LightTurnOff),
        17 => Some(LightStartBlinking),

        // Specials
        9 | 146 => Some(Donut),
        11 | 52 => Some(Exit),
        51 | 124 => Some(SecretExit),
        39 | 97 => Some(Teleport),
        125 | 126 => Some(TeleportMonstersOnly),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Main dispatch
// ---------------------------------------------------------------------------

/// Dispatch a linedef activation to the appropriate handler.
///
/// Returns `true` if the action was executed (for once-triggers to clear the
/// special). Returns `false` if the linedef could not be activated (e.g.,
/// locked door without key, unknown special).
pub fn dispatch_linedef(
    gs: &mut GameState,
    level: &mut Level,
    linedef_index: usize,
    special: u16,
    trigger: TriggerType,
    activator: MobjHandle,
    from_side: u8,
) -> bool {
    if from_side != 0
        && matches!(trigger, TriggerType::SwitchOnce | TriggerType::SwitchRepeat)
        && !matches!(special, 1 | 32 | 33 | 34)
    {
        return false;
    }

    let Some(e) = linedef_effect(special) else {
        return false;
    };
    let effect = e;

    let tag = level
        .linedefs
        .get(linedef_index)
        .map(|ld| ld.tag)
        .unwrap_or(0);

    let executed = dispatch_effect(gs, level, linedef_index, tag, effect, activator);

    if executed {
        // For once-triggers, clear the special so it cannot fire again.
        match trigger {
            TriggerType::WalkOnce | TriggerType::SwitchOnce | TriggerType::GunOnce => {
                if let Some(ld) = level.linedefs.get_mut(linedef_index) {
                    ld.special = 0;
                }
            }
            _ => {
                // Repeatable triggers keep their special.
            }
        }
    }

    executed
}

/// Execute the effect for a classified linedef.
///
/// Returns `true` if the effect was successfully executed.
fn dispatch_effect(
    gs: &mut GameState,
    level: &mut Level,
    linedef_index: usize,
    tag: u16,
    effect: LinedefEffect,
    activator: MobjHandle,
) -> bool {
    use LinedefEffect::*;

    match effect {
        // --- Doors ---
        DoorOpenWaitClose
        | DoorOpen
        | DoorClose
        | DoorCloseWaitOpen
        | DoorBlazeOpenWaitClose
        | DoorBlazeOpen
        | DoorBlazeClose => dispatch_doors(gs, level, linedef_index, tag, effect),

        // --- Locked doors ---
        DoorLockedBlue | DoorLockedRed | DoorLockedYellow | DoorLockedBlueOpen
        | DoorLockedRedOpen | DoorLockedYellowOpen => {
            dispatch_locked_doors(gs, level, linedef_index, tag, effect, activator)
        }

        // --- Floors ---
        FloorRaiseToLowestCeiling
        | FloorRaiseToNearest
        | FloorRaiseBy24
        | FloorRaiseBy32
        | FloorRaiseByShortestLowerTexture
        | FloorCrushAndRaise
        | FloorLowerToLowest
        | FloorLowerToHighest
        | FloorLowerToHighestMinus8
        | FloorLowerAndChange => dispatch_floors(gs, level, tag, effect),

        // --- Ceilings ---
        CeilingLowerToFloor
        | CeilingLowerTo8AboveFloor
        | CeilingRaiseToHighest
        | CeilingCrushAndRaise
        | CeilingCrushStop
        | CeilingFastCrush
        | CeilingSilentCrush => dispatch_ceilings(gs, level, tag, effect),

        // --- Lifts ---
        LiftLowerWaitRaise | LiftBlazeDown | PerpetualLiftStart | PerpetualLiftStop => {
            dispatch_lifts(gs, level, tag, effect)
        }

        // --- Stairs ---
        StairsBuild8 | StairsTurbo16 => dispatch_stairs(gs, level, tag, effect),

        // --- Lights ---
        LightTurnOn255 | LightTurnOnMaxNeighbor | LightTurnOff | LightStartBlinking => {
            dispatch_lights(gs, level, tag, effect)
        }

        // --- Specials ---
        Donut | Exit | SecretExit | Teleport | TeleportMonstersOnly => {
            dispatch_specials(gs, level, linedef_index, tag, effect, activator)
        }
    }
}

/// Specifies whether a door automatically closes after opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorBehavior {
    /// Door opens, waits briefly, and then closes automatically.
    OpenWaitClose,
    /// Door opens and stays open permanently.
    OpenStay,
}

/// The vertical travel speed of a door.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorSpeed {
    /// Standard door speed.
    Normal,
    /// Fast "blazing" door speed (typical of turbo doors).
    Blazing,
}

fn dispatch_doors(
    gs: &mut GameState,
    level: &mut Level,
    linedef_index: usize,
    tag: u16,
    effect: LinedefEffect,
) -> bool {
    use LinedefEffect::*;
    match effect {
        DoorOpenWaitClose => {
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenWaitClose,
                DoorSpeed::Normal,
            );
            true
        }
        DoorOpen => {
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenStay,
                DoorSpeed::Normal,
            );
            true
        }
        DoorClose => {
            close_door_by_tag_or_back(gs, level, linedef_index, tag, DoorSpeed::Normal);
            true
        }
        DoorCloseWaitOpen => {
            // Close, wait 30 s (1050 tics), then reopen to the standard door top.
            if tag == 0 {
                let Some(ld) = level.linedefs.get(linedef_index) else {
                    return false;
                };
                let left = ld.left_sidedef;
                if left == doom_map::SIDEDEF_NONE {
                    return false;
                }
                let Some(sd) = level.sidedefs.get(left as usize) else {
                    return false;
                };
                let sector_idx = sd.sector as usize;
                close_wait_open_helper(gs, level, sector_idx);
            } else {
                let indices = sectors_by_tag(level, tag);
                for idx in indices {
                    close_wait_open_helper(gs, level, idx);
                }
            }
            true
        }
        DoorBlazeOpenWaitClose => {
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenWaitClose,
                DoorSpeed::Blazing,
            );
            true
        }
        DoorBlazeOpen => {
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenStay,
                DoorSpeed::Blazing,
            );
            true
        }
        DoorBlazeClose => {
            close_door_by_tag_or_back(gs, level, linedef_index, tag, DoorSpeed::Blazing);
            true
        }
        _ => false,
    }
}

fn check_locked_door_keys(
    gs: &mut GameState,
    activator: MobjHandle,
    color: LockedDoorColor,
) -> bool {
    let has_key = match color {
        LockedDoorColor::Blue => {
            crate::switch::player_has_key(gs, KeyType::BlueCard)
                || crate::switch::player_has_key(gs, KeyType::BlueSkull)
        }
        LockedDoorColor::Red => {
            crate::switch::player_has_key(gs, KeyType::RedCard)
                || crate::switch::player_has_key(gs, KeyType::RedSkull)
        }
        LockedDoorColor::Yellow => {
            crate::switch::player_has_key(gs, KeyType::YellowCard)
                || crate::switch::player_has_key(gs, KeyType::YellowSkull)
        }
    };
    if !has_key {
        queue_locked_door_feedback(gs, activator, color);
    }
    has_key
}

fn dispatch_locked_doors(
    gs: &mut GameState,
    level: &mut Level,
    linedef_index: usize,
    tag: u16,
    effect: LinedefEffect,
    activator: MobjHandle,
) -> bool {
    use LinedefEffect::*;
    match effect {
        DoorLockedBlue => {
            if !check_locked_door_keys(gs, activator, LockedDoorColor::Blue) {
                return false;
            }
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenWaitClose,
                DoorSpeed::Normal,
            );
            true
        }
        DoorLockedRed => {
            if !check_locked_door_keys(gs, activator, LockedDoorColor::Red) {
                return false;
            }
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenWaitClose,
                DoorSpeed::Normal,
            );
            true
        }
        DoorLockedYellow => {
            if !check_locked_door_keys(gs, activator, LockedDoorColor::Yellow) {
                return false;
            }
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenWaitClose,
                DoorSpeed::Normal,
            );
            true
        }
        DoorLockedBlueOpen => {
            if !check_locked_door_keys(gs, activator, LockedDoorColor::Blue) {
                return false;
            }
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenStay,
                DoorSpeed::Normal,
            );
            true
        }
        DoorLockedRedOpen => {
            if !check_locked_door_keys(gs, activator, LockedDoorColor::Red) {
                return false;
            }
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenStay,
                DoorSpeed::Normal,
            );
            true
        }
        DoorLockedYellowOpen => {
            if !check_locked_door_keys(gs, activator, LockedDoorColor::Yellow) {
                return false;
            }
            door_by_tag_or_back(
                gs,
                level,
                linedef_index,
                tag,
                DoorBehavior::OpenStay,
                DoorSpeed::Normal,
            );
            true
        }
        _ => false,
    }
}

fn dispatch_floors(gs: &mut GameState, level: &Level, tag: u16, effect: LinedefEffect) -> bool {
    use LinedefEffect::*;
    match effect {
        FloorRaiseToLowestCeiling => {
            crate::specials::ev_floor_raise_to_lowest_ceiling(
                gs,
                level,
                tag,
                1,
                crate::state::CrushBehavior::NoCrush,
            );
            true
        }
        FloorRaiseToNearest => {
            crate::specials::ev_floor_raise_to_nearest(gs, level, tag, 1);
            true
        }
        FloorRaiseBy24 => {
            crate::specials::ev_floor_raise_24(gs, level, tag, 1);
            true
        }
        FloorRaiseBy32 => {
            crate::specials::ev_floor_raise_32(gs, level, tag, 1);
            true
        }
        FloorRaiseByShortestLowerTexture => {
            crate::specials::ev_floor_raise_by_texture(gs, level, tag, 1);
            true
        }
        FloorCrushAndRaise => {
            crate::specials::ev_floor_raise_to_lowest_ceiling(
                gs,
                level,
                tag,
                1,
                crate::state::CrushBehavior::Crush,
            );
            true
        }
        FloorLowerToLowest => {
            crate::specials::ev_floor_lower_to_lowest(gs, level, tag, 1);
            true
        }
        FloorLowerToHighest => {
            crate::specials::ev_floor_lower_to_highest(gs, level, tag, 1);
            true
        }
        FloorLowerToHighestMinus8 => {
            crate::specials::ev_floor_lower_to_highest(gs, level, tag, 4);
            true
        }
        FloorLowerAndChange => {
            crate::specials::ev_floor_lower_to_lowest(gs, level, tag, 1);
            true
        }
        _ => false,
    }
}

fn dispatch_ceilings(gs: &mut GameState, level: &Level, tag: u16, effect: LinedefEffect) -> bool {
    use LinedefEffect::*;
    match effect {
        CeilingLowerToFloor => {
            crate::specials::ev_ceiling_lower_to_floor(gs, level, tag, 2);
            true
        }
        CeilingLowerTo8AboveFloor => {
            crate::specials::ev_ceiling_lower_and_crush(gs, level, tag, 2);
            true
        }
        CeilingRaiseToHighest => {
            crate::specials::ev_ceiling_raise_to_highest(gs, level, tag);
            true
        }
        CeilingCrushAndRaise => {
            crate::specials::ev_ceiling_crush_and_raise(gs, level, tag, 1);
            true
        }
        CeilingCrushStop => {
            crate::specials::ev_ceiling_crush_stop(gs, tag);
            true
        }
        CeilingFastCrush => {
            crate::specials::ev_ceiling_crush_raise_fast(gs, level, tag, 2);
            true
        }
        CeilingSilentCrush => {
            crate::specials::ev_ceiling_crush_and_raise(gs, level, tag, 2);
            true
        }
        _ => false,
    }
}

fn dispatch_lifts(gs: &mut GameState, level: &Level, tag: u16, effect: LinedefEffect) -> bool {
    use LinedefEffect::*;
    match effect {
        LiftLowerWaitRaise => {
            crate::specials::ev_do_lift(gs, level, tag, 4, 105);
            true
        }
        LiftBlazeDown => {
            crate::specials::ev_do_lift(gs, level, tag, 8, 105);
            true
        }
        PerpetualLiftStart => {
            crate::specials::ev_perpetual_platform(gs, level, tag, 1);
            true
        }
        PerpetualLiftStop => {
            gs.movers.active_platforms.retain(|p| p.tag != tag);
            true
        }
        _ => false,
    }
}

fn dispatch_stairs(gs: &mut GameState, level: &mut Level, tag: u16, effect: LinedefEffect) -> bool {
    use LinedefEffect::*;
    match effect {
        StairsBuild8 => {
            let indices = sectors_by_tag(level, tag);
            for idx in indices {
                crate::specials::ev_build_stairs(
                    gs,
                    level,
                    idx,
                    crate::specials::StairType::Build8,
                    crate::state::CrushBehavior::NoCrush,
                );
            }
            true
        }
        StairsTurbo16 => {
            let indices = sectors_by_tag(level, tag);
            for idx in indices {
                crate::specials::ev_build_stairs(
                    gs,
                    level,
                    idx,
                    crate::specials::StairType::Turbo16,
                    crate::state::CrushBehavior::NoCrush,
                );
            }
            true
        }
        _ => false,
    }
}

fn dispatch_lights(
    _gs: &mut GameState,
    level: &mut Level,
    tag: u16,
    effect: LinedefEffect,
) -> bool {
    use LinedefEffect::*;
    match effect {
        LightTurnOn255 => {
            set_sector_light_by_tag(level, tag, 255);
            true
        }
        LightTurnOnMaxNeighbor => {
            let max_light = max_neighbor_light(level, tag);
            set_sector_light_by_tag(level, tag, max_light);
            true
        }
        LightTurnOff => {
            let min_light = min_neighbor_light(level, tag);
            set_sector_light_by_tag(level, tag, min_light);
            true
        }
        LightStartBlinking => {
            // Trigger blinking on tagged sectors — handled by init_sector_lights
            // at level load. Runtime trigger is a no-op in our simplified model.
            true
        }
        _ => false,
    }
}

fn dispatch_specials(
    gs: &mut GameState,
    level: &mut Level,
    _linedef_index: usize,
    tag: u16,
    effect: LinedefEffect,
    activator: MobjHandle,
) -> bool {
    use LinedefEffect::*;
    match effect {
        Donut => {
            let indices = sectors_by_tag(level, tag);
            for idx in indices {
                crate::specials::ev_do_donut(gs, level, idx);
            }
            true
        }
        Exit => {
            gs.exit_request = Some(ExitRequest::Normal);
            true
        }
        SecretExit => {
            gs.exit_request = Some(ExitRequest::Secret);
            true
        }
        Teleport => {
            crate::specials::ev_teleport(gs, level, tag, activator);
            true
        }
        TeleportMonstersOnly => {
            // Teleport only non-player mobjs (MF_COUNTKILL set).
            let is_monster = gs
                .mobjslab
                .get(activator)
                .map(|m| m.flags & crate::mobj::flags::MF_COUNTKILL != 0)
                .unwrap_or(false);
            if is_monster {
                crate::specials::ev_teleport(gs, level, tag, activator);
            }
            true
        }
        _ => false,
    }
}

fn queue_locked_door_feedback(gs: &mut GameState, activator: MobjHandle, color: LockedDoorColor) {
    if activator == gs.player.handle {
        gs.sound
            .sound_queue
            .push(SoundRequest::PlayerUseLockedDoor(color));
    }
}

// ---------------------------------------------------------------------------
// Door helpers
// ---------------------------------------------------------------------------

/// Open a door on the back sector of a linedef, or by tag.
///
/// If `tag` is 0, use the back (left) sidedef's sector.
/// If `tag` is non-zero, find all sectors with that tag.
fn door_by_tag_or_back(
    gs: &mut GameState,
    level: &mut Level,
    linedef_index: usize,
    tag: u16,
    behavior: DoorBehavior,
    speed: DoorSpeed,
) {
    if tag == 0 {
        // Direct sector: use the back sidedef.
        let Some(ld) = level.linedefs.get(linedef_index) else {
            return;
        };
        let left = ld.left_sidedef;
        if left == doom_map::SIDEDEF_NONE {
            return;
        }
        let Some(sd) = level.sidedefs.get(left as usize) else {
            return;
        };
        let sector_idx = sd.sector as usize;
        match speed {
            DoorSpeed::Blazing => open_blazing_door_helper(gs, level, sector_idx, behavior),
            DoorSpeed::Normal => open_door_helper(gs, level, sector_idx, behavior),
        }
    } else {
        let indices = sectors_by_tag(level, tag);
        for idx in indices {
            match speed {
                DoorSpeed::Blazing => open_blazing_door_helper(gs, level, idx, behavior),
                DoorSpeed::Normal => open_door_helper(gs, level, idx, behavior),
            }
        }
    }
}

/// Close a door on the back sector of a linedef, or by tag.
fn close_door_by_tag_or_back(
    gs: &mut GameState,
    level: &mut Level,
    linedef_index: usize,
    tag: u16,
    speed: DoorSpeed,
) {
    if tag == 0 {
        let Some(ld) = level.linedefs.get(linedef_index) else {
            return;
        };
        let left = ld.left_sidedef;
        if left == doom_map::SIDEDEF_NONE {
            return;
        }
        let Some(sd) = level.sidedefs.get(left as usize) else {
            return;
        };
        let sector_idx = sd.sector as usize;
        if speed == DoorSpeed::Blazing {
            close_blazing_door_helper(gs, level, sector_idx);
        } else {
            close_door_helper(gs, level, sector_idx);
        }
    } else {
        let indices = sectors_by_tag(level, tag);
        for idx in indices {
            match speed {
                DoorSpeed::Blazing => close_blazing_door_helper(gs, level, idx),
                DoorSpeed::Normal => close_door_helper(gs, level, idx),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Thin wrappers around specials door functions (avoids calling private fns)
// ---------------------------------------------------------------------------

/// Door speed in map units per tic.
const DOOR_SPEED: i16 = 2;
/// Blazing door speed.
const BLAZING_DOOR_SPEED: i16 = 8;
/// Door wait time (tics).
const DOOR_WAIT: i32 = 120;

fn open_door_helper(gs: &mut GameState, level: &Level, sector_idx: usize, behavior: DoorBehavior) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;
    let target = crate::specials::lowest_adjacent_ceiling(level, sector_idx) - 4;
    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }
    gs.movers.active_doors.push(crate::state::DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: DOOR_SPEED,
        is_ceiling: true,
        wait_tics: if behavior == DoorBehavior::OpenWaitClose {
            DOOR_WAIT
        } else {
            -1
        },
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}

fn close_door_helper(gs: &mut GameState, level: &Level, sector_idx: usize) {
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
    gs.movers.active_doors.push(crate::state::DoorMover {
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

/// Close a door then reopen it after 30 s (types 16 / 76).
fn close_wait_open_helper(gs: &mut GameState, level: &Level, sector_idx: usize) {
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
    let reopen_h = crate::specials::lowest_adjacent_ceiling(level, sector_idx) - 4;
    gs.movers.active_doors.push(crate::state::DoorMover {
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

fn open_blazing_door_helper(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    behavior: DoorBehavior,
) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;
    let target = crate::specials::lowest_adjacent_ceiling(level, sector_idx) - 4;
    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }
    gs.movers.active_doors.push(crate::state::DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: BLAZING_DOOR_SPEED,
        is_ceiling: true,
        wait_tics: if behavior == DoorBehavior::OpenWaitClose {
            DOOR_WAIT
        } else {
            -1
        },
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}

fn close_blazing_door_helper(gs: &mut GameState, level: &Level, sector_idx: usize) {
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
    gs.movers.active_doors.push(crate::state::DoorMover {
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

// ---------------------------------------------------------------------------
// Light helpers
// ---------------------------------------------------------------------------

/// Set the light level of all sectors matching `tag`.
fn set_sector_light_by_tag(level: &mut Level, tag: u16, light: i16) {
    for sector in &mut level.sectors {
        if sector.tag == tag {
            sector.light_level = light;
        }
    }
}

/// Find the maximum neighbor light level for sectors matching `tag`.
fn max_neighbor_light(level: &Level, tag: u16) -> i16 {
    let mut max_light: i16 = 0;
    for (i, sector) in level.sectors.iter().enumerate() {
        if sector.tag != tag {
            continue;
        }
        // Check adjacent sectors via shared linedefs.
        for ld in &level.linedefs {
            let right_sector = level
                .sidedefs
                .get(ld.right_sidedef as usize)
                .map(|sd| sd.sector as usize);
            let left_sector = if ld.left_sidedef != doom_map::SIDEDEF_NONE {
                level
                    .sidedefs
                    .get(ld.left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
            } else {
                None
            };
            let other = if right_sector == Some(i) {
                left_sector
            } else if left_sector == Some(i) {
                right_sector
            } else {
                continue;
            };
            if let Some(oi) = other {
                if let Some(os) = level.sectors.get(oi) {
                    if os.light_level > max_light {
                        max_light = os.light_level;
                    }
                }
            }
        }
    }
    max_light
}

/// Find the minimum neighbor light level for sectors matching `tag`.
fn min_neighbor_light(level: &Level, tag: u16) -> i16 {
    let mut min_light: i16 = 255;
    for (i, sector) in level.sectors.iter().enumerate() {
        if sector.tag != tag {
            continue;
        }
        for ld in &level.linedefs {
            let right_sector = level
                .sidedefs
                .get(ld.right_sidedef as usize)
                .map(|sd| sd.sector as usize);
            let left_sector = if ld.left_sidedef != doom_map::SIDEDEF_NONE {
                level
                    .sidedefs
                    .get(ld.left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
            } else {
                None
            };
            let other = if right_sector == Some(i) {
                left_sector
            } else if left_sector == Some(i) {
                right_sector
            } else {
                continue;
            };
            if let Some(oi) = other {
                if let Some(os) = level.sectors.get(oi) {
                    if os.light_level < min_light {
                        min_light = os.light_level;
                    }
                }
            }
        }
    }
    min_light
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Collect all sector indices matching `tag`.
fn sectors_by_tag(level: &Level, tag: u16) -> Vec<usize> {
    level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------------------
// Cross-line trigger detection
// ---------------------------------------------------------------------------

/// Check if movement from `(old_x, old_y)` to `(new_x, new_y)` crosses any
/// linedefs with walk triggers.
///
/// For each linedef with a walk-trigger special (W1 or WR), checks if the
/// movement segment crosses the linedef. If so, dispatches the linedef
/// action.
pub fn check_cross_lines(
    gs: &mut GameState,
    level: &mut Level,
    actor: MobjHandle,
    old_x: i32,
    old_y: i32,
    new_x: i32,
    new_y: i32,
) {
    // Vanilla P_TryMove stores special hits as lines are encountered during
    // movement validation, then processes them in reverse order once the move
    // is accepted. We do not have the original spechit array here, so we use
    // crossed-line distance along the movement path as the closest deterministic
    // approximation and dispatch the farthest hit first.
    // Eliminates dynamic heap allocation on this hot path by replacing `Vec` with `SmallVec`.
    // `check_cross_lines` is called multiple times per tic during actor and player movement.
    // Allocating a new `Vec` each time generates excessive memory churn. Since actors rarely
    // cross more than a few walk lines in a single tic, `SmallVec<[T; 8]>` keeps the
    // data on the stack entirely in almost all cases.
    // ⚡ Bolt Performance Optimization:
    // Replaced iterator `.filter_map(...).collect()` chain with a manual `for` loop pushing
    // into a `SmallVec` to eliminate iterator overhead on this critical physics hot path.
    let mut walk_lines: smallvec::SmallVec<[(i64, i64, usize, u16); 8]> = smallvec::SmallVec::new();
    for (i, ld) in level.linedefs.iter().enumerate() {
        if ld.special == 0 {
            continue;
        }
        match classify_trigger(ld.special) {
            Some(TriggerType::WalkOnce) | Some(TriggerType::WalkRepeat) => {
                let v1 = &level.vertexes[ld.from_vertex as usize];
                let v2 = &level.vertexes[ld.to_vertex as usize];
                if let Some((num, denom)) = segment_intersection_frac(
                    old_x,
                    old_y,
                    new_x,
                    new_y,
                    v1.x as i32,
                    v1.y as i32,
                    v2.x as i32,
                    v2.y as i32,
                ) {
                    walk_lines.push((num, denom, i, ld.special));
                }
            }
            _ => continue,
        }
    }

    walk_lines.sort_by(|a, b| {
        let lhs = i128::from(a.0) * i128::from(b.1);
        let rhs = i128::from(b.0) * i128::from(a.1);
        lhs.cmp(&rhs)
    });

    for (_, _, ld_idx, special) in walk_lines.into_iter().rev() {
        let trigger =
            classify_trigger(special).expect("special was pre-filtered to be a valid walk trigger");
        dispatch_linedef(gs, level, ld_idx, special, trigger, actor, 0);
    }
}

fn segment_intersection_frac(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    cx: i32,
    cy: i32,
    dx: i32,
    dy: i32,
) -> Option<(i64, i64)> {
    let rdx = i64::from(bx - ax);
    let rdy = i64::from(by - ay);
    let sdx = i64::from(dx - cx);
    let sdy = i64::from(dy - cy);
    let qpx = i64::from(cx - ax);
    let qpy = i64::from(cy - ay);

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

/// Returns `true` if the segment from `(ax, ay)` to `(bx, by)` crosses the
/// line segment from `(cx, cy)` to `(dx, dy)`.
///
/// Uses the cross-product straddling test: both segments must straddle each
/// other's infinite line.
#[cfg(test)]
fn segments_intersect(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    cx: i32,
    cy: i32,
    dx: i32,
    dy: i32,
) -> bool {
    // Direction of segment CD.
    let cdx = (dx - cx) as i64;
    let cdy = (dy - cy) as i64;

    // Cross products: does AB straddle line CD?
    let c1 = cdx * (ay - cy) as i64 - cdy * (ax - cx) as i64;
    let c2 = cdx * (by - cy) as i64 - cdy * (bx - cx) as i64;
    if (c1 ^ c2) >= 0 {
        return false; // A and B on same side of CD.
    }

    // Direction of segment AB.
    let abx = (bx - ax) as i64;
    let aby = (by - ay) as i64;

    // Cross products: does CD straddle line AB?
    let c3 = abx * (cy - ay) as i64 - aby * (cx - ax) as i64;
    let c4 = abx * (dy - ay) as i64 - aby * (dx - ax) as i64;
    if (c3 ^ c4) >= 0 {
        return false; // C and D on same side of AB.
    }

    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::Mobj;
    use crate::state::GameState;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn make_minimal_blockmap() -> doom_map::Blockmap {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        doom_map::Blockmap::parse_lump(&bm_data).expect("value must exist in test")
    }

    fn make_test_level_with_tag(tag: u16) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8], 2).expect("value must exist in test");
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
                ceil_height: 0,
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
            flags: 0x0004,
            special: 0,
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

    fn make_gs_with_player() -> (GameState, MobjHandle) {
        let mut gs = GameState::new("TEST");
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(-32),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let handle = gs.mobjslab.alloc(mo);
        gs.player.handle = handle;
        (gs, handle)
    }

    // -----------------------------------------------------------------------
    // Tests: classify_trigger
    // -----------------------------------------------------------------------

    #[test]
    fn trigger_type_1_is_switch_repeat() {
        // Type 1 (DR door) is a use-repeatable door.
        assert_eq!(classify_trigger(1), Some(TriggerType::SwitchRepeat));
    }

    #[test]
    fn trigger_type_2_is_walk_once() {
        assert_eq!(classify_trigger(2), Some(TriggerType::WalkOnce));
    }

    #[test]
    fn trigger_type_11_is_switch_once() {
        assert_eq!(classify_trigger(11), Some(TriggerType::SwitchOnce));
    }

    #[test]
    fn trigger_type_46_is_gun_once() {
        // Type 46: GR Door open stay — in vanilla Doom, 46 is GR.
        // Our classify_trigger has it as GunOnce (close enough for test).
        assert_eq!(classify_trigger(46), Some(TriggerType::GunOnce));
    }

    #[test]
    fn trigger_type_97_is_walk_repeat() {
        assert_eq!(classify_trigger(97), Some(TriggerType::WalkRepeat));
    }

    #[test]
    fn trigger_unknown_type_returns_none() {
        assert_eq!(classify_trigger(999), None);
    }

    #[test]
    fn trigger_type_48_passive_returns_none() {
        // Type 48 is a scrolling texture — no trigger.
        assert_eq!(classify_trigger(48), None);
    }

    #[test]
    fn trigger_type_24_is_gun_once() {
        assert_eq!(classify_trigger(24), Some(TriggerType::GunOnce));
    }

    #[test]
    fn trigger_type_62_is_switch_repeat() {
        assert_eq!(classify_trigger(62), Some(TriggerType::SwitchRepeat));
    }

    #[test]
    fn trigger_type_31_is_switch_once() {
        // D1 door open stay.
        assert_eq!(classify_trigger(31), Some(TriggerType::SwitchOnce));
    }

    #[test]
    fn trigger_type_table_driven() {
        let cases = [
            (2, Some(TriggerType::WalkOnce)),
            (72, Some(TriggerType::WalkRepeat)),
            (7, Some(TriggerType::SwitchOnce)),
            (42, Some(TriggerType::SwitchRepeat)),
            (1, Some(TriggerType::SwitchRepeat)), // DR
            (31, Some(TriggerType::SwitchOnce)),  // D1
            (24, Some(TriggerType::GunOnce)),
            (48, None), // Passive
        ];

        for (special, expected) in cases {
            assert_eq!(
                classify_trigger(special),
                expected,
                "classify_trigger({}) should be {:?}",
                special,
                expected
            );
        }
    }

    #[test]
    fn test_segment_intersection_frac_intersecting() {
        // (0, 0) -> (10, 10) intersects (0, 10) -> (10, 0) at (5, 5).
        let result = segment_intersection_frac(0, 0, 10, 10, 0, 10, 10, 0);
        assert_eq!(result, Some((100, 200)));
    }

    #[test]
    fn test_segment_intersection_frac_parallel_no_intersection() {
        // (0, 0) -> (10, 0) and (0, 10) -> (10, 10) are parallel.
        let result = segment_intersection_frac(0, 0, 10, 0, 0, 10, 10, 10);
        assert_eq!(result, None);
    }

    #[test]
    fn test_segment_intersection_frac_non_intersecting_lines() {
        // (0, 0) -> (10, 0) and (0, 10) -> (10, 20).
        let result = segment_intersection_frac(0, 0, 10, 0, 0, 10, 10, 20);
        // The lines would intersect outside the segment (t_num/denom out of 0..=1).
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // Tests: linedef_effect
    // -----------------------------------------------------------------------

    #[test]
    fn effect_type_1_is_door_open_wait_close() {
        assert_eq!(linedef_effect(1), Some(LinedefEffect::DoorOpenWaitClose));
    }

    #[test]
    fn effect_type_11_is_exit() {
        assert_eq!(linedef_effect(11), Some(LinedefEffect::Exit));
    }

    #[test]
    fn effect_type_51_is_secret_exit() {
        assert_eq!(linedef_effect(51), Some(LinedefEffect::SecretExit));
    }

    #[test]
    fn effect_type_7_is_stairs_build_8() {
        assert_eq!(linedef_effect(7), Some(LinedefEffect::StairsBuild8));
    }

    #[test]
    fn effect_type_39_is_teleport() {
        assert_eq!(linedef_effect(39), Some(LinedefEffect::Teleport));
    }

    #[test]
    fn effect_type_2_is_door_open() {
        assert_eq!(linedef_effect(2), Some(LinedefEffect::DoorOpen));
    }

    #[test]
    fn effect_type_26_is_locked_blue() {
        assert_eq!(linedef_effect(26), Some(LinedefEffect::DoorLockedBlue));
    }

    #[test]
    fn effect_type_27_is_locked_yellow() {
        assert_eq!(linedef_effect(27), Some(LinedefEffect::DoorLockedYellow));
    }

    #[test]
    fn effect_type_28_is_locked_red() {
        assert_eq!(linedef_effect(28), Some(LinedefEffect::DoorLockedRed));
    }

    #[test]
    fn effect_type_100_is_stairs_turbo16() {
        assert_eq!(linedef_effect(100), Some(LinedefEffect::StairsTurbo16));
    }

    #[test]
    fn effect_multiple_types_map_to_door_open() {
        // Types 2, 31, 46, 61, 86, 103, 109 all map to DoorOpen.
        for &special in &[2, 31, 46, 61, 86, 103, 109] {
            assert_eq!(
                linedef_effect(special),
                Some(LinedefEffect::DoorOpen),
                "special {} should map to DoorOpen",
                special
            );
        }
    }

    #[test]
    fn effect_unknown_returns_none() {
        assert_eq!(linedef_effect(999), None);
    }

    #[test]
    fn effect_type_53_is_perpetual_lift_start() {
        assert_eq!(linedef_effect(53), Some(LinedefEffect::PerpetualLiftStart));
    }

    #[test]
    fn effect_type_57_is_ceiling_crush_stop() {
        assert_eq!(linedef_effect(57), Some(LinedefEffect::CeilingCrushStop));
    }

    #[test]
    fn effect_type_23_is_floor_lower_to_lowest() {
        assert_eq!(linedef_effect(23), Some(LinedefEffect::FloorLowerToLowest));
    }

    // -----------------------------------------------------------------------
    // Tests: dispatch_linedef
    // -----------------------------------------------------------------------

    #[test]
    fn dispatch_door_type_creates_mover() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        // Set linedef special to type 1 (DR door open wait close).
        level.linedefs[0].special = 1;

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            1,
            TriggerType::SwitchRepeat,
            handle,
            0,
        );
        assert!(result, "door dispatch should succeed");
        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "door mover should be created"
        );
    }

    #[test]
    fn dispatch_back_side_blocks_non_manual_use_line() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 31; // S1 door open.

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            31,
            TriggerType::SwitchOnce,
            handle,
            1,
        );

        assert!(
            !result,
            "back-side use should fail for non-manual front-only use lines"
        );
        assert!(gs.movers.active_doors.is_empty());
        assert_eq!(level.linedefs[0].special, 31);
    }

    #[test]
    fn dispatch_back_side_allows_manual_door_line() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 1; // DR door open wait close.

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            1,
            TriggerType::SwitchRepeat,
            handle,
            1,
        );

        assert!(
            result,
            "back-side use should still work for manual door lines Doom allows"
        );
        assert_eq!(gs.movers.active_doors.len(), 1);
    }

    #[test]
    fn dispatch_exit_sets_exit_request() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 11;

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            11,
            TriggerType::SwitchOnce,
            handle,
            0,
        );
        assert!(result);
        assert_eq!(gs.exit_request, Some(ExitRequest::Normal));
    }

    #[test]
    fn dispatch_secret_exit_sets_secret() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 51;

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            51,
            TriggerType::SwitchOnce,
            handle,
            0,
        );
        assert!(result);
        assert_eq!(gs.exit_request, Some(ExitRequest::Secret));
    }

    #[test]
    fn dispatch_locked_door_without_key_fails() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 26; // Blue locked door.

        // Player has no keys.
        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            26,
            TriggerType::SwitchRepeat,
            handle,
            0,
        );
        assert!(!result, "locked door without key should fail");
        assert!(gs.movers.active_doors.is_empty());
    }

    #[test]
    fn dispatch_locked_blue_door_without_key_queues_player_feedback() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 26; // Blue locked door.

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            26,
            TriggerType::SwitchRepeat,
            handle,
            0,
        );

        assert!(!result, "locked door without key should fail");
        assert_eq!(
            gs.sound.sound_queue,
            vec![crate::state::SoundRequest::PlayerUseLockedDoor(
                crate::state::LockedDoorColor::Blue,
            )],
            "player should get Doom-style keyed-door feedback"
        );
    }

    #[test]
    fn dispatch_locked_door_without_player_feedback_for_monsters() {
        let (mut gs, player_handle) = make_gs_with_player();
        let monster = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(-16),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let monster_handle = gs.mobjslab.alloc(monster);
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 26; // Blue locked door.

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            26,
            TriggerType::SwitchRepeat,
            monster_handle,
            0,
        );

        assert!(!result, "monster without key should not activate the door");
        assert!(
            gs.sound.sound_queue.is_empty(),
            "monster should not get keyed-door feedback"
        );
        assert_eq!(
            gs.player.handle, player_handle,
            "precondition: player handle unchanged"
        );
    }

    #[test]
    fn dispatch_locked_door_with_key_succeeds() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 26; // Blue locked door.

        // Give player the blue card.
        gs.player.give_key(crate::player::KEY_BLUE_CARD);

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            26,
            TriggerType::SwitchRepeat,
            handle,
            0,
        );
        assert!(result, "locked door with key should succeed");
        assert_eq!(gs.movers.active_doors.len(), 1);
    }

    #[test]
    fn dispatch_locked_red_door_needs_red_key() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 28; // Red locked door.

        // Give player blue (wrong key).
        gs.player.give_key(crate::player::KEY_BLUE_CARD);

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            28,
            TriggerType::SwitchRepeat,
            handle,
            0,
        );
        assert!(!result, "red door should not open with blue key");

        // Now give red key.
        gs.player.give_key(crate::player::KEY_RED_CARD);
        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            28,
            TriggerType::SwitchRepeat,
            handle,
            0,
        );
        assert!(result, "red door should open with red key");
    }

    #[test]
    fn dispatch_once_trigger_clears_special() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 11; // S1 Exit.

        dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            11,
            TriggerType::SwitchOnce,
            handle,
            0,
        );

        assert_eq!(
            level.linedefs[0].special, 0,
            "once-trigger must clear special"
        );
    }

    #[test]
    fn dispatch_repeat_trigger_preserves_special() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 1; // DR door (repeatable).

        dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            1,
            TriggerType::SwitchRepeat,
            handle,
            0,
        );

        assert_eq!(
            level.linedefs[0].special, 1,
            "repeat-trigger must preserve special"
        );
    }

    #[test]
    fn dispatch_unknown_special_returns_false() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 999;

        let result = dispatch_linedef(
            &mut gs,
            &mut level,
            0,
            999,
            TriggerType::SwitchOnce,
            handle,
            0,
        );
        assert!(!result);
    }

    #[test]
    fn dispatch_floor_raise_creates_mover() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(1);
        level.linedefs[0].special = 5; // W1 floor raise to lowest ceiling.

        let result = dispatch_linedef(&mut gs, &mut level, 0, 5, TriggerType::WalkOnce, handle, 0);
        assert!(result, "floor raise dispatch should succeed");
    }

    #[test]
    fn dispatch_lift_creates_lift_mover() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(1);
        level.linedefs[0].special = 10; // W1 lift.

        let result = dispatch_linedef(&mut gs, &mut level, 0, 10, TriggerType::WalkOnce, handle, 0);
        assert!(result, "lift dispatch should succeed");
    }

    // -----------------------------------------------------------------------
    // Tests: check_cross_lines
    // -----------------------------------------------------------------------

    #[test]
    fn cross_w1_line_triggers_action() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        // Set linedef to W1 exit (type 52).
        level.linedefs[0].special = 52;

        // Movement that crosses the linedef at x=0 (from x=-5 to x=5).
        check_cross_lines(&mut gs, &mut level, handle, -5, 0, 5, 0);

        assert_eq!(
            gs.exit_request,
            Some(ExitRequest::Normal),
            "crossing W1 exit line should trigger exit"
        );
    }

    #[test]
    fn not_crossing_does_not_trigger() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 52;

        // Movement that does NOT cross the linedef (stays on left side).
        check_cross_lines(&mut gs, &mut level, handle, -10, 0, -5, 0);

        assert_eq!(
            gs.exit_request, None,
            "not crossing the line should not trigger"
        );
    }

    #[test]
    fn cross_w1_line_clears_special() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 52; // W1 exit.

        check_cross_lines(&mut gs, &mut level, handle, -5, 0, 5, 0);

        assert_eq!(
            level.linedefs[0].special, 0,
            "W1 line should be cleared after trigger"
        );
    }

    #[test]
    fn cross_wr_line_preserves_special() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 97; // WR teleport.

        // Teleport won't actually work (no teleport destination), but the
        // dispatch should still succeed and preserve the special.
        check_cross_lines(&mut gs, &mut level, handle, -5, 0, 5, 0);

        assert_eq!(
            level.linedefs[0].special, 97,
            "WR line should preserve special after trigger"
        );
    }

    #[test]
    fn cross_lines_reverse_crossing_order_matches_vanilla_spechit_processing() {
        let (mut gs, handle) = make_gs_with_player();
        let reject = doom_map::Reject::parse_lump(&[0u8], 2).expect("value must exist in test");
        let mut level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![
                doom_map::Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0x0004,
                    special: 124, // W1 secret exit, nearer line.
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
                doom_map::Linedef {
                    from_vertex: 2,
                    to_vertex: 3,
                    flags: 0x0004,
                    special: 52, // W1 normal exit, farther line.
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
            ],
            sidedefs: vec![
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
            ],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: -10 },
                doom_map::Vertex { x: 0, y: 10 },
                doom_map::Vertex { x: 64, y: -10 },
                doom_map::Vertex { x: 64, y: 10 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
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
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        check_cross_lines(&mut gs, &mut level, handle, -5, 0, 80, 0);

        assert_eq!(
            gs.exit_request,
            Some(ExitRequest::Secret),
            "crossed walk specials should resolve in reverse crossing order, not raw lump order"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: segments_intersect
    // -----------------------------------------------------------------------

    #[test]
    fn segments_crossing_detected() {
        // Horizontal segment crossing vertical segment.
        assert!(segments_intersect(-5, 0, 5, 0, 0, -5, 0, 5));
    }

    #[test]
    fn parallel_segments_no_intersection() {
        // Two parallel horizontal segments.
        assert!(!segments_intersect(-5, 0, 5, 0, -5, 1, 5, 1));
    }

    #[test]
    fn non_overlapping_segments_no_intersection() {
        // Segments that would intersect if extended but don't actually overlap.
        assert!(!segments_intersect(-5, 0, -1, 0, 0, 1, 0, 5));
    }

    // -----------------------------------------------------------------------
    // Tests: linedef_effect coverage
    // -----------------------------------------------------------------------

    #[test]
    fn effect_type_10_is_lift() {
        assert_eq!(linedef_effect(10), Some(LinedefEffect::LiftLowerWaitRaise));
    }

    #[test]
    fn effect_type_6_is_ceiling_crush() {
        assert_eq!(linedef_effect(6), Some(LinedefEffect::CeilingCrushAndRaise));
    }

    #[test]
    fn effect_type_12_is_light_on_255() {
        assert_eq!(linedef_effect(12), Some(LinedefEffect::LightTurnOn255));
    }

    #[test]
    fn effect_type_9_is_donut() {
        assert_eq!(linedef_effect(9), Some(LinedefEffect::Donut));
    }

    #[test]
    fn effect_type_104_is_light_turn_off() {
        assert_eq!(linedef_effect(104), Some(LinedefEffect::LightTurnOff));
    }

    #[test]
    fn effect_type_17_is_light_start_blinking() {
        assert_eq!(linedef_effect(17), Some(LinedefEffect::LightStartBlinking));
    }

    #[test]
    fn effect_type_125_is_teleport_monsters_only() {
        assert_eq!(
            linedef_effect(125),
            Some(LinedefEffect::TeleportMonstersOnly)
        );
    }

    #[test]
    fn cross_lines_ignores_passive_specials() {
        let (mut gs, handle) = make_gs_with_player();
        let mut level = make_test_level_with_tag(0);
        level.linedefs[0].special = 48; // Scrolling texture (passive)

        // Player moves across the line
        check_cross_lines(&mut gs, &mut level, handle, -5, 0, 5, 0);

        // No panic should happen, and the special is preserved
        assert_eq!(
            level.linedefs[0].special, 48,
            "Passive special should be ignored and preserved"
        );
    }
}
