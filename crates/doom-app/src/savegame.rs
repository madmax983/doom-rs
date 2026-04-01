//! The Savegame Chronicles: Preserving the Demonic Timeline.
//!
//! # The Abstract
//! In a realm governed by strict deterministic rules, restarting a level after
//! every tragic demise isn't merely punishing—it's demoralizing. We needed a way
//! to freeze time, capture every bleeding demon and flying fireball, and etch it
//! into the bedrock of the disk.
//!
//! Enter the `savegame` module. This isn't your grandpappy's `bincode` dump.
//! By leveraging the engine's native binary format (`doom_game::savegame`),
//! we serialize the entire deterministic [`GameState`] without duplicating state
//! extraction logic in the app layer. It guarantees exact deterministic replay
//! when you resume the slaughter.
//!
//! # The Fine Print
//! This module handles the app-layer disk I/O, magic byte validation, and bridging
//! the `doom_game` save state directly over our active simulation state.
//!
//! Be warned: Save files created with different engine versions or incompatible
//! WADs may invite nasal demons and result in a [`SaveError::BadVersion`].

use doom_game::{
    GameState,
    savegame::{SaveGame, load_game as engine_load, save_game as engine_save},
};
use std::path::Path;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during the delicate act of manipulating time (saving/loading).
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// The physical realm rejected our request (file not found, permission denied, etc.).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to encode game state to bytes.

    /// File does not start with the `b"DRS1"` magic bytes. We don't read hieroglyphics.
    #[error("invalid save file magic")]
    BadMagic,

    /// Save file has an unsupported version number. Only modern sorcery is permitted.
    #[error("unsupported save version")]
    BadVersion,

    /// The save payload was truncated. An incomplete incantation!
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

/// Etches the current [`GameState`] into the disk at the given `path`.
///
/// Overwrites any existing file at `path` without mercy.
/// The game is saved into a specific `slot` for easy retrieval.
///
/// # The Hero's Journey
///
/// ```rust,no_run
/// # use doom_app::savegame::save_game;
/// # use doom_game::GameState;
/// # use std::path::Path;
/// let mut state = GameState::new("E1M1");
///
/// // The hero bravely conquers the first room...
/// // state.player.health() -= 10;
///
/// // Time to rest at the campfire.
/// let save_path = Path::new("save1.dsg");
/// save_game(save_path, &state, 1).expect("Failed to write save file!");
/// ```
///
/// # Errors
/// Returns [`SaveError::Io`] if the disk write fails (e.g., read-only filesystem).
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

/// Resurrects a fallen game state from the disk.
///
/// Reads the save file at `path`, validating its magic bytes and version,
/// before returning both the parsed header and the complete [`SaveGame`] payload.
///
/// # The Hero's Journey
///
/// ```rust,no_run
/// # use doom_app::savegame::load_game;
/// # use std::path::Path;
/// let save_path = Path::new("save1.dsg");
///
/// // A tragic end... but we can try again!
/// match load_game(save_path) {
///     Ok((header, payload)) => {
///         println!("Restoring: {}", core::str::from_utf8(&header.description).unwrap_or("Unknown"));
///         // Now apply the payload to your GameState!
///     }
///     Err(e) => eprintln!("The save file is corrupted: {}", e),
/// }
/// ```
///
/// # Errors
/// * Returns [`SaveError::Io`] if the file cannot be read.
/// * Returns [`SaveError::Truncated`] if the binary format is malformed or cut off.
/// * Returns [`SaveError::BadMagic`] if the file lacks the `b"DRS1"` signature.
/// * Returns [`SaveError::BadVersion`] for an unsupported version number.
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

/// Seamlessly overlays a loaded [`SaveGame`] onto the active [`GameState`].
///
/// Unlike older systems which only restored player inventory, this engine-native
/// system restores the *complete* simulation. Every Mobj, sector mover, and RNG
/// index is brought back to the exact moment it was saved.
///
/// # The Hero's Journey
///
/// ```rust,no_run
/// # use doom_app::savegame::{load_game, apply_save};
/// # use doom_game::GameState;
/// # use std::path::Path;
/// let mut current_state = GameState::new("E1M1");
/// let save_path = Path::new("save1.dsg");
///
/// if let Ok((_header, payload)) = load_game(save_path) {
///     // Overwrite the current timeline with the saved one.
///     apply_save(&mut current_state, &payload).expect("Failed to apply save!");
/// }
/// ```
///
/// # Errors
/// Currently always returns `Ok(())`, but exists as a `Result` for future-proofing
/// validation logic.
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
