// ---------------------------------------------------------------------------
// DoomRng
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
