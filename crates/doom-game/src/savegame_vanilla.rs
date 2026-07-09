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
    fn parse_header_too_short() {
        let short_data = vec![0u8; VANILLA_HEADER_LEN - 1];
        let result = parse_header(&short_data);
        assert_eq!(result.expect_err("Must be an error"), SaveError::TooShort);
    }

    #[test]
    fn parse_header_bad_magic() {
        let bad_data = vec![0u8; VANILLA_HEADER_LEN];
        // Not "version 109"
        let result = parse_header(&bad_data);
        assert_eq!(result.expect_err("Must be an error"), SaveError::BadMagic);
    }

    #[test]
    fn parse_header_bad_version() {
        let mut bad_data = vec![0u8; VANILLA_HEADER_LEN];
        // Correct magic prefix "version " but wrong version
        bad_data[DESCRIPTION_LEN..DESCRIPTION_LEN + 8].copy_from_slice(b"version ");
        bad_data[DESCRIPTION_LEN + 8..DESCRIPTION_LEN + 11].copy_from_slice(b"108");

        let result = parse_header(&bad_data);
        assert_eq!(result.expect_err("Must be an error"), SaveError::BadVersion);
    }

    #[test]
    fn parse_header_success() {
        let mut good_data = vec![0u8; VANILLA_HEADER_LEN];

        // Fill description
        good_data[0..9].copy_from_slice(b"save1.dsg");

        // Fill version
        good_data[DESCRIPTION_LEN..DESCRIPTION_LEN + 11].copy_from_slice(b"version 109");

        // Fill skill, episode, map
        let offset = DESCRIPTION_LEN + VERSION_LEN;
        good_data[offset] = 2; // Skill 3 (0-indexed)
        good_data[offset + 1] = 1; // Episode 1
        good_data[offset + 2] = 1; // Map 1

        // Players in game
        good_data[offset + 3] = 1; // Player 1 active

        // Level time (little-endian 24-bit)
        good_data[offset + 7] = 0xAA;
        good_data[offset + 8] = 0xBB;
        good_data[offset + 9] = 0xCC;

        let result = parse_header(&good_data).expect("Should parse successfully");

        assert_eq!(&result.description[0..9], b"save1.dsg");
        assert_eq!(&result.version[0..11], b"version 109");
        assert_eq!(result.skill, 2);
        assert_eq!(result.episode, 1);
        assert_eq!(result.map, 1);
        assert_eq!(result.players_in_game, [1, 0, 0, 0]);
        assert_eq!(result.level_time, 0xCCBBAA);
    }

    #[test]
    fn test_version_string_valid() {
        let mut bytes = [0u8; 16];
        bytes[0..11].copy_from_slice(b"version 109");
        assert_eq!(version_string(&bytes), Some("version 109"));
    }

    #[test]
    fn test_version_string_empty() {
        let bytes: [u8; 0] = [];
        assert_eq!(version_string(&bytes), Some(""));
    }

    #[test]
    fn test_version_string_no_null_terminator() {
        let bytes = b"version 109_more";
        assert_eq!(version_string(bytes), Some("version 109_more"));
    }

    #[test]
    fn test_version_string_invalid_utf8() {
        let bytes = [0xFF, 0xFE, 0x00];
        assert_eq!(version_string(&bytes), None);
    }

    #[test]
    fn load_game_unsupported() {
        let mut good_data = vec![0u8; VANILLA_HEADER_LEN];
        good_data[DESCRIPTION_LEN..DESCRIPTION_LEN + 11].copy_from_slice(b"version 109");
        let result = load_game(&good_data);
        assert_eq!(
            result.expect_err("Must be an error"),
            SaveError::UnsupportedVanillaDsg
        );
    }

    #[test]
    fn save_game_unsupported() {
        let gs = GameState::new("E1M1    ");
        let result = save_game(&gs, b"E1M1    ", 2, "Test");
        assert_eq!(
            result.expect_err("Must be an error"),
            SaveError::UnsupportedVanillaDsg
        );
    }
}
