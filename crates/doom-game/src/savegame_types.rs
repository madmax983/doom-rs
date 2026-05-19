// ---------------------------------------------------------------------------
// Savegame Types
// ---------------------------------------------------------------------------

use crate::state::GameState;

/// Supported binary savegame formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    /// The project-native `DRS1` save format.
    DoomRs,
    /// A vanilla Doom `.dsg` save header/payload boundary.
    VanillaDsg,
}

/// Header for a save file — identifies format, level, and slot description.
#[derive(Debug, Clone)]
pub struct SaveHeader {
    /// Magic bytes (`SAVE_MAGIC`).
    pub magic: [u8; 4],
    /// Format version (currently 2).
    pub version: u32,
    /// Level name, null-padded to 8 bytes (e.g. `b"E1M1\0\0\0\0"`).
    pub level_name: [u8; 8],
    /// Skill level (0=Baby .. 4=Nightmare).
    pub skill: u8,
    /// Tics elapsed in the current level at save time.
    pub level_time: u32,
    /// Player-facing slot description, null-padded to 24 bytes.
    pub description: [u8; 24],
}

/// The result of a successful `load_game` call.
#[derive(Debug, Clone)]
pub struct SaveGame {
    /// The header read from the save data.
    pub header: SaveHeader,
    /// The restored game state.
    pub state: GameState,
}

/// Errors that can occur during `load_game`.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// Input data is too short to contain even a header.
    #[error("save data too short for header")]
    TooShort,
    /// Save data does not match any recognized header.
    #[error("unrecognized save file header")]
    BadMagic,
    /// Format version is not supported.
    #[error("unsupported save format version")]
    BadVersion,
    /// Data ended before all fields could be read.
    #[error("save data truncated")]
    Truncated,
    /// Vanilla DSG payload support is not implemented yet.
    #[error("vanilla DSG payload support is not implemented yet")]
    UnsupportedVanillaDsg,
}
