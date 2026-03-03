//! Top-level game simulation state.
//!
//! `GameState` is fully self-contained and `Clone`-able for rollback.
//! It must never contain `Arc`, `Rc`, raw pointers, `HashMap`, or any
//! non-deterministic source (no `Instant::now()`, no OS calls).

use crate::mobj::{MobjHandle, MobjSlab};
use crate::player::PlayerState;

// ---------------------------------------------------------------------------
// Exit request
// ---------------------------------------------------------------------------

/// The type of level exit the player triggered.
///
/// Set by `activate_linedef` when a switch or walk-trigger exit line is
/// activated.  Cleared to `None` at the start of each tick so the caller
/// can observe it exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

// ---------------------------------------------------------------------------
// Ceiling / floor mover types
// ---------------------------------------------------------------------------

/// Direction a ceiling or floor is currently moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
}

/// The type of ceiling motion behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// The type of floor motion behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub crush: bool,
    /// Tag from the activating linedef.
    pub tag: u16,
    /// The type of floor motion (for savegame serialization and behavior differentiation).
    pub floor_type: FloorType,
}

// ---------------------------------------------------------------------------
// Perpetual platform
// ---------------------------------------------------------------------------

/// Current movement status of a perpetual platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
// Sector light effect types (extended)
// ---------------------------------------------------------------------------

/// Type of light effect applied to a sector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightEffectType {
    /// Special 1: Light oscillates between base and dark at random intervals.
    BlinkRandom,
    /// Special 2: Light blinks every ~17 tics.
    Blink05s,
    /// Special 3: Light blinks every ~35 tics.
    Blink1s,
    /// Special 8: Light smoothly oscillates.
    Oscillate,
    /// Special 12: Synchronized blink every ~17 tics.
    BlinkSync05s,
    /// Special 13: Synchronized blink every ~35 tics.
    BlinkSync1s,
    /// Special 17: Random light variation (fire flicker).
    FireFlicker,
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
/// it safe to use inside the game simulation.  `DoomRng::next()` returns
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
    pub fn next(&mut self) -> u8 {
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

    // --- End-of-level statistics ---
    pub kill_count: u32,
    pub item_count: u32,
    pub secret_count: u32,
    /// Total killable monsters in the map (for percentage display).
    pub total_kills: u32,
    /// Total collectable items.
    pub total_items: u32,
    /// Total secret sectors in the map (sectors with special type 9).
    pub total_secrets: u32,

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

    /// Level exit requested this tic (cleared to `None` at start of each tick).
    pub exit_request: Option<ExitRequest>,

    /// Number of tics elapsed in the current level (incremented each tick).
    pub level_time: u32,

    // --- Sound propagation state ---

    /// Per-sector sound target: which actor made noise that this sector "heard".
    ///
    /// Indexed by sector index.  `None` = no noise has reached this sector.
    /// Resized to `level.sectors.len()` by `sound::init_sound_state`.
    pub sound_targets: Vec<Option<MobjHandle>>,

    /// Per-sector generation counter for flood-fill visited tracking.
    ///
    /// Avoids clearing the whole vec each time `p_noise_alert` runs.
    /// A sector is considered "visited this generation" when
    /// `sound_traversed[s] >= sound_gen`.
    pub sound_traversed: Vec<u32>,

    /// Current sound generation counter.
    ///
    /// Incremented each time `p_noise_alert` is called to mark a new
    /// flood-fill pass.
    pub sound_gen: u32,
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
            kill_count: 0,
            item_count: 0,
            secret_count: 0,
            total_kills: 0,
            total_items: 0,
            total_secrets: 0,
            active_doors: Vec::new(),
            active_lights: Vec::new(),
            active_ceilings: Vec::new(),
            active_floors: Vec::new(),
            active_platforms: Vec::new(),
            lifts: Vec::new(),
            sector_lights: Vec::new(),
            scrolling_walls: Vec::new(),
            conveyors: Vec::new(),
            exit_request: None,
            level_time: 0,
            sound_targets: Vec::new(),
            sound_traversed: Vec::new(),
            sound_gen: 0,
        }
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
        self.rng.next()
    }

    /// Return a random value in `[min, max]` using `p_random`.
    ///
    /// If `min >= max`, returns `min`.
    pub fn p_random_range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u32;
        let r = self.p_random() as u32;
        min + (r % span) as i32
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

    /// Return the accumulated scroll offset for a given linedef index.
    ///
    /// Returns `(0, 0)` if no `ScrollingWall` exists for this linedef.
    /// The renderer adds these offsets to the sidedef's `x_offset`/`y_offset`
    /// when drawing the wall texture.
    pub fn get_scroll_offset(&self, linedef_index: usize) -> (i32, i32) {
        for sw in &self.scrolling_walls {
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
            rng.next();
        }
        assert_eq!(rng.index(), 0, "RNG must wrap to index 0 after 256 calls");
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = DoomRng::new();
        let mut b = DoomRng::new();
        for _ in 0..256 {
            assert_eq!(a.next(), b.next());
        }
    }

    #[test]
    fn rng_snapshot_restore() {
        let mut rng = DoomRng::new();
        for _ in 0..77 {
            rng.next();
        }
        let saved = rng.index();
        let seq1: Vec<u8> = (0..10).map(|_| rng.next()).collect();
        rng.set_index(saved);
        let seq2: Vec<u8> = (0..10).map(|_| rng.next()).collect();
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
        assert_eq!(gs.kill_count, 0);
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
}
