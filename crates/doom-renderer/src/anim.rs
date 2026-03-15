//! Animated texture and flat management, plus switch texture pairs.
//!
//! Doom has hardcoded animation sequences for both floor/ceiling flats and wall
//! textures.  Each sequence is defined by a list of frame names that cycle at a
//! rate of 8 game tics per frame (~4.4 Hz at 35 tics/sec).
//!
//! Animation is **global** -- all instances of a texture in the same sequence
//! show the same frame at any given tic.  The animation state is advanced once
//! per game tic via [`AnimState::tick`].
//!
//! Switch textures come in pairs (e.g., `SW1EXIT` / `SW2EXIT`).  When a player
//! activates a switch linedef, the wall texture is swapped from one variant to
//! the other.  [`SwitchList`] stores these pairs and provides bidirectional
//! lookup.
//!
//! # Usage
//! ```ignore
//! let mut anim = AnimState::new();
//! // Each game tic:
//! anim.tick();
//! // When rendering:
//! let resolved = anim.resolve_flat(&sector.floor_flat);
//! let texels = flat_cache.get(resolved);
//!
//! // Switch lookup:
//! let switches = SwitchList::new();
//! if let Some(opposite) = switches.get_opposite(b"SW1EXIT\0") {
//!     // opposite == b"SW2EXIT\0"
//! }
//! ```

// ---------------------------------------------------------------------------
// Name comparison helper
// ---------------------------------------------------------------------------

/// Compare two 8-byte lump names (case-insensitive, null-terminated).
///
/// Doom lump names are ASCII, uppercase, null-padded to 8 bytes.  This
/// function treats a null byte as an early terminator: `b"SKY1\0\0\0\0"`
/// equals `b"SKY1\0XXX"`.
pub fn names_equal(a: &[u8; 8], b: &[u8; 8]) -> bool {
    for i in 0..8 {
        let ca = a[i].to_ascii_uppercase();
        let cb = b[i].to_ascii_uppercase();
        if ca == 0 && cb == 0 {
            return true;
        }
        if ca != cb {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// AnimType
// ---------------------------------------------------------------------------

/// Distinguishes flat (floor/ceiling) animations from wall texture animations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimType {
    /// Floor/ceiling flat animation.
    Flat,
    /// Wall texture animation.
    Wall,
}

// ---------------------------------------------------------------------------
// AnimSequence
// ---------------------------------------------------------------------------

/// A single animation sequence (flat or wall texture).
///
/// Contains an ordered list of frame names.  The sequence cycles: after the
/// last frame it wraps back to the first.
#[derive(Clone, Debug)]
pub struct AnimSequence {
    /// Names of all frames in order (e.g., `[NUKAGE1, NUKAGE2, NUKAGE3]`).
    pub frames: Vec<[u8; 8]>,
    /// Current frame index (driven by global tic counter in `AnimState`).
    pub current_frame: usize,
    /// Game tics per frame (typically 8).
    pub tics_per_frame: u32,
    /// Whether this is a flat or wall animation.
    pub anim_type: AnimType,
}

impl AnimSequence {
    /// Create a new animation sequence from raw 8-byte names.
    pub fn new(names: &[[u8; 8]], tics_per_frame: u32) -> Self {
        Self {
            frames: names.to_vec(),
            current_frame: 0,
            tics_per_frame,
            anim_type: AnimType::Flat, // default; overridden by AnimState::new
        }
    }

    /// Create a new animation sequence with an explicit type.
    pub fn with_type(names: &[[u8; 8]], tics_per_frame: u32, anim_type: AnimType) -> Self {
        Self {
            frames: names.to_vec(),
            current_frame: 0,
            tics_per_frame,
            anim_type,
        }
    }

    /// Returns `true` if `name` is one of the frames in this sequence.
    pub fn contains(&self, name: &[u8; 8]) -> bool {
        self.frames.iter().any(|f| names_equal(f, name))
    }
}

// ---------------------------------------------------------------------------
// AnimState
// ---------------------------------------------------------------------------

/// Manages all texture/flat animation state.
///
/// Holds the hardcoded Doom animation sequences for both flats and wall
/// textures, plus a global tic counter that drives the current frame.
#[derive(Clone, Debug)]
pub struct AnimState {
    /// All active animation sequences (flats and walls combined).
    sequences: Vec<AnimSequence>,
    /// Flat animation sequences (references into `sequences` by index).
    flat_anims: Vec<AnimSequence>,
    /// Wall texture animation sequences (references into `sequences` by index).
    wall_anims: Vec<AnimSequence>,
    /// Current global tic counter (incremented once per game tic).
    tic_count: u32,
}

impl Default for AnimState {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimState {
    /// Build a new `AnimState` with all hardcoded Doom animation sequences.
    pub fn new() -> Self {
        // ----- Flat animations (Doom 1 & 2) -----
        let flat_anims = vec![
            // Green slime
            AnimSequence::with_type(
                &[*b"NUKAGE1\0", *b"NUKAGE2\0", *b"NUKAGE3\0"],
                8,
                AnimType::Flat,
            ),
            // Water
            AnimSequence::with_type(
                &[*b"FWATER1\0", *b"FWATER2\0", *b"FWATER3\0", *b"FWATER4\0"],
                8,
                AnimType::Flat,
            ),
            // Sewage water
            AnimSequence::with_type(
                &[*b"SWATER1\0", *b"SWATER2\0", *b"SWATER3\0", *b"SWATER4\0"],
                8,
                AnimType::Flat,
            ),
            // Lava
            AnimSequence::with_type(
                &[
                    *b"LAVA1\0\0\0",
                    *b"LAVA2\0\0\0",
                    *b"LAVA3\0\0\0",
                    *b"LAVA4\0\0\0",
                ],
                8,
                AnimType::Flat,
            ),
            // Blood
            AnimSequence::with_type(
                &[*b"BLOOD1\0\0", *b"BLOOD2\0\0", *b"BLOOD3\0\0"],
                8,
                AnimType::Flat,
            ),
            // Rock
            AnimSequence::with_type(
                &[*b"RROCK05\0", *b"RROCK06\0", *b"RROCK07\0", *b"RROCK08\0"],
                8,
                AnimType::Flat,
            ),
            // Slime variant 1
            AnimSequence::with_type(
                &[*b"SLIME01\0", *b"SLIME02\0", *b"SLIME03\0", *b"SLIME04\0"],
                8,
                AnimType::Flat,
            ),
            // Slime variant 2
            AnimSequence::with_type(
                &[*b"SLIME05\0", *b"SLIME06\0", *b"SLIME07\0", *b"SLIME08\0"],
                8,
                AnimType::Flat,
            ),
            // Slime variant 3
            AnimSequence::with_type(
                &[*b"SLIME09\0", *b"SLIME10\0", *b"SLIME11\0", *b"SLIME12\0"],
                8,
                AnimType::Flat,
            ),
        ];

        // ----- Wall texture animations -----
        let wall_anims = vec![
            // Bloody wall
            AnimSequence::with_type(
                &[*b"BLODGR1\0", *b"BLODGR2\0", *b"BLODGR3\0", *b"BLODGR4\0"],
                8,
                AnimType::Wall,
            ),
            // Blood drip
            AnimSequence::with_type(
                &[*b"BLODRIP1", *b"BLODRIP2", *b"BLODRIP3", *b"BLODRIP4"],
                8,
                AnimType::Wall,
            ),
            // Blue fire
            AnimSequence::with_type(&[*b"FIREBLU1", *b"FIREBLU2"], 8, AnimType::Wall),
            // Fire lava (note: irregular naming)
            AnimSequence::with_type(&[*b"FIRELAV3", *b"FIRELAVA"], 8, AnimType::Wall),
            // Fire magma
            AnimSequence::with_type(
                &[*b"FIREMAG1", *b"FIREMAG2", *b"FIREMAG3"],
                8,
                AnimType::Wall,
            ),
            // Fire wall
            AnimSequence::with_type(
                &[*b"FIREWALA", *b"FIREWALB", *b"FIREWALC"],
                8,
                AnimType::Wall,
            ),
            // Stone font
            AnimSequence::with_type(
                &[*b"GSTFONT1", *b"GSTFONT2", *b"GSTFONT3"],
                8,
                AnimType::Wall,
            ),
            // Red rock
            AnimSequence::with_type(
                &[*b"ROCKRED1", *b"ROCKRED2", *b"ROCKRED3"],
                8,
                AnimType::Wall,
            ),
            // Slime drip
            AnimSequence::with_type(
                &[*b"SLADRIP1", *b"SLADRIP2", *b"SLADRIP3"],
                8,
                AnimType::Wall,
            ),
            // Waterfall
            AnimSequence::with_type(
                &[
                    *b"WFALL1\0\0",
                    *b"WFALL2\0\0",
                    *b"WFALL3\0\0",
                    *b"WFALL4\0\0",
                ],
                8,
                AnimType::Wall,
            ),
            // Blood waterfall
            AnimSequence::with_type(
                &[
                    *b"BFALL1\0\0",
                    *b"BFALL2\0\0",
                    *b"BFALL3\0\0",
                    *b"BFALL4\0\0",
                ],
                8,
                AnimType::Wall,
            ),
            // Slime waterfall
            AnimSequence::with_type(
                &[
                    *b"SFALL1\0\0",
                    *b"SFALL2\0\0",
                    *b"SFALL3\0\0",
                    *b"SFALL4\0\0",
                ],
                8,
                AnimType::Wall,
            ),
            // Brain
            AnimSequence::with_type(
                &[*b"DBRAIN1\0", *b"DBRAIN2\0", *b"DBRAIN3\0", *b"DBRAIN4\0"],
                8,
                AnimType::Wall,
            ),
        ];

        // Build the unified sequence list.
        let mut sequences = Vec::with_capacity(flat_anims.len() + wall_anims.len());
        sequences.extend(flat_anims.iter().cloned());
        sequences.extend(wall_anims.iter().cloned());

        Self {
            sequences,
            flat_anims,
            wall_anims,
            tic_count: 0,
        }
    }

    /// Advance the global animation counter by one game tic.
    pub fn tick(&mut self) {
        self.tic_count = self.tic_count.wrapping_add(1);

        // Update current_frame on each sequence.
        for seq in &mut self.sequences {
            let frame_count = seq.frames.len() as u32;
            seq.current_frame = ((self.tic_count / seq.tics_per_frame) % frame_count) as usize;
        }
        for seq in &mut self.flat_anims {
            let frame_count = seq.frames.len() as u32;
            seq.current_frame = ((self.tic_count / seq.tics_per_frame) % frame_count) as usize;
        }
        for seq in &mut self.wall_anims {
            let frame_count = seq.frames.len() as u32;
            seq.current_frame = ((self.tic_count / seq.tics_per_frame) % frame_count) as usize;
        }
    }

    /// Return the current tic count (for testing/debugging).
    pub fn tic_count(&self) -> u32 {
        self.tic_count
    }

    /// Return all animation sequences (both flat and wall).
    pub fn sequences(&self) -> &[AnimSequence] {
        &self.sequences
    }

    /// Return only the flat animation sequences.
    pub fn flat_sequences(&self) -> &[AnimSequence] {
        &self.flat_anims
    }

    /// Return only the wall animation sequences.
    pub fn wall_sequences(&self) -> &[AnimSequence] {
        &self.wall_anims
    }

    /// Resolve a flat name to the current animation frame.
    ///
    /// If the name belongs to a flat animation sequence, returns the name of
    /// the frame that should be displayed at the current tic.  If the name is
    /// not part of any animation, it is returned unchanged.
    ///
    /// Returns `[u8; 8]` by value (it is `Copy`) to avoid lifetime complexity.
    pub fn resolve_flat(&self, name: &[u8; 8]) -> [u8; 8] {
        for seq in &self.flat_anims {
            if seq.contains(name) {
                let frame_count = seq.frames.len() as u32;
                let frame = (self.tic_count / seq.tics_per_frame) % frame_count;
                return seq.frames[frame as usize];
            }
        }
        *name
    }

    /// Resolve a wall texture name to the current animation frame.
    ///
    /// Same logic as [`resolve_flat`](Self::resolve_flat) but searches the
    /// wall texture animation sequences instead.
    pub fn resolve_wall(&self, name: &[u8; 8]) -> [u8; 8] {
        for seq in &self.wall_anims {
            if seq.contains(name) {
                let frame_count = seq.frames.len() as u32;
                let frame = (self.tic_count / seq.tics_per_frame) % frame_count;
                return seq.frames[frame as usize];
            }
        }
        *name
    }

    /// Given a flat name that belongs to an animation sequence, return the
    /// current frame name.  If not animated, return the input name unchanged.
    ///
    /// This is a convenience alias for [`resolve_flat`](Self::resolve_flat)
    /// that returns a reference to the static frame name stored inside the
    /// sequence (avoiding a copy), or the input if it is not animated.
    pub fn current_flat<'a>(&'a self, name: &'a [u8; 8]) -> &'a [u8; 8] {
        for seq in &self.flat_anims {
            if seq.contains(name) {
                let frame_count = seq.frames.len() as u32;
                let frame = (self.tic_count / seq.tics_per_frame) % frame_count;
                return &seq.frames[frame as usize];
            }
        }
        name
    }

    /// Given a wall texture name that belongs to an animation sequence, return
    /// the current frame name.  If not animated, return the input name
    /// unchanged.
    ///
    /// This is a convenience alias for [`resolve_wall`](Self::resolve_wall)
    /// that returns a reference.
    pub fn current_wall<'a>(&'a self, name: &'a [u8; 8]) -> &'a [u8; 8] {
        for seq in &self.wall_anims {
            if seq.contains(name) {
                let frame_count = seq.frames.len() as u32;
                let frame = (self.tic_count / seq.tics_per_frame) % frame_count;
                return &seq.frames[frame as usize];
            }
        }
        name
    }
}

// ---------------------------------------------------------------------------
// SwitchList
// ---------------------------------------------------------------------------

/// Bidirectional lookup table for Doom switch texture pairs.
///
/// Doom switches come in pairs: `SW1xxx` (off) and `SW2xxx` (on).  When the
/// player activates a switch linedef, the engine swaps the wall texture from
/// one to the other.  This struct stores the canonical list and provides
/// O(n) lookup (the list is small, ~30 entries — no hash needed).
#[derive(Clone, Debug)]
pub struct SwitchList {
    /// Each entry is `(off_name, on_name)` where off = `SW1xxx`, on = `SW2xxx`.
    pairs: Vec<([u8; 8], [u8; 8])>,
}

/// Helper: make a null-padded 8-byte name from a string slice.
fn make_name(s: &str) -> [u8; 8] {
    let mut buf = [0u8; 8];
    for (i, &b) in s.as_bytes().iter().take(8).enumerate() {
        buf[i] = b;
    }
    buf
}

impl Default for SwitchList {
    fn default() -> Self {
        Self::new()
    }
}

impl SwitchList {
    /// Build a `SwitchList` with all standard Doom switch texture pairs.
    pub fn new() -> Self {
        let pair_names: &[(&str, &str)] = &[
            ("SW1BRCOM", "SW2BRCOM"),
            ("SW1BRN1", "SW2BRN1"),
            ("SW1BRN2", "SW2BRN2"),
            ("SW1BRNGN", "SW2BRNGN"),
            ("SW1BROWN", "SW2BROWN"),
            ("SW1COMM", "SW2COMM"),
            ("SW1COMP", "SW2COMP"),
            ("SW1DIRT", "SW2DIRT"),
            ("SW1EXIT", "SW2EXIT"),
            ("SW1GRAY", "SW2GRAY"),
            ("SW1GRAY1", "SW2GRAY1"),
            ("SW1METAL", "SW2METAL"),
            ("SW1PIPE", "SW2PIPE"),
            ("SW1SLAD", "SW2SLAD"),
            ("SW1STARG", "SW2STARG"),
            ("SW1STON1", "SW2STON1"),
            ("SW1STON2", "SW2STON2"),
            ("SW1STONE", "SW2STONE"),
            ("SW1STRTN", "SW2STRTN"),
            ("SW1BLUE", "SW2BLUE"),
            ("SW1CMT", "SW2CMT"),
            ("SW1GARG", "SW2GARG"),
            ("SW1GSTON", "SW2GSTON"),
            ("SW1HOT", "SW2HOT"),
            ("SW1LION", "SW2LION"),
            ("SW1SATYR", "SW2SATYR"),
            ("SW1SKIN", "SW2SKIN"),
            ("SW1VINE", "SW2VINE"),
            ("SW1WOOD", "SW2WOOD"),
        ];

        let pairs = pair_names
            .iter()
            .map(|&(a, b)| (make_name(a), make_name(b)))
            .collect();

        Self { pairs }
    }

    /// Given a switch texture name, return its opposite.
    ///
    /// - `SW1EXIT` -> `Some(SW2EXIT)`
    /// - `SW2EXIT` -> `Some(SW1EXIT)`
    /// - Unknown   -> `None`
    pub fn get_opposite(&self, name: &[u8; 8]) -> Option<[u8; 8]> {
        for (off, on) in &self.pairs {
            if names_equal(off, name) {
                return Some(*on);
            }
            if names_equal(on, name) {
                return Some(*off);
            }
        }
        None
    }

    /// Return the number of switch pairs.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Returns `true` if there are no switch pairs.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- names_equal tests --------------------------------------------------

    #[test]
    fn names_equal_identical() {
        assert!(names_equal(b"NUKAGE1\0", b"NUKAGE1\0"));
    }

    #[test]
    fn names_equal_case_insensitive() {
        assert!(names_equal(b"NUKAGE1\0", b"nukage1\0"));
        assert!(names_equal(b"Nukage1\0", b"nUKAGE1\0"));
    }

    #[test]
    fn names_equal_different() {
        assert!(!names_equal(b"NUKAGE1\0", b"NUKAGE2\0"));
    }

    #[test]
    fn names_equal_null_terminated() {
        // Both end at the same effective string even if trailing bytes differ.
        assert!(names_equal(b"SKY1\0\0\0\0", b"SKY1\0XXX"));
    }

    #[test]
    fn names_equal_full_length_no_null() {
        // 8-char names with no null: must match all 8 bytes.
        assert!(names_equal(b"FIREBLU1", b"FIREBLU1"));
        assert!(!names_equal(b"FIREBLU1", b"FIREBLU2"));
    }

    // -- AnimSequence::contains tests ---------------------------------------

    #[test]
    fn anim_sequence_contains_member() {
        let seq = AnimSequence::new(&[*b"NUKAGE1\0", *b"NUKAGE2\0", *b"NUKAGE3\0"], 8);
        assert!(seq.contains(b"NUKAGE1\0"));
        assert!(seq.contains(b"NUKAGE2\0"));
        assert!(seq.contains(b"NUKAGE3\0"));
    }

    #[test]
    fn anim_sequence_does_not_contain_nonmember() {
        let seq = AnimSequence::new(&[*b"NUKAGE1\0", *b"NUKAGE2\0", *b"NUKAGE3\0"], 8);
        assert!(!seq.contains(b"FWATER1\0"));
    }

    // -- resolve_flat tests -------------------------------------------------

    #[test]
    fn resolve_flat_returns_animated_frame() {
        let mut anim = AnimState::new();

        // At tic 0, NUKAGE1 resolves to NUKAGE1 (frame 0).
        assert!(names_equal(&anim.resolve_flat(b"NUKAGE1\0"), b"NUKAGE1\0"));

        // After 8 tics, should resolve to NUKAGE2.
        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_flat(b"NUKAGE1\0"), b"NUKAGE2\0"));

        // After 16 tics total, should resolve to NUKAGE3.
        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_flat(b"NUKAGE1\0"), b"NUKAGE3\0"));

        // After 24 tics, wraps back to NUKAGE1.
        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_flat(b"NUKAGE1\0"), b"NUKAGE1\0"));
    }

    #[test]
    fn resolve_flat_non_animated_returns_same() {
        let anim = AnimState::new();
        let name = b"FLOOR4_8";
        assert!(names_equal(&anim.resolve_flat(name), name));
    }

    #[test]
    fn any_frame_in_sequence_resolves_to_current() {
        let mut anim = AnimState::new();

        // Both NUKAGE1 and NUKAGE2 are in the same sequence.
        // At tic 0, both should resolve to NUKAGE1 (frame 0).
        assert!(names_equal(&anim.resolve_flat(b"NUKAGE1\0"), b"NUKAGE1\0"));
        assert!(names_equal(&anim.resolve_flat(b"NUKAGE2\0"), b"NUKAGE1\0"));

        // After 8 tics, both resolve to NUKAGE2.
        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_flat(b"NUKAGE1\0"), b"NUKAGE2\0"));
        assert!(names_equal(&anim.resolve_flat(b"NUKAGE3\0"), b"NUKAGE2\0"));
    }

    // -- resolve_wall tests -------------------------------------------------

    #[test]
    fn resolve_wall_returns_animated_frame() {
        let mut anim = AnimState::new();

        // At tic 0, FIREBLU1 resolves to FIREBLU1 (frame 0).
        assert!(names_equal(&anim.resolve_wall(b"FIREBLU1"), b"FIREBLU1"));

        // After 8 tics, FIREBLU1 -> FIREBLU2.
        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_wall(b"FIREBLU1"), b"FIREBLU2"));

        // After 16 tics, wraps back to FIREBLU1 (2-frame sequence).
        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_wall(b"FIREBLU1"), b"FIREBLU1"));
    }

    #[test]
    fn resolve_wall_non_animated_returns_same() {
        let anim = AnimState::new();
        let name = b"BRICK1\0\0";
        assert!(names_equal(&anim.resolve_wall(name), name));
    }

    #[test]
    fn resolve_wall_four_frame_sequence() {
        let mut anim = AnimState::new();

        // BLODGR1..4 is a 4-frame wall animation.
        assert!(names_equal(&anim.resolve_wall(b"BLODGR1\0"), b"BLODGR1\0"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_wall(b"BLODGR1\0"), b"BLODGR2\0"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_wall(b"BLODGR1\0"), b"BLODGR3\0"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_wall(b"BLODGR1\0"), b"BLODGR4\0"));

        // Wraps after 32 tics.
        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(&anim.resolve_wall(b"BLODGR1\0"), b"BLODGR1\0"));
    }

    // -- tick / wrapping tests ----------------------------------------------

    #[test]
    fn tick_wraps_correctly() {
        let mut anim = AnimState::new();
        for _ in 0..1000 {
            anim.tick();
        }
        // Should not panic, tic_count wraps with wrapping_add.
        let _ = anim.resolve_flat(b"NUKAGE1\0");
        let _ = anim.resolve_wall(b"FIREBLU1");
    }

    #[test]
    fn tick_count_monotonic() {
        let mut anim = AnimState::new();
        assert_eq!(anim.tic_count(), 0);
        anim.tick();
        assert_eq!(anim.tic_count(), 1);
        for _ in 0..99 {
            anim.tick();
        }
        assert_eq!(anim.tic_count(), 100);
    }

    // -- edge case: water 4-frame flat cycle --------------------------------

    #[test]
    fn resolve_flat_water_four_frame_cycle() {
        let mut anim = AnimState::new();

        // FWATER1..4 is a 4-frame flat animation.
        for expected in [b"FWATER1\0", b"FWATER2\0", b"FWATER3\0", b"FWATER4\0"]
            .iter()
            .cycle()
            .take(8)
        {
            assert!(
                names_equal(&anim.resolve_flat(b"FWATER1\0"), expected),
                "at tic {}, expected {:?}",
                anim.tic_count(),
                core::str::from_utf8(*expected).unwrap_or("?"),
            );
            for _ in 0..8 {
                anim.tick();
            }
        }
    }

    // -- default trait impl -------------------------------------------------

    #[test]
    fn default_impl_works() {
        let anim = AnimState::default();
        assert_eq!(anim.tic_count(), 0);
        // Should have sequences loaded.
        assert!(names_equal(&anim.resolve_flat(b"NUKAGE1\0"), b"NUKAGE1\0"));
    }

    // ======================================================================
    // NEW TESTS — Animated textures + switch texture pairs
    // ======================================================================

    // -- 1. AnimState::new creates all standard sequences -------------------

    #[test]
    fn anim_state_new_creates_all_standard_flat_sequences() {
        let anim = AnimState::new();
        // 9 flat animation sequences defined in the standard table.
        assert_eq!(
            anim.flat_sequences().len(),
            9,
            "should have 9 flat animation sequences"
        );
    }

    #[test]
    fn anim_state_new_creates_all_standard_wall_sequences() {
        let anim = AnimState::new();
        // 13 wall animation sequences (BLODGR, BLODRIP, FIREBLU, FIRELAV,
        // FIREMAG, FIREWALA, GSTFONT, ROCKRED, SLADRIP, WFALL, BFALL, SFALL, DBRAIN).
        assert_eq!(
            anim.wall_sequences().len(),
            13,
            "should have 13 wall animation sequences"
        );
    }

    #[test]
    fn anim_state_new_creates_all_sequences_combined() {
        let anim = AnimState::new();
        assert_eq!(
            anim.sequences().len(),
            9 + 13,
            "total sequences should be 22 (9 flat + 13 wall)"
        );
    }

    // -- 2. tick advances tic counter (already tested above, adding one
    //       that verifies tick increments from arbitrary state) -------------

    #[test]
    fn tick_advances_tic_counter_from_arbitrary() {
        let mut anim = AnimState::new();
        for _ in 0..50 {
            anim.tick();
        }
        assert_eq!(anim.tic_count(), 50);
        anim.tick();
        assert_eq!(anim.tic_count(), 51);
    }

    // -- 3. Flat animation cycles (NUKAGE1->2->3->1) -----------------------
    //       (covered by resolve_flat_returns_animated_frame above, but let's
    //       test via current_flat)

    #[test]
    fn current_flat_nukage_cycle() {
        let mut anim = AnimState::new();
        assert!(names_equal(anim.current_flat(b"NUKAGE1\0"), b"NUKAGE1\0"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(anim.current_flat(b"NUKAGE1\0"), b"NUKAGE2\0"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(anim.current_flat(b"NUKAGE1\0"), b"NUKAGE3\0"));

        for _ in 0..8 {
            anim.tick();
        }
        // Wraps back to NUKAGE1.
        assert!(names_equal(anim.current_flat(b"NUKAGE1\0"), b"NUKAGE1\0"));
    }

    // -- 4. Wall animation cycles (FIREBLU1->2->1) -------------------------

    #[test]
    fn current_wall_fireblu_cycle() {
        let mut anim = AnimState::new();
        assert!(names_equal(anim.current_wall(b"FIREBLU1"), b"FIREBLU1"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(anim.current_wall(b"FIREBLU1"), b"FIREBLU2"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(anim.current_wall(b"FIREBLU1"), b"FIREBLU1"));
    }

    // -- 5. current_flat returns input unchanged for non-animated flat ------

    #[test]
    fn current_flat_non_animated_returns_input() {
        let anim = AnimState::new();
        let name = b"FLOOR4_8";
        let result = anim.current_flat(name);
        assert!(
            std::ptr::eq(result, name),
            "non-animated flat should return the same reference"
        );
    }

    // -- 6. current_wall returns input unchanged for non-animated wall ------

    #[test]
    fn current_wall_non_animated_returns_input() {
        let anim = AnimState::new();
        let name = b"BRICK1\0\0";
        let result = anim.current_wall(name);
        assert!(
            std::ptr::eq(result, name),
            "non-animated wall should return the same reference"
        );
    }

    // -- 7. Animation timing: 8 tics per frame (granularity test) ----------

    #[test]
    fn animation_timing_eight_tics_per_frame() {
        let mut anim = AnimState::new();

        // At tic 0..7, frame should be 0 (NUKAGE1).
        for tic in 0..8 {
            assert!(
                names_equal(anim.current_flat(b"NUKAGE1\0"), b"NUKAGE1\0"),
                "at tic {tic}, should still be NUKAGE1"
            );
            anim.tick();
        }
        // At tic 8, frame should now be 1 (NUKAGE2).
        assert!(names_equal(anim.current_flat(b"NUKAGE1\0"), b"NUKAGE2\0"));
    }

    // -- 8. Multiple animations tick independently -------------------------

    #[test]
    fn multiple_animations_tick_independently() {
        let mut anim = AnimState::new();

        // Advance 8 tics. Both flat and wall animations advance independently.
        for _ in 0..8 {
            anim.tick();
        }

        // NUKAGE is 3-frame: tic 8 -> frame 1 -> NUKAGE2
        assert!(names_equal(anim.current_flat(b"NUKAGE1\0"), b"NUKAGE2\0"));
        // FWATER is 4-frame: tic 8 -> frame 1 -> FWATER2
        assert!(names_equal(anim.current_flat(b"FWATER1\0"), b"FWATER2\0"));
        // FIREBLU is 2-frame: tic 8 -> frame 1 -> FIREBLU2
        assert!(names_equal(anim.current_wall(b"FIREBLU1"), b"FIREBLU2"));
        // BLODGR is 4-frame: tic 8 -> frame 1 -> BLODGR2
        assert!(names_equal(anim.current_wall(b"BLODGR1\0"), b"BLODGR2\0"));
    }

    // -- 9. SwitchList::new creates all pairs ------------------------------

    #[test]
    fn switch_list_new_creates_all_pairs() {
        let switches = SwitchList::new();
        assert_eq!(
            switches.len(),
            29,
            "should have 29 standard Doom switch pairs"
        );
    }

    // -- 10. get_opposite SW1EXIT returns SW2EXIT --------------------------

    #[test]
    fn switch_get_opposite_sw1exit_returns_sw2exit() {
        let switches = SwitchList::new();
        let result = switches.get_opposite(b"SW1EXIT\0").unwrap();
        assert!(
            names_equal(&result, b"SW2EXIT\0"),
            "SW1EXIT should map to SW2EXIT, got {:?}",
            core::str::from_utf8(&result)
        );
    }

    // -- 11. get_opposite SW2EXIT returns SW1EXIT --------------------------

    #[test]
    fn switch_get_opposite_sw2exit_returns_sw1exit() {
        let switches = SwitchList::new();
        let result = switches.get_opposite(b"SW2EXIT\0").unwrap();
        assert!(
            names_equal(&result, b"SW1EXIT\0"),
            "SW2EXIT should map to SW1EXIT, got {:?}",
            core::str::from_utf8(&result)
        );
    }

    // -- 12. get_opposite unknown returns None -----------------------------

    #[test]
    fn switch_get_opposite_unknown_returns_none() {
        let switches = SwitchList::new();
        assert!(
            switches.get_opposite(b"BRICK1\0\0").is_none(),
            "unknown texture should return None"
        );
    }

    // -- 13. AnimState tick wraps around (frame count cycles) --------------

    #[test]
    fn anim_state_tick_wraps_frame_count() {
        let mut anim = AnimState::new();
        // NUKAGE is 3-frame, 8 tics/frame. Full cycle = 24 tics.
        // After exactly 24 tics, should be back to frame 0.
        for _ in 0..24 {
            anim.tick();
        }
        assert!(names_equal(anim.current_flat(b"NUKAGE1\0"), b"NUKAGE1\0"));

        // After 48 tics (two full cycles), still frame 0.
        for _ in 0..24 {
            anim.tick();
        }
        assert!(names_equal(anim.current_flat(b"NUKAGE1\0"), b"NUKAGE1\0"));
    }

    // -- 14. 4-frame animation cycles correctly (FWATER1..4) ---------------

    #[test]
    fn four_frame_flat_animation_cycles_correctly() {
        let mut anim = AnimState::new();

        let expected_cycle = [b"FWATER1\0", b"FWATER2\0", b"FWATER3\0", b"FWATER4\0"];
        // Run through 3 complete cycles (12 frames = 96 tics).
        for cycle in 0..3 {
            for (frame_idx, &expected) in expected_cycle.iter().enumerate() {
                assert!(
                    names_equal(anim.current_flat(b"FWATER1\0"), expected),
                    "cycle {cycle} frame {frame_idx}: expected {:?}",
                    core::str::from_utf8(expected).unwrap_or("?"),
                );
                for _ in 0..8 {
                    anim.tick();
                }
            }
        }
    }

    // -- 15. All standard flat animations have valid frame counts ----------

    #[test]
    fn all_standard_flat_animations_have_valid_frame_counts() {
        let anim = AnimState::new();
        for seq in anim.flat_sequences() {
            assert!(
                !seq.frames.is_empty(),
                "flat animation must have at least one frame"
            );
            assert!(
                seq.frames.len() <= 4,
                "flat animation has {} frames (max expected 4)",
                seq.frames.len()
            );
            assert_eq!(
                seq.tics_per_frame, 8,
                "flat animation tics_per_frame should be 8"
            );
            assert_eq!(
                seq.anim_type,
                AnimType::Flat,
                "flat animation should have AnimType::Flat"
            );
        }
    }

    // -- 16. All standard wall animations have valid frame counts ----------

    #[test]
    fn all_standard_wall_animations_have_valid_frame_counts() {
        let anim = AnimState::new();
        for seq in anim.wall_sequences() {
            assert!(
                !seq.frames.is_empty(),
                "wall animation must have at least one frame"
            );
            assert!(
                seq.frames.len() <= 4,
                "wall animation has {} frames (max expected 4)",
                seq.frames.len()
            );
            assert_eq!(
                seq.tics_per_frame, 8,
                "wall animation tics_per_frame should be 8"
            );
            assert_eq!(
                seq.anim_type,
                AnimType::Wall,
                "wall animation should have AnimType::Wall"
            );
        }
    }

    // -- 17. AnimType derives correctly ------------------------------------

    #[test]
    fn anim_type_clone_debug_eq() {
        let flat = AnimType::Flat;
        let wall = AnimType::Wall;
        assert_eq!(flat, flat.clone());
        assert_eq!(wall, wall.clone());
        assert_ne!(flat, wall);
        // Debug impl should not panic.
        let _ = format!("{flat:?}");
        let _ = format!("{wall:?}");
    }

    // -- 18. SwitchList bidirectional for all entries ----------------------

    #[test]
    fn switch_list_bidirectional_for_all_pairs() {
        let switches = SwitchList::new();
        // For every known pair, check that get_opposite works both ways.
        let known: &[(&[u8; 8], &[u8; 8])] = &[
            (b"SW1BRCOM", b"SW2BRCOM"),
            (b"SW1BRN1\0", b"SW2BRN1\0"),
            (b"SW1BROWN", b"SW2BROWN"),
            (b"SW1BLUE\0", b"SW2BLUE\0"),
            (b"SW1WOOD\0", b"SW2WOOD\0"),
        ];
        for &(off, on) in known {
            let got_on = switches.get_opposite(off).expect("SW1 should map to SW2");
            assert!(names_equal(&got_on, on), "SW1 -> SW2 mismatch");
            let got_off = switches.get_opposite(on).expect("SW2 should map to SW1");
            assert!(names_equal(&got_off, off), "SW2 -> SW1 mismatch");
        }
    }

    // -- 19. SwitchList default impl works ---------------------------------

    #[test]
    fn switch_list_default_impl() {
        let switches = SwitchList::default();
        assert_eq!(switches.len(), 29);
        assert!(!switches.is_empty());
    }

    // -- 20. WFALL wall animation present and cycles ----------------------

    #[test]
    fn wfall_wall_animation_cycles() {
        let mut anim = AnimState::new();

        // Verify WFALL is in the wall animations.
        assert!(
            anim.wall_sequences()
                .iter()
                .any(|seq| seq.contains(b"WFALL1\0\0")),
            "WFALL1 should be a known wall animation"
        );

        // Cycle through all 4 frames.
        assert!(names_equal(anim.current_wall(b"WFALL1\0\0"), b"WFALL1\0\0"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(anim.current_wall(b"WFALL1\0\0"), b"WFALL2\0\0"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(anim.current_wall(b"WFALL1\0\0"), b"WFALL3\0\0"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(anim.current_wall(b"WFALL1\0\0"), b"WFALL4\0\0"));

        for _ in 0..8 {
            anim.tick();
        }
        assert!(names_equal(anim.current_wall(b"WFALL1\0\0"), b"WFALL1\0\0"));
    }

    // -- 21. AnimSequence with_type sets anim_type correctly ---------------

    #[test]
    fn anim_sequence_with_type_sets_type() {
        let flat_seq =
            AnimSequence::with_type(&[*b"TEST1\0\0\0", *b"TEST2\0\0\0"], 8, AnimType::Flat);
        assert_eq!(flat_seq.anim_type, AnimType::Flat);
        assert_eq!(flat_seq.current_frame, 0);

        let wall_seq =
            AnimSequence::with_type(&[*b"WALL1\0\0\0", *b"WALL2\0\0\0"], 8, AnimType::Wall);
        assert_eq!(wall_seq.anim_type, AnimType::Wall);
    }

    // -- 22. SwitchList case insensitive lookup ----------------------------

    #[test]
    fn switch_list_case_insensitive_lookup() {
        let switches = SwitchList::new();
        // names_equal is case-insensitive, so lowercase should work.
        let result = switches.get_opposite(b"sw1exit\0");
        assert!(result.is_some(), "case-insensitive lookup should work");
        assert!(names_equal(&result.unwrap(), b"SW2EXIT\0"));
    }
}
