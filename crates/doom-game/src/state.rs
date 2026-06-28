//! Top-level game simulation state.
//!
//! `GameState` is fully self-contained and `Clone`-able for rollback.
//! It must never contain `Arc`, `Rc`, raw pointers, `HashMap`, or any
//! non-deterministic source (no `Instant::now()`, no OS calls).

use doom_types::Fixed16_16;

use crate::mobj::MobjSlab;
use crate::player::PlayerState;
use doom_types::primitives::Skill;

use crate::random::DoomRng;
use crate::stats::LevelStats;

pub use crate::sound_prop::{SoundPropagation, SoundRequest};

pub use crate::movers::*;

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
    /// The style meter, which tracks rapid multi-kills and assigns a DMC-like style rank.
    #[cfg(feature = "style_meter")]
    pub style: crate::style::StyleMeter,
    #[cfg(feature = "telemetry")]
    /// Tracks spatial player path data over the session.
    pub telemetry: crate::telemetry::SessionTelemetry,
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
            #[cfg(feature = "telemetry")]
            telemetry: crate::telemetry::SessionTelemetry::new(),
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
        assert_eq!(crate::random::RNG_TABLE.len(), 256);
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
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("item must exist in tests")
                .health,
            100
        );

        gs.sync_player_mobj_health();

        // After sync, mobj health matches player state
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("item must exist in tests")
                .health,
            50
        );
    }

    #[test]
    fn test_damage_player() {
        let mut gs = setup_player_gs();
        gs.damage_player(30);

        assert_eq!(gs.player.health(), 70);
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("item must exist in tests")
                .health,
            70
        );
    }

    #[test]
    fn test_heal_player() {
        let mut gs = setup_player_gs();
        gs.damage_player(50);
        assert_eq!(gs.player.health(), 50);

        gs.heal_player(20);
        assert_eq!(gs.player.health(), 70);
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("item must exist in tests")
                .health,
            70
        );

        // Heal up to max capacity
        gs.heal_player(100);
        assert_eq!(gs.player.health(), 100);
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("item must exist in tests")
                .health,
            100
        );
    }

    #[test]
    fn test_heal_player_overheal() {
        let mut gs = setup_player_gs();
        gs.heal_player_overheal(50, 200);

        assert_eq!(gs.player.health(), 150);
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("item must exist in tests")
                .health,
            150
        );

        // Ensure cap works
        gs.heal_player_overheal(100, 200);
        assert_eq!(gs.player.health(), 200);
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("item must exist in tests")
                .health,
            200
        );
    }

    #[test]
    fn test_set_player_health_capped() {
        let mut gs = setup_player_gs();
        gs.set_player_health_capped(120, 150);

        assert_eq!(gs.player.health(), 120);
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("item must exist in tests")
                .health,
            120
        );

        // Ensure cap works
        gs.set_player_health_capped(200, 150);
        assert_eq!(gs.player.health(), 150);
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("item must exist in tests")
                .health,
            150
        );
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
