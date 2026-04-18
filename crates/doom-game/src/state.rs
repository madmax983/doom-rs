//! Top-level game simulation state.
//!
//! `GameState` is fully self-contained and `Clone`-able for rollback.
//! It must never contain `Arc`, `Rc`, raw pointers, `HashMap`, or any
//! non-deterministic source (no `Instant::now()`, no OS calls).

use doom_types::Fixed16_16;

use crate::mobj::{MobjHandle, MobjSlab};
use crate::player::PlayerState;
use crate::spawn::Skill;
use doom_types::mobj_kind::MobjKind;

// ---------------------------------------------------------------------------
// Sound events
// ---------------------------------------------------------------------------

/// Represents the required key color when a locked door is denied access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockedDoorColor {
    /// A blue keycard or skull key is required.
    Blue,
    /// A red keycard or skull key is required.
    Red,
    /// A yellow keycard or skull key is required.
    Yellow,
}

/// A sound event emitted by the game simulation.
///
/// The app (doom-app) drains `GameState::sound_queue` each tic and maps each
/// variant to the appropriate WAD lump name for playback.  The game crate
/// intentionally has no audio dependency — it only describes *what* happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundRequest {
    /// Monster spotted the player / woke up.
    /// Fields: (kind, origin_handle, map_x, map_y), where map_x/map_y are the
    /// emitter's position in Fixed16_16 map units.
    MonsterWake(
        MobjKind,
        MobjHandle,
        doom_types::Fixed16_16,
        doom_types::Fixed16_16,
    ),
    /// Monster was killed.  Fields: (kind, origin_handle, map_x, map_y).
    MonsterDie(
        MobjKind,
        MobjHandle,
        doom_types::Fixed16_16,
        doom_types::Fixed16_16,
    ),
    /// Monster fired a hitscan or projectile attack.
    /// Fields: (kind, origin_handle, map_x, map_y).
    MonsterAttack(
        MobjKind,
        MobjHandle,
        doom_types::Fixed16_16,
        doom_types::Fixed16_16,
    ),
    /// Player weapon actually fired this tic.
    PlayerWeaponFire(crate::player::WeaponType),
    /// Super shotgun break-open sound.
    PlayerSuperShotgunOpen,
    /// Super shotgun shell-load sound.
    PlayerSuperShotgunLoad,
    /// Super shotgun close-and-lock sound.
    PlayerSuperShotgunClose,
    /// Player died.
    PlayerDie,
    /// Player pressed use into a blocking ordinary wall.
    PlayerUseFail,
    /// Player tried to use a keyed door without the required key color.
    PlayerUseLockedDoor(LockedDoorColor),
}

impl SoundRequest {
    pub fn emitter(
        &self,
        player_x: doom_types::Fixed16_16,
        player_y: doom_types::Fixed16_16,
    ) -> Option<(doom_types::Fixed16_16, doom_types::Fixed16_16)> {
        match *self {
            SoundRequest::MonsterWake(_, _, x, y)
            | SoundRequest::MonsterAttack(_, _, x, y)
            | SoundRequest::MonsterDie(_, _, x, y) => Some((x, y)),
            SoundRequest::PlayerWeaponFire(_)
            | SoundRequest::PlayerSuperShotgunOpen
            | SoundRequest::PlayerSuperShotgunLoad
            | SoundRequest::PlayerSuperShotgunClose => Some((player_x, player_y)),
            SoundRequest::PlayerDie
            | SoundRequest::PlayerUseFail
            | SoundRequest::PlayerUseLockedDoor(_) => None,
        }
    }

    pub fn origin_handle(
        &self,
        player_origin: Option<crate::mobj::MobjHandle>,
    ) -> Option<crate::mobj::MobjHandle> {
        match *self {
            SoundRequest::MonsterWake(_, handle, _, _)
            | SoundRequest::MonsterAttack(_, handle, _, _)
            | SoundRequest::MonsterDie(_, handle, _, _) => Some(handle),
            SoundRequest::PlayerWeaponFire(_)
            | SoundRequest::PlayerSuperShotgunOpen
            | SoundRequest::PlayerSuperShotgunLoad
            | SoundRequest::PlayerSuperShotgunClose => player_origin,
            SoundRequest::PlayerDie
            | SoundRequest::PlayerUseFail
            | SoundRequest::PlayerUseLockedDoor(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Exit request
// ---------------------------------------------------------------------------

/// The type of level exit the player triggered.
///
/// Set by `activate_linedef` when a switch or walk-trigger exit line is
/// activated.  Cleared to `None` at the start of each tick so the caller
/// can observe it exactly once.
#[derive(strum_macros::FromRepr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitRequest {
    /// Normal exit (next sequential map).
    Normal,
    /// Secret exit (secret map).
    Secret,
}

// ---------------------------------------------------------------------------
// Sector mover / light thinker types
// ---------------------------------------------------------------------------

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

/// Doom's original 256-entry pseudo-random number table (from `m_random.c`).
///
/// The sequence is deterministic and identical on all network peers, making
/// it safe to use inside the game simulation.  `DoomRng::next_byte()` returns
/// successive bytes from this table, wrapping at index 255.
pub static RNG_TABLE: [u8; 256] = [
    0, 8, 109, 220, 222, 241, 149, 107, 75, 248, 254, 140, 16, 66, 74, 21, 211, 47, 80, 242, 154,
    27, 205, 253, 197, 224, 120, 244, 122, 173, 177, 144, 96, 255, 183, 114, 170, 72, 91, 148, 88,
    197, 243, 40, 204, 114, 237, 236, 64, 226, 104, 152, 112, 94, 237, 158, 106, 236, 168, 185,
    254, 241, 107, 208, 190, 68, 200, 60, 239, 88, 107, 55, 197, 236, 132, 150, 12, 217, 103, 177,
    166, 166, 130, 130, 172, 170, 247, 234, 60, 82, 18, 100, 250, 225, 58, 170, 26, 109, 59, 251,
    120, 125, 172, 100, 59, 181, 121, 228, 191, 130, 63, 185, 151, 189, 79, 92, 38, 30, 94, 51,
    216, 211, 165, 203, 28, 200, 216, 219, 104, 108, 175, 186, 241, 217, 45, 20, 219, 163, 43, 55,
    197, 228, 237, 51, 79, 73, 145, 181, 180, 97, 237, 232, 241, 161, 166, 174, 28, 116, 190, 174,
    16, 64, 44, 126, 161, 229, 141, 81, 89, 169, 253, 226, 116, 147, 143, 224, 11, 223, 175, 137,
    65, 68, 66, 239, 166, 196, 206, 241, 26, 189, 221, 244, 12, 201, 79, 167, 243, 246, 200, 240,
    205, 24, 113, 79, 221, 253, 2, 79, 199, 200, 40, 167, 170, 93, 69, 80, 84, 112, 210, 8, 137,
    33, 208, 185, 218, 130, 110, 91, 162, 236, 52, 249, 228, 252, 154, 232, 12, 128, 252, 183, 41,
    108, 195, 135, 183, 46, 64, 183, 231, 238, 36, 90, 201, 139, 254, 33,
];

/// Deterministic Doom RNG.
///
/// Wraps an index into `RNG_TABLE`, advancing by 1 each call (mod 256).
/// The index is part of `GameState` and is included in snapshots.
#[derive(Clone, Debug, Default)]
pub struct DoomRng {
    /// Current position in `RNG_TABLE` (always in `0..256`).
    index: u32,
}

impl DoomRng {
    /// Create a new RNG at index 0 (start of table).
    pub fn new() -> Self {
        Self { index: 0 }
    }

    /// Return the next random byte and advance the index.
    #[inline]
    pub fn next_byte(&mut self) -> u8 {
        let val = RNG_TABLE[(self.index & 255) as usize];
        self.index = (self.index + 1) & 255;
        val
    }

    /// Current table index (for snapshot / serialization).
    #[inline]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Restore to a specific index (for snapshot restore).
    #[inline]
    pub fn set_index(&mut self, index: u32) {
        self.index = index & 255;
    }
}

// ---------------------------------------------------------------------------
// GameState
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// GameState Sub-structs
// ---------------------------------------------------------------------------

/// End-of-level statistics and map tracking.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelStats {
    /// Number of monsters killed by the player so far.
    pub kill_count: u32,
    /// Number of special items collected by the player.
    pub item_count: u32,
    /// Number of secret areas discovered.
    pub secret_count: u32,
    /// Total killable monsters in the map (for percentage display).
    pub total_kills: u32,
    /// Total collectable items.
    pub total_items: u32,
    /// Total secret sectors in the map (sectors with special type 9).
    pub total_secrets: u32,
    /// Number of tics elapsed in the current level (incremented each tick).
    pub level_time: u32,
}

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

/// Sound propagation and event queues.
#[derive(Clone, Debug, Default)]
pub struct SoundPropagation {
    /// Per-sector sound target: which actor made noise that this sector "heard".
    /// Indexed by sector index. `None` = no noise has reached this sector.
    pub sound_targets: Vec<Option<MobjHandle>>,
    /// Per-sector generation counter for flood-fill visited tracking.
    pub sound_traversed: Vec<u32>,
    /// Current sound generation counter.
    pub sound_gen: u32,
    /// Sound events queued this tic.
    pub sound_queue: Vec<SoundRequest>,
}

/// Complete, self-contained game simulation state.
///
/// All simulation ticks are pure functions of this struct + the (immutable)
/// level geometry.  The struct can be `Clone`d for rollback netcode.
///
/// # Determinism requirements
/// - No `HashMap` — use `Vec` + sorted indices or `BTreeMap`.
/// - No `Instant::now()` inside tic logic.
/// - No `Arc`/`Rc` — everything must deep-clone cleanly.
/// - RNG state (`rng`) is the only source of "randomness".
#[derive(Clone, Debug)]
pub struct GameState {
    /// Monotonically increasing tic counter (wraps at `u32::MAX`).
    pub tic_num: u32,

    /// Deterministic RNG — must advance identically on all peers.
    pub rng: DoomRng,

    /// All live (and recently-killed) actors.
    pub mobjslab: MobjSlab,

    /// Player 1 state.
    pub player: PlayerState,

    /// Current level identifier (e.g. `"E1M1"`).
    pub level_name: String,

    /// Player statistics tracking for the end-of-level intermission screen.
    pub stats: LevelStats,

    /// Manager for active doors, lifts, ceilings, and crushers.
    pub movers: SectorMovers,

    /// Sound emission and propagation state (blockmap sound zones).
    pub sound: SoundPropagation,

    /// Level exit requested this tic (cleared to `None` at start of each tick).
    pub exit_request: Option<ExitRequest>,

    // --- Automap visibility state ---
    /// Per-linedef visibility flag: `true` if the player has visited a
    /// subsector adjacent to this linedef.
    ///
    /// Sized to `level.linedefs.len()` by `automap::init_seen_lines` or
    /// lazily resized by `mark_lines_seen`.
    pub seen_lines: Vec<bool>,

    // --- Difficulty ---
    /// Current skill level (affects Nightmare respawning).
    pub skill: Skill,

    // --- Boss Brain (Icon of Sin) ---
    /// Set `true` once the Boss Brain's see state fires; cubes only
    /// start spawning after this flag is set.
    pub brain_awake: bool,
    /// Spawn spot positions collected from DoomEd thing type 87.
    pub brain_targets: Vec<(Fixed16_16, Fixed16_16)>,
    /// Round-robin index into `brain_targets` for the next cube.
    pub brain_target_index: usize,
    #[cfg(feature = "style_meter")]
    pub style: crate::style::StyleMeter,
}

impl GameState {
    /// Create a fresh game state for a pistol-start entry on `level_name`.
    ///
    /// The caller is responsible for:
    /// 1. Spawning the player `Mobj` into `mobjslab`.
    /// 2. Setting `player.handle` to the returned handle.
    /// 3. Spawning all level actors.
    pub fn new(level_name: &str) -> Self {
        Self {
            tic_num: 0,
            rng: DoomRng::new(),
            mobjslab: MobjSlab::new(),
            player: PlayerState::default(),
            level_name: level_name.to_string(),
            stats: LevelStats::default(),
            movers: SectorMovers::default(),
            sound: SoundPropagation::default(),
            exit_request: None,
            seen_lines: Vec::new(),
            skill: Skill::Medium,
            brain_awake: false,
            brain_targets: Vec::new(),
            brain_target_index: 0,
            #[cfg(feature = "style_meter")]
            style: crate::style::StyleMeter::new(),
        }
    }

    /// Mirror `PlayerState::health()` onto the live player mobj.
    ///
    /// Several gameplay systems still consult the player mobj health directly
    /// (for example monster target liveness), so pickup/heal/damage paths must
    /// keep both representations aligned.
    pub fn sync_player_mobj_health(&mut self) {
        let health = self.player.health();
        if let Some(player_mo) = self.mobjslab.get_mut(self.player.handle) {
            player_mo.health = health;
        }
    }

    /// Apply damage to the player and keep the player mobj health in sync.
    pub fn damage_player(&mut self, amount: i32) {
        self.player.apply_damage(amount);
        self.sync_player_mobj_health();
    }

    /// Heal the player up to the normal cap and keep the player mobj in sync.
    pub fn heal_player(&mut self, amount: i32) {
        self.player.heal(amount);
        self.sync_player_mobj_health();
    }

    /// Heal the player past 100 up to `cap`, keeping the player mobj in sync.
    pub fn heal_player_overheal(&mut self, amount: i32, cap: i32) {
        self.player.heal_overheal(amount, cap);
        self.sync_player_mobj_health();
    }

    /// Set the player's health directly and keep the player mobj in sync.
    pub fn set_player_health_capped(&mut self, value: i32, cap: i32) {
        self.player.set_health_capped(value, cap);
        self.sync_player_mobj_health();
    }

    // -----------------------------------------------------------------------
    // P_Random helpers — deterministic RNG used for all game randomness
    // -----------------------------------------------------------------------

    /// Return the next random byte from Doom's deterministic RNG table and
    /// advance the index.
    ///
    /// Port of `P_Random()` from `m_random.c`.
    #[inline]
    pub fn p_random(&mut self) -> u8 {
        self.rng.next_byte()
    }

    /// Return a random value in `[min, max]` using `p_random`.
    ///
    /// If `min >= max`, returns `min`.
    pub fn p_random_range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        // Use abs_diff and saturating_add to prevent i32 overflow
        // on extremely large ranges (e.g., i32::MIN to i32::MAX).
        let span = min.abs_diff(max).saturating_add(1);
        let r = self.p_random() as u32;

        let offset = r % span;
        // Compute securely in i64 to avoid wrapping the u32 offset into a negative i32.
        let result = (min as i64) + (offset as i64);
        result.clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }

    /// Return `p_random() as i32 - p_random() as i32`.
    ///
    /// Result is in `[-255, 255]`.  Used for angle spread and other symmetric
    /// randomness (e.g. bullet spread, melee miss offset).
    ///
    /// Port of `P_SubRandom()` from various Doom source files.
    pub fn p_subrandom(&mut self) -> i32 {
        let a = self.p_random() as i32;
        let b = self.p_random() as i32;
        a - b
    }

    // -----------------------------------------------------------------------
    // Automap visibility — mark linedefs seen during BSP traversal
    // -----------------------------------------------------------------------

    /// Mark all linedefs adjacent to the given subsector as "seen" on the
    /// automap.
    ///
    /// Should be called during BSP traversal (rendering) for each subsector
    /// the player can see. The `seen_lines` vector is lazily resized to match
    /// the level's linedef count if needed.
    pub fn mark_lines_seen(&mut self, subsector_idx: usize, level: &doom_map::Level) {
        crate::automap::mark_lines_seen(&mut self.seen_lines, subsector_idx, level);
    }

    /// Return the accumulated scroll offset for a given linedef index.
    ///
    /// Returns `(0, 0)` if no `ScrollingWall` exists for this linedef.
    /// The renderer adds these offsets to the sidedef's `x_offset`/`y_offset`
    /// when drawing the wall texture.
    pub fn get_scroll_offset(&self, linedef_index: usize) -> (i32, i32) {
        for sw in &self.movers.scrolling_walls {
            if sw.linedef_index == linedef_index {
                return (sw.accumulated_x, sw.accumulated_y);
            }
        }
        (0, 0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_table_has_256_entries() {
        assert_eq!(RNG_TABLE.len(), 256);
    }

    #[test]
    fn rng_wraps_at_256() {
        let mut rng = DoomRng::new();
        for _ in 0..256 {
            rng.next_byte();
        }
        assert_eq!(rng.index(), 0, "RNG must wrap to index 0 after 256 calls");
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = DoomRng::new();
        let mut b = DoomRng::new();
        for _ in 0..256 {
            assert_eq!(a.next_byte(), b.next_byte());
        }
    }

    #[test]
    fn rng_snapshot_restore() {
        let mut rng = DoomRng::new();
        for _ in 0..77 {
            rng.next_byte();
        }
        let saved = rng.index();
        let seq1: Vec<u8> = (0..10).map(|_| rng.next_byte()).collect();
        rng.set_index(saved);
        let seq2: Vec<u8> = (0..10).map(|_| rng.next_byte()).collect();
        assert_eq!(
            seq1, seq2,
            "RNG must replay identical sequence after set_index"
        );
    }

    #[test]
    fn game_state_new_is_clean() {
        let gs = GameState::new("E1M1");
        assert_eq!(gs.tic_num, 0);
        assert_eq!(gs.level_name, "E1M1");
        assert!(gs.mobjslab.is_empty());
        assert_eq!(gs.stats.kill_count, 0);
    }

    #[test]
    fn game_state_clone_is_independent() {
        let mut gs = GameState::new("E1M1");
        gs.tic_num = 42;
        let gs2 = gs.clone();
        assert_eq!(gs2.tic_num, 42);
        gs.tic_num = 99;
        assert_eq!(
            gs2.tic_num, 42,
            "clone must not share tic_num with original"
        );
    }

    fn setup_player_gs() -> GameState {
        let mut gs = GameState::new("E1M1");
        let mut player_mo = crate::mobj::Mobj::new(
            doom_types::mobj_kind::MobjKind::Player,
            doom_types::Fixed16_16::ZERO,
            doom_types::Fixed16_16::ZERO,
            doom_types::Bam::ZERO,
        );
        player_mo.health = 100;
        let handle = gs.mobjslab.alloc(player_mo);
        gs.player.handle = handle;
        gs.player.set_health_capped(100, 100);
        gs
    }

    #[test]
    fn test_sync_player_mobj_health() {
        let mut gs = setup_player_gs();
        gs.player.set_health_capped(50, 100);

        // Before sync, mobj health is still 100
        assert_eq!(gs.mobjslab.get(gs.player.handle).unwrap().health, 100);

        gs.sync_player_mobj_health();

        // After sync, mobj health matches player state
        assert_eq!(gs.mobjslab.get(gs.player.handle).unwrap().health, 50);
    }

    #[test]
    fn test_damage_player() {
        let mut gs = setup_player_gs();
        gs.damage_player(30);

        assert_eq!(gs.player.health(), 70);
        assert_eq!(gs.mobjslab.get(gs.player.handle).unwrap().health, 70);
    }

    #[test]
    fn test_heal_player() {
        let mut gs = setup_player_gs();
        gs.damage_player(50);
        assert_eq!(gs.player.health(), 50);

        gs.heal_player(20);
        assert_eq!(gs.player.health(), 70);
        assert_eq!(gs.mobjslab.get(gs.player.handle).unwrap().health, 70);

        // Heal up to max capacity
        gs.heal_player(100);
        assert_eq!(gs.player.health(), 100);
        assert_eq!(gs.mobjslab.get(gs.player.handle).unwrap().health, 100);
    }

    #[test]
    fn test_heal_player_overheal() {
        let mut gs = setup_player_gs();
        gs.heal_player_overheal(50, 200);

        assert_eq!(gs.player.health(), 150);
        assert_eq!(gs.mobjslab.get(gs.player.handle).unwrap().health, 150);

        // Ensure cap works
        gs.heal_player_overheal(100, 200);
        assert_eq!(gs.player.health(), 200);
        assert_eq!(gs.mobjslab.get(gs.player.handle).unwrap().health, 200);
    }

    #[test]
    fn test_set_player_health_capped() {
        let mut gs = setup_player_gs();
        gs.set_player_health_capped(120, 150);

        assert_eq!(gs.player.health(), 120);
        assert_eq!(gs.mobjslab.get(gs.player.handle).unwrap().health, 120);

        // Ensure cap works
        gs.set_player_health_capped(200, 150);
        assert_eq!(gs.player.health(), 150);
        assert_eq!(gs.mobjslab.get(gs.player.handle).unwrap().health, 150);
    }

    #[test]
    fn test_p_random_range() {
        let mut gs = GameState::new("E1M1");

        // Single value
        assert_eq!(gs.p_random_range(5, 5), 5);
        assert_eq!(gs.p_random_range(10, 5), 10); // min > max returns min

        // Small range
        let mut found_min = false;
        let mut found_max = false;
        for _ in 0..1000 {
            let v = gs.p_random_range(1, 10);
            assert!((1..=10).contains(&v));
            if v == 1 {
                found_min = true;
            }
            if v == 10 {
                found_max = true;
            }
        }
        assert!(found_min && found_max);

        // Negative range
        let mut found_min_neg = false;
        let mut found_max_neg = false;
        for _ in 0..1000 {
            let v = gs.p_random_range(-20, -10);
            assert!((-20..=-10).contains(&v));
            if v == -20 {
                found_min_neg = true;
            }
            if v == -10 {
                found_max_neg = true;
            }
        }
        assert!(found_min_neg && found_max_neg);

        // Range crossing zero
        let v = gs.p_random_range(-10, 10);
        assert!((-10..=10).contains(&v));

        // Large range that would overflow max - min
        let v = gs.p_random_range(-2_000_000_000, 2_000_000_000);
        assert!((-2_000_000_000..=2_000_000_000).contains(&v));

        // Extreme i32 range
        let _v = gs.p_random_range(i32::MIN, i32::MAX);
        // This should not panic
    }

    #[test]
    fn test_p_subrandom() {
        let mut gs = GameState::new("E1M1");

        let mut found_neg = false;
        let mut found_pos = false;
        for _ in 0..1000 {
            let v = gs.p_subrandom();
            assert!((-255..=255).contains(&v));
            if v < 0 {
                found_neg = true;
            }
            if v > 0 {
                found_pos = true;
            }
        }
        assert!(found_neg && found_pos);
    }
}
