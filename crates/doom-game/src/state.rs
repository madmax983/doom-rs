//! Top-level game simulation state.
//!
//! `GameState` is fully self-contained and `Clone`-able for rollback.
//! It must never contain `Arc`, `Rc`, raw pointers, `HashMap`, or any
//! non-deterministic source (no `Instant::now()`, no OS calls).

use crate::mobj::MobjSlab;
use crate::player::PlayerState;

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
// Doom's deterministic RNG
// ---------------------------------------------------------------------------

/// Doom's original 256-entry pseudo-random number table (from `m_random.c`).
///
/// The sequence is deterministic and identical on all network peers, making
/// it safe to use inside the game simulation.  `DoomRng::next()` returns
/// successive bytes from this table, wrapping at index 255.
pub static RNG_TABLE: [u8; 256] = [
      0,   8, 109, 220, 222, 241, 149, 107,  75, 248, 254, 140,  16,  66,  74,  21,
    211,  47,  80, 242, 154,  27, 205, 253, 197, 224, 120, 244, 122, 173, 177, 144,
     96, 255, 183, 114, 170,  72,  91, 148,  88, 197, 243,  40, 204, 114, 237, 236,
     64, 226, 104, 152, 112,  94, 237, 158, 106, 236, 168, 185, 254, 241, 107, 208,
    190,  68, 200,  60, 239,  88, 107,  55, 197, 236, 132, 150,  12, 217, 103, 177,
    166, 166, 130, 130, 172, 170, 247, 234,  60,  82,  18, 100, 250, 225,  58, 170,
     26, 109,  59, 251, 120, 125, 172, 100,  59, 181, 121, 228, 191, 130,  63, 185,
    151, 189,  79,  92,  38,  30,  94,  51, 216, 211, 165, 203,  28, 200, 216, 219,
    104, 108, 175, 186, 241, 217,  45,  20, 219, 163,  43,  55, 197, 228, 237,  51,
     79,  73, 145, 181, 180,  97, 237, 232, 241, 161, 166, 174,  28, 116, 190, 174,
     16,  64,  44, 126, 161, 229, 141,  81,  89, 169, 253, 226, 116, 147, 143, 224,
     11, 223, 175, 137,  65,  68,  66, 239, 166, 196, 206, 241,  26, 189, 221, 244,
     12, 201,  79, 167, 243, 246, 200, 240, 205,  24, 113,  79, 221, 253,   2,  79,
    199, 200,  40, 167, 170,  93,  69,  80,  84, 112, 210,   8, 137,  33, 208, 185,
    218, 130, 110,  91, 162, 236,  52, 249, 228, 252, 154, 232,  12, 128, 252, 183,
     41, 108, 195, 135, 183,  46,  64, 183, 231, 238,  36,  90, 201, 139, 254,  33,
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
    pub kill_count:   u32,
    pub item_count:   u32,
    pub secret_count: u32,
    /// Total killable monsters in the map (for percentage display).
    pub total_kills:  u32,
    /// Total collectable items.
    pub total_items:  u32,

    /// Active door/floor/ceiling movers (ticked by `specials::tick_doors`).
    pub active_doors:  Vec<DoorMover>,
    /// Active light specials (ticked by `specials::tick_lights`).
    pub active_lights: Vec<LightSpecial>,
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
            active_doors: Vec::new(),
            active_lights: Vec::new(),
        }
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
        assert_eq!(seq1, seq2, "RNG must replay identical sequence after set_index");
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
        assert_eq!(gs2.tic_num, 42, "clone must not share tic_num with original");
    }
}
