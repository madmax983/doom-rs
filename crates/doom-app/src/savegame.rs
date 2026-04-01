//! Save/load game state to/from disk.
//!
//! Uses the single source of truth binary format from `doom_game::savegame`
//! to fully serialize and deserialize the entire deterministic `GameState`.
//! This removes the need for `bincode` or duplicate state extraction in the app layer.

use doom_game::{
    GameState,
    savegame::{SaveGame, load_game as engine_load, save_game as engine_save},
};
use std::path::Path;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during save/load operations.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// Underlying I/O error (file not found, permission denied, etc.).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to encode game state to bytes.
    ///
    /// This error could be raised when the internal logic fails to serialize
    /// the game state properly into the deterministic binary format.
    #[allow(dead_code)]
    #[error("failed to encode save game")]
    Encode,

    /// Failed to decode bytes into game state.
    ///
    /// The binary format might be malformed or it failed to reconstruct
    /// valid engine structures from the byte stream.
    #[allow(dead_code)]
    #[error("failed to decode save game")]
    Decode,

    /// File does not start with the `b"DRS1"` magic bytes.
    #[error("invalid save file magic")]
    BadMagic,

    /// Save file has an unsupported version number.
    #[error("unsupported save version")]
    BadVersion,

    /// The save payload was truncated.
    #[error("save payload truncated")]
    Truncated,
}

impl From<doom_game::savegame::SaveError> for SaveError {
    fn from(err: doom_game::savegame::SaveError) -> Self {
        match err {
            doom_game::savegame::SaveError::TooShort => SaveError::Truncated,
            doom_game::savegame::SaveError::BadMagic => SaveError::BadMagic,
            doom_game::savegame::SaveError::BadVersion => SaveError::BadVersion,
            doom_game::savegame::SaveError::Truncated => SaveError::Truncated,
        }
    }
}

// ---------------------------------------------------------------------------
// save_game
// ---------------------------------------------------------------------------

/// Save the current game to `path` using slot number `slot`.
///
/// Overwrites any existing file at `path`.
///
/// # Errors
/// Returns [`SaveError::Io`] on file write failure.
pub fn save_game(path: &Path, gs: &GameState, slot: u8) -> Result<(), SaveError> {
    // Generate an 8-byte padded level name.
    let mut level_name = [0u8; 8];
    let src_bytes = gs.level_name.as_bytes();
    let copy_len = src_bytes.len().min(8);
    level_name[..copy_len].copy_from_slice(&src_bytes[..copy_len]);

    let description = format!("Slot {slot}");

    // We assume skill 2 (Medium) for now since `GameState` does not currently store the skill level.
    // However, the `doom-app` sets skill at level spawn.
    // Let's pass 2 (HMP) to satisfy the engine signature.
    let data = engine_save(gs, &level_name, 2, &description);
    std::fs::write(path, &data)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// load_game
// ---------------------------------------------------------------------------

/// Load a save file from `path`, returning the header and payload.
///
/// Validates magic bytes and version before returning.
///
/// # Errors
/// Returns [`SaveError::Io`] if the file cannot be read.
/// Returns [`SaveError::Decode`] if the binary format is malformed.
/// Returns [`SaveError::BadMagic`] if the file lacks the `b"DRS1"` signature.
/// Returns [`SaveError::BadVersion`] for an unsupported version number.
pub fn load_game(path: &Path) -> Result<(doom_game::savegame::SaveHeader, SaveGame), SaveError> {
    let data = std::fs::read(path)?;
    let save_game = engine_load(&data)?;
    // We clone the header so we can return both. Note `SaveGame` already contains the header.
    let header = save_game.header.clone();
    Ok((header, save_game))
}

// ---------------------------------------------------------------------------
// apply_save
// ---------------------------------------------------------------------------

/// Apply a loaded [`SaveGame`] to `gs`, restoring the full simulation state.
///
/// Unlike the old bincode system which only restored player inventory, this
/// engine-native system restores the complete `GameState` including all Mobj
/// handles, sector movers, and RNG state to guarantee exact deterministic replay.
///
/// # Errors
/// Currently always returns `Ok(())`.
pub fn apply_save(gs: &mut GameState, payload: &SaveGame) -> Result<(), SaveError> {
    // Completely overwrite the current game state with the deserialized one.
    // This is valid because `GameState` implements `Clone` and owns all its data.
    *gs = payload.state.clone();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_game::{Mobj, MobjKind, PlayerState};
    use doom_types::{Bam, Fixed16_16};

    /// Create a minimal GameState with a spawned player Mobj.
    fn make_gs() -> GameState {
        let mut gs = GameState::new("E1M1");
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(100),
            Fixed16_16::from_int(200),
            Bam(0x4000_0000), // 90°
        );
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs.tic_num = 42;
        // Advance rng a few steps so index is non-zero.
        for _ in 0..17 {
            gs.rng.next_byte();
        }
        gs
    }

    /// Save to a temp file and return the path.
    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    // -----------------------------------------------------------------------
    // Test 1: header roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn save_load_roundtrip_header() {
        let gs = make_gs();
        let path = temp_path("doom_rs_test_header.bin");

        save_game(&path, &gs, 3).expect("save_game must succeed");

        let (header, _payload) = load_game(&path).expect("load_game must succeed");

        assert_eq!(
            header.magic,
            doom_game::savegame::SAVE_MAGIC,
            "magic must be b\"DRS1\""
        );
        // Description starts with 'Slot 3'.
        let desc = core::str::from_utf8(&header.description)
            .unwrap()
            .trim_matches('\0');
        assert_eq!(desc, "Slot 3", "slot must round-trip via description");

        // Clean up.
        let _ = std::fs::remove_file(&path);
    }

    // -----------------------------------------------------------------------
    // Test 2: payload roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn save_load_roundtrip_payload() {
        let gs = make_gs();
        let path = temp_path("doom_rs_test_payload.bin");

        save_game(&path, &gs, 0).expect("save_game must succeed");

        let (_header, payload) = load_game(&path).expect("load_game must succeed");

        assert_eq!(
            payload.state.tic_num, gs.tic_num,
            "payload tic_num must match gs"
        );
        assert_eq!(
            payload.state.rng.index(),
            gs.rng.index(),
            "rng_index must round-trip"
        );
        assert_eq!(
            payload.state.player.health(),
            gs.player.health(),
            "health must round-trip"
        );

        // Clean up.
        let _ = std::fs::remove_file(&path);
    }

    // -----------------------------------------------------------------------
    // Test 3: empty file returns Err (not panic)
    // -----------------------------------------------------------------------

    #[test]
    fn load_empty_file_errors() {
        let path = temp_path("doom_rs_test_empty.bin");
        std::fs::write(&path, b"").unwrap();

        let result = load_game(&path);
        assert!(result.is_err(), "loading an empty file must return Err");

        let _ = std::fs::remove_file(&path);
    }
}
