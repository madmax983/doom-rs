//! Animated texture and flat management.
//!
//! Doom has hardcoded animation sequences for both floor/ceiling flats and wall
//! textures.  Each sequence is defined by a list of frame names that cycle at a
//! rate of 8 game tics per frame (~4.4 Hz at 35 tics/sec).
//!
//! Animation is **global** -- all instances of a texture in the same sequence
//! show the same frame at any given tic.  The animation state is advanced once
//! per game tic via [`AnimState::tick`].
//!
//! # Usage
//! ```ignore
//! let mut anim = AnimState::new();
//! // Each game tic:
//! anim.tick();
//! // When rendering:
//! let resolved = anim.resolve_flat(&sector.floor_flat);
//! let texels = flat_cache.get(resolved);
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
    /// Game tics per frame (typically 8).
    pub tics_per_frame: u32,
}

impl AnimSequence {
    /// Create a new animation sequence from raw 8-byte names.
    pub fn new(names: &[[u8; 8]], tics_per_frame: u32) -> Self {
        Self {
            frames: names.to_vec(),
            tics_per_frame,
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
    /// Flat animation sequences.
    flat_anims: Vec<AnimSequence>,
    /// Wall texture animation sequences.
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
            AnimSequence::new(&[*b"NUKAGE1\0", *b"NUKAGE2\0", *b"NUKAGE3\0"], 8),
            // Water
            AnimSequence::new(
                &[*b"FWATER1\0", *b"FWATER2\0", *b"FWATER3\0", *b"FWATER4\0"],
                8,
            ),
            // Sewage water
            AnimSequence::new(
                &[*b"SWATER1\0", *b"SWATER2\0", *b"SWATER3\0", *b"SWATER4\0"],
                8,
            ),
            // Lava
            AnimSequence::new(
                &[
                    *b"LAVA1\0\0\0",
                    *b"LAVA2\0\0\0",
                    *b"LAVA3\0\0\0",
                    *b"LAVA4\0\0\0",
                ],
                8,
            ),
            // Blood
            AnimSequence::new(&[*b"BLOOD1\0\0", *b"BLOOD2\0\0", *b"BLOOD3\0\0"], 8),
            // Rock
            AnimSequence::new(
                &[*b"RROCK05\0", *b"RROCK06\0", *b"RROCK07\0", *b"RROCK08\0"],
                8,
            ),
            // Slime variant 1
            AnimSequence::new(
                &[*b"SLIME01\0", *b"SLIME02\0", *b"SLIME03\0", *b"SLIME04\0"],
                8,
            ),
            // Slime variant 2
            AnimSequence::new(
                &[*b"SLIME05\0", *b"SLIME06\0", *b"SLIME07\0", *b"SLIME08\0"],
                8,
            ),
            // Slime variant 3
            AnimSequence::new(
                &[*b"SLIME09\0", *b"SLIME10\0", *b"SLIME11\0", *b"SLIME12\0"],
                8,
            ),
        ];

        // ----- Wall texture animations -----
        let wall_anims = vec![
            // Bloody wall
            AnimSequence::new(
                &[*b"BLODGR1\0", *b"BLODGR2\0", *b"BLODGR3\0", *b"BLODGR4\0"],
                8,
            ),
            // Blood drip
            AnimSequence::new(&[*b"BLODRIP1", *b"BLODRIP2", *b"BLODRIP3", *b"BLODRIP4"], 8),
            // Blue fire
            AnimSequence::new(&[*b"FIREBLU1", *b"FIREBLU2"], 8),
            // Fire lava (note: irregular naming)
            AnimSequence::new(&[*b"FIRELAV3", *b"FIRELAVA"], 8),
            // Fire magma
            AnimSequence::new(&[*b"FIREMAG1", *b"FIREMAG2", *b"FIREMAG3"], 8),
            // Fire wall
            AnimSequence::new(&[*b"FIREWALA", *b"FIREWALB", *b"FIREWALC"], 8),
            // Stone font
            AnimSequence::new(&[*b"GSTFONT1", *b"GSTFONT2", *b"GSTFONT3"], 8),
            // Red rock
            AnimSequence::new(&[*b"ROCKRED1", *b"ROCKRED2", *b"ROCKRED3"], 8),
            // Slime drip
            AnimSequence::new(&[*b"SLADRIP1", *b"SLADRIP2", *b"SLADRIP3"], 8),
            // Blood waterfall
            AnimSequence::new(
                &[
                    *b"BFALL1\0\0",
                    *b"BFALL2\0\0",
                    *b"BFALL3\0\0",
                    *b"BFALL4\0\0",
                ],
                8,
            ),
            // Slime waterfall
            AnimSequence::new(
                &[
                    *b"SFALL1\0\0",
                    *b"SFALL2\0\0",
                    *b"SFALL3\0\0",
                    *b"SFALL4\0\0",
                ],
                8,
            ),
            // Brain
            AnimSequence::new(
                &[*b"DBRAIN1\0", *b"DBRAIN2\0", *b"DBRAIN3\0", *b"DBRAIN4\0"],
                8,
            ),
        ];

        Self {
            flat_anims,
            wall_anims,
            tic_count: 0,
        }
    }

    /// Advance the global animation counter by one game tic.
    pub fn tick(&mut self) {
        self.tic_count = self.tic_count.wrapping_add(1);
    }

    /// Return the current tic count (for testing/debugging).
    pub fn tic_count(&self) -> u32 {
        self.tic_count
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
                names_equal(&anim.resolve_flat(b"FWATER1\0"), *expected),
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
}
