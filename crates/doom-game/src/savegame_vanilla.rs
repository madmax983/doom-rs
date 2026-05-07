//! Vanilla Doom `.dsg` savegame header detection and parsing.
//!
//! This module deliberately stops at the format boundary. It can recognize and
//! parse the fixed vanilla header, but full payload read/write still returns an
//! explicit unsupported error until the engine has a faithful serializer.

use crate::savegame::{SaveError, SaveGame};
use crate::state::GameState;

/// Bytes in the fixed vanilla savegame header before the serialized payload.
pub const VANILLA_HEADER_LEN: usize = 24 + 16 + 1 + 1 + 1 + 4 + 3;

const DESCRIPTION_LEN: usize = 24;
const VERSION_LEN: usize = 16;
const VERSION_PREFIX: &str = "version ";
const SUPPORTED_VERSION: &str = "version 109";

/// Parsed vanilla savegame header fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VanillaSaveHeader {
    /// User-facing save description, null-padded to 24 bytes.
    pub description: [u8; DESCRIPTION_LEN],
    /// Version string, typically `version 109`, null-padded to 16 bytes.
    pub version: [u8; VERSION_LEN],
    /// Doom skill level encoded in the save header.
    pub skill: u8,
    /// Episode number for shareware/Ultimate Doom style maps.
    pub episode: u8,
    /// Map number within the episode.
    pub map: u8,
    /// Per-player active flags from the fixed vanilla header.
    pub players_in_game: [u8; 4],
    /// Elapsed level time stored as a 24-bit little-endian counter.
    pub level_time: u32,
}

/// Return true when `data` looks like a vanilla/chocolate Doom savegame header.
#[must_use]
pub fn looks_like_vanilla_dsg(data: &[u8]) -> bool {
    if data.len() < DESCRIPTION_LEN + VERSION_LEN {
        return false;
    }

    version_string(&data[DESCRIPTION_LEN..DESCRIPTION_LEN + VERSION_LEN])
        .is_some_and(|version| version.starts_with(VERSION_PREFIX))
}

/// Parse the fixed vanilla savegame header.
pub fn parse_header(data: &[u8]) -> Result<VanillaSaveHeader, SaveError> {
    if data.len() < VANILLA_HEADER_LEN {
        return Err(SaveError::TooShort);
    }
    if !looks_like_vanilla_dsg(data) {
        return Err(SaveError::BadMagic);
    }

    let mut description = [0u8; DESCRIPTION_LEN];
    description.copy_from_slice(&data[..DESCRIPTION_LEN]);

    let mut version = [0u8; VERSION_LEN];
    version.copy_from_slice(&data[DESCRIPTION_LEN..DESCRIPTION_LEN + VERSION_LEN]);
    if version_string(&version) != Some(SUPPORTED_VERSION) {
        return Err(SaveError::BadVersion);
    }

    let offset = DESCRIPTION_LEN + VERSION_LEN;
    let skill = data[offset];
    let episode = data[offset + 1];
    let map = data[offset + 2];

    let mut players_in_game = [0u8; 4];
    players_in_game.copy_from_slice(&data[offset + 3..offset + 7]);

    let level_time = u32::from(data[offset + 7])
        | (u32::from(data[offset + 8]) << 8)
        | (u32::from(data[offset + 9]) << 16);

    Ok(VanillaSaveHeader {
        description,
        version,
        skill,
        episode,
        map,
        players_in_game,
        level_time,
    })
}

/// Recognize a vanilla savegame header, then fail explicitly until payload
/// parity work lands.
pub fn load_game(data: &[u8]) -> Result<SaveGame, SaveError> {
    let _header = parse_header(data)?;
    Err(SaveError::UnsupportedVanillaDsg)
}

/// Writing a faithful vanilla payload is not implemented in this pass.
pub fn save_game(
    _gs: &GameState,
    _level_name: &[u8; 8],
    _skill: u8,
    _description: &str,
) -> Result<Vec<u8>, SaveError> {
    Err(SaveError::UnsupportedVanillaDsg)
}

fn version_string(bytes: &[u8]) -> Option<&str> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..end]).ok()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string_handles_missing_null_terminator() {
        // Provide bytes that don't have a null byte, forcing `unwrap_or` to use `bytes.len()`.
        let bytes = b"NO_NULL_HERE";
        let result = version_string(bytes);
        assert_eq!(result, Some("NO_NULL_HERE"));
    }

    #[test]
    fn version_string_handles_null_terminator() {
        let bytes = b"HAVE_NULL\0IGNORE";
        let result = version_string(bytes);
        assert_eq!(result, Some("HAVE_NULL"));
    }
}
