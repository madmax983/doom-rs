//! Active sector movers, environmental specials, and lighting animations.

/// An animated door or floor/ceiling mover.
///
/// Added to `GameState::active_doors` when a door linedef is activated.
/// Ticked each tic by `specials::tick_doors`.
#[derive(Clone, Debug)]
pub struct DoorMover {
    /// Index into `level.sectors`.
    pub sector: usize,
    /// Target ceiling height (doors) or floor height (floors).
    pub target_height: i16,
    /// Current ceiling/floor height (updated each tic — mirrors the sector value).
    pub current_height: i16,
    /// Speed in map units per tic (positive = opening/rising, negative = closing/lowering).
    pub speed: i16,
    /// `true` = this mover operates on ceiling height, `false` = floor height.
    pub is_ceiling: bool,
    /// Tics to wait at top/bottom before reversing (0 = no wait, no reverse).
    pub wait_tics: i32,
    /// Countdown until the door starts closing again (−1 = permanent open/close).
    pub countdown: i32,
    /// For close-wait-open doors: ceiling height to reopen to (0 = not applicable).
    pub reopen_height: i16,
    /// For close-wait-open doors: tics remaining before reopening (−1 = not applicable).
    pub reopen_countdown: i32,
}

// ---------------------------------------------------------------------------
// Ceiling / floor mover types
// ---------------------------------------------------------------------------

/// Direction a ceiling or floor is currently moving.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MoveDirection {
    /// Moving vertically upward.
    Up,
    /// Moving vertically downward.
    Down,
}

/// The type of ceiling motion behavior.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CeilingType {
    /// Lower ceiling to floor height.
    LowerToFloor,
    /// Perpetual: lower to floor+8, raise back, repeat.
    CrushAndRaise,
    /// Lower to floor+8 and stop.
    LowerAndCrush,
    /// Like CrushAndRaise but faster.
    FastCrushAndRaise,
    /// Crush without sound flag.
    SilentCrush,
    /// Raise ceiling to highest adjacent ceiling and stop.
    RaiseToHighest,
}

/// A ceiling crusher that oscillates between top and bottom heights,
/// damaging actors caught in between.
///
/// Added to `GameState::active_ceilings` when a crusher linedef is activated.
/// Ticked each tic by `specials::tick_ceilings`.
#[derive(Clone, Debug)]
pub struct CeilingMover {
    /// Index into `level.sectors`.
    pub sector_index: usize,
    /// Original ceiling height (return position).
    pub top_height: i16,
    /// Lowest point the ceiling descends to (usually 8 units above floor).
    pub bottom_height: i16,
    /// Movement speed in map units per tic (typically 1 for slow, 2 for fast).
    pub speed: i16,
    /// Normal (un-slowed) speed, remembered for resume after crush slow-down.
    pub normal_speed: i16,
    /// Damage per tic when crushing an actor (typically 10).
    pub crush_damage: i32,
    /// Current movement direction.
    pub direction: MoveDirection,
    /// Some crushers make no sound.
    pub silent: bool,
    /// `true` for one-shot crushers that remove themselves when done,
    /// `false` for perpetual oscillating crushers.
    pub remove_when_done: bool,
    /// Tag from the activating linedef (used by line type 57 to stop crushers).
    pub tag: u16,
    /// The type of ceiling motion.
    pub ceiling_type: CeilingType,
}

/// Defines whether a floor mover applies crushing damage when moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrushBehavior {
    /// Floor damages actors it moves into.
    Crush,
    /// Floor stops or acts normally without causing damage.
    NoCrush,
}

/// Represents the behavior target of a floor mover.
///
/// Used by map specials (like stairs, lifts, and generic moving floors) to
/// define how the destination height is calculated relative to adjacent sectors.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FloorType {
    /// Lower floor to lowest adjacent floor.
    LowerToLowest,
    /// Lower floor to highest adjacent floor.
    LowerToHighest,
    /// Lower floor to nearest (next lowest) adjacent floor.
    LowerToNearest,
    /// Raise floor to highest adjacent floor (unused as standalone, but included).
    RaiseToHighest,
    /// Raise floor to next highest adjacent floor.
    RaiseToNearest,
    /// Raise floor by shortest lower texture height.
    RaiseByTexture,
    /// Raise floor to sector's own ceiling.
    RaiseToCeiling,
    /// Lower floor and change flat/type.
    LowerAndChange,
    /// Raise floor and change flat/type.
    RaiseAndChange,
    /// Raise floor by exactly 24 units.
    Raise24,
    /// Raise floor by exactly 32 units.
    Raise32,
    /// Raise floor with crush damage.
    RaiseCrush,
}

/// A floor that moves to a target height, optionally waits, then returns.
///
/// Used for lifts (lower-wait-raise) and floor raisers/lowerers (one-shot).
/// Added to `GameState::active_floors` when activated.
/// Ticked each tic by `specials::tick_floors`.
#[derive(Clone, Debug)]
pub struct FloorMover {
    /// Index into `level.sectors`.
    pub sector_index: usize,
    /// Destination floor height.
    pub target_height: i16,
    /// Movement speed in map units per tic (typically 1-4).
    pub speed: i16,
    /// Current movement direction.
    pub direction: MoveDirection,
    /// Tics to wait at destination before returning.
    /// `-1` = no wait (one-shot mover that removes itself at target).
    /// `>0` = wait then reverse.
    pub wait_tics: i32,
    /// Height to return to after waiting (original floor height for lifts).
    pub return_height: i16,
    /// Currently in the wait phase.
    pub waiting: bool,
    /// Tics remaining in the wait phase.
    pub wait_remaining: i32,
    /// Does this floor damage actors when raising into them?
    pub crush: CrushBehavior,
    /// Tag from the activating linedef.
    pub tag: u16,
    /// The type of floor motion (for savegame serialization and behavior differentiation).
    pub floor_type: FloorType,
}

// ---------------------------------------------------------------------------
// Perpetual platform
// ---------------------------------------------------------------------------

/// Current movement status of a perpetual platform.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PlatformStatus {
    /// Platform is moving upward.
    Up,
    /// Platform is moving downward.
    Down,
    /// Platform is waiting at a stop before reversing.
    Waiting,
}

/// A perpetual platform that oscillates between low and high heights.
///
/// Used for line types 53, 54, 87, 89.
/// Added to `GameState::active_platforms` when activated.
/// Ticked each tic by `specials::tick_platforms`.
#[derive(Clone, Debug)]
pub struct PerpetualPlatform {
    /// Index into `level.sectors`.
    pub sector_index: usize,
    /// Lowest floor height (lowest adjacent floor).
    pub low_height: i16,
    /// Highest floor height (original sector floor height).
    pub high_height: i16,
    /// Movement speed in map units per tic.
    pub speed: i16,
    /// Tics to wait at each stop before reversing.
    pub wait_tics: i32,
    /// Tics remaining in the current wait phase.
    pub wait_remaining: i32,
    /// Current movement status.
    pub status: PlatformStatus,
    /// Tag from the activating linedef.
    pub tag: u16,
}

// ---------------------------------------------------------------------------
// Lift mover types
// ---------------------------------------------------------------------------

/// Current movement status of a lift.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LiftStatus {
    /// Lift floor is lowering toward `low_height`.
    Lowering,
    /// Lift is waiting at the bottom before raising.
    Waiting,
    /// Lift floor is raising back toward `high_height`.
    Raising,
    /// Lift has completed its cycle and should be removed.
    Done,
}

/// A lift (platform) that lowers, waits, then raises back.
///
/// Added to `GameState::lifts` when a lift linedef is activated.
/// Ticked each tic by `specials::tick_lifts`.
#[derive(Clone, Debug)]
pub struct LiftMover {
    /// Index into `level.sectors`.
    pub sector_index: usize,
    /// Lowest adjacent floor height (destination when lowering).
    pub low_height: i16,
    /// Original floor height before lowering (destination when raising).
    pub high_height: i16,
    /// Movement speed in map units per tic.
    pub speed: i16,
    /// Tics to wait at bottom before raising (typically 105 = 3 seconds).
    pub wait_tics: i32,
    /// Countdown remaining in the wait phase.
    pub wait_remaining: i32,
    /// Current movement status.
    pub status: LiftStatus,
}

/// A flickering or blinking light special.
///
/// Added to `GameState::active_lights` by `specials::spawn_level_specials`.
/// Ticked each tic by `specials::tick_lights`.
#[derive(Clone, Debug)]
pub struct LightSpecial {
    /// Index into `level.sectors`.
    pub sector: usize,
    /// Timer counting down to next toggle.
    pub timer: i32,
    /// Timer period in tics (reset to this value after each toggle).
    pub period: i32,
    /// Light value when in the bright phase.
    pub bright: i16,
    /// Light value when in the dark phase.
    pub dark: i16,
    /// `true` if currently in the bright phase.
    pub is_bright: bool,
}

// ---------------------------------------------------------------------------
// Sector damage types
// ---------------------------------------------------------------------------

/// Type of periodic sector damage applied to a sector.
#[derive(strum_macros::FromRepr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SectorDamageType {
    /// Special 4: Nukage, blink 0.5s (-20% health randomly, ~5 damage per period)
    NukageBlink = 4,
    /// Special 5: Hellslime (-10% health, ~5 damage per period)
    Hellslime = 5,
    /// Special 7: Nukage, no blink (-5% health, ~2 damage per period)
    Nukage = 7,
    /// Special 11: God exit (-20% health + end level when health <= 10)
    GodExit = 11,
    /// Special 16: Super hellslime (-20% health, ~20 damage per period)
    SuperHellslime = 16,
}

// ---------------------------------------------------------------------------
// Sector light effect types (extended)
// ---------------------------------------------------------------------------

/// Type of light effect applied to a sector.
#[derive(strum_macros::FromRepr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum LightEffectType {
    /// Special 1: Light oscillates between base and dark at random intervals.
    BlinkRandom = 1,
    /// Special 2: Light blinks every ~17 tics.
    Blink05s = 2,
    /// Special 3: Light blinks every ~35 tics.
    Blink1s = 3,
    /// Special 8: Light smoothly oscillates.
    Oscillate = 8,
    /// Special 12: Synchronized blink every ~17 tics.
    BlinkSync05s = 12,
    /// Special 13: Synchronized blink every ~35 tics.
    BlinkSync1s = 13,
    /// Special 17: Random light variation (fire flicker).
    FireFlicker = 17,
}

/// Extended sector light effect with per-sector state tracking.
///
/// Created by `specials::init_sector_lights` from sector specials.
/// Ticked each tic by `specials::tick_sector_lights`.
#[derive(Debug, Clone)]
pub struct SectorLightEffect {
    /// Index into `level.sectors`.
    pub sector_index: usize,
    /// Type of light animation.
    pub effect_type: LightEffectType,
    /// Base (bright) light level for this sector.
    pub base_light: i16,
    /// Minimum (dark) light level for this sector.
    pub min_light: i16,
    /// Timer counting down to next state change.
    pub timer: u32,
}

// ---------------------------------------------------------------------------
// Scrolling wall / conveyor belt types
// ---------------------------------------------------------------------------

/// A wall whose texture scrolls horizontally or vertically each tic.
///
/// Added to `GameState::scrolling_walls` during level setup by
/// `specials::init_scrolling_walls`. Ticked each tic by
/// `specials::tick_scrollers`.
///
/// Line type 48: scroll left (speed_x = 1, speed_y = 0).
/// Line type 85: scroll right (speed_x = -1, speed_y = 0).
#[derive(Clone, Debug)]
pub struct ScrollingWall {
    /// Index into `level.linedefs` for the scrolling linedef.
    pub linedef_index: usize,
    /// Horizontal scroll speed in texels per tic (positive = scroll left).
    pub speed_x: i16,
    /// Vertical scroll speed in texels per tic (positive = scroll down, 0 for most).
    pub speed_y: i16,
    /// Accumulated horizontal offset (grows each tic by `speed_x`).
    pub accumulated_x: i32,
    /// Accumulated vertical offset (grows each tic by `speed_y`).
    pub accumulated_y: i32,
}

/// A conveyor belt sector that pushes actors standing on it.
///
/// Added to `GameState::conveyors` during level setup by
/// `specials::init_conveyors`. Ticked each tic by
/// `specials::tick_conveyors`.
///
/// Line type 253: scroll floor + push things.
/// Line type 254: scroll floor + push things + scroll wall.
/// Line type 255: scroll wall using linedef offsets (generalized).
#[derive(Clone, Debug)]
pub struct ConveyorBelt {
    /// Index into `level.sectors` for the conveyor sector.
    pub sector_index: usize,
    /// Push force X component (fixed-point or map units per tic).
    pub push_x: i32,
    /// Push force Y component.
    pub push_y: i32,
    /// Direction angle in degrees (0-359, for reference).
    pub direction: i16,
    /// Push magnitude (speed of the conveyor).
    pub speed: i16,
}

// ---------------------------------------------------------------------------
// Doom's deterministic RNG
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// GameState
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// GameState Sub-structs
// ---------------------------------------------------------------------------

/// Active sector movers and environmental specials.
#[derive(Clone, Debug, Default)]
pub struct SectorMovers {
    /// Active door/floor/ceiling movers (ticked by `specials::tick_doors`).
    pub active_doors: Vec<DoorMover>,
    /// Active light specials (ticked by `specials::tick_lights`).
    pub active_lights: Vec<LightSpecial>,
    /// Active ceiling movers / crushers (ticked by `specials::tick_ceilings`).
    pub active_ceilings: Vec<CeilingMover>,
    /// Active floor movers / lifts (ticked by `specials::tick_floors`).
    pub active_floors: Vec<FloorMover>,
    /// Active perpetual platforms (ticked by `specials::tick_platforms`).
    pub active_platforms: Vec<PerpetualPlatform>,
    /// Active lifts (lower-wait-raise) (ticked by `specials::tick_lifts`).
    pub lifts: Vec<LiftMover>,
    /// Extended sector light effects (ticked by `specials::tick_sector_lights`).
    pub sector_lights: Vec<SectorLightEffect>,
    /// Active scrolling wall textures (ticked by `specials::tick_scrollers`).
    pub scrolling_walls: Vec<ScrollingWall>,
    /// Active conveyor belt sectors (ticked by `specials::tick_conveyors`).
    pub conveyors: Vec<ConveyorBelt>,
}
