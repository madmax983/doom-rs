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
//! This module handles the app-layer disk I/O, save format selection, and
//! bridging the `doom_game` save state directly over our active simulation
//! state.
//!
//! Be warned: Save files created with different engine versions or incompatible
//! WADs may invite nasal demons and result in a [`SaveError::BadVersion`].

use doom_game::{
    GameState,
    savegame::{
        SaveFormat, SaveGame, detect_save_format, load_game as engine_load,
        save_game_with_format as engine_save_with_format,
    },
};
use doom_types::CompatibilityProfile;
use std::path::Path;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during the delicate act of manipulating time (saving/loading).
#[derive(Debug, thiserror::Error)]
pub(crate) enum SaveError {
    /// The physical realm rejected our request (file not found, permission denied, etc.).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Save file header does not match any recognized format.
    #[error("invalid save file magic")]
    BadMagic,

    /// Save file has an unsupported version number. Only modern sorcery is permitted.
    #[error("unsupported save version")]
    BadVersion,

    /// The save payload was truncated. An incomplete incantation!
    #[error("save payload truncated")]
    Truncated,

    /// The save file exists, but not in the format required by the active compatibility mode.
    #[error("save file format mismatch: expected {expected:?}, found {actual:?}")]
    FormatMismatch {
        expected: SaveFormat,
        actual: SaveFormat,
    },

    /// Vanilla DSG payload support has not landed yet.
    #[error("vanilla DSG payload support is not implemented yet")]
    UnsupportedVanillaDsg,
}

impl From<doom_game::savegame::SaveError> for SaveError {
    fn from(err: doom_game::savegame::SaveError) -> Self {
        match err {
            doom_game::savegame::SaveError::TooShort => SaveError::Truncated,
            doom_game::savegame::SaveError::BadMagic => SaveError::BadMagic,
            doom_game::savegame::SaveError::BadVersion => SaveError::BadVersion,
            doom_game::savegame::SaveError::Truncated => SaveError::Truncated,
            doom_game::savegame::SaveError::UnsupportedVanillaDsg => {
                SaveError::UnsupportedVanillaDsg
            }
        }
    }
}

/// Map an app-level compatibility profile to the required on-disk save format.
#[must_use]
pub(crate) const fn save_format_for_compat(compat: CompatibilityProfile) -> SaveFormat {
    match compat {
        CompatibilityProfile::Extended => SaveFormat::DoomRs,
        CompatibilityProfile::VanillaStrict => SaveFormat::VanillaDsg,
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
/// # use doom_types::CompatibilityProfile;
/// # use std::path::Path;
/// let mut state = GameState::new("E1M1");
///
/// // The hero bravely conquers the first room...
/// // state.player.health() -= 10;
///
/// // Time to rest at the campfire.
/// let save_path = Path::new("save1.dsg");
/// save_game(save_path, &state, 1, CompatibilityProfile::Extended)
///     .expect("Failed to write save file!");
/// ```
///
/// # Errors
/// Returns [`SaveError::Io`] if the disk write fails (e.g., read-only filesystem).
/// Returns [`SaveError::UnsupportedVanillaDsg`] when strict compatibility is
/// selected before vanilla payload support exists.
pub(crate) fn save_game(
    path: &Path,
    gs: &GameState,
    slot: u8,
    compat: CompatibilityProfile,
) -> Result<(), SaveError> {
    save_game_with_format(path, gs, slot, save_format_for_compat(compat))
}

/// Save the current game using an explicit on-disk format, bypassing profile mapping.
pub(crate) fn save_game_with_format(
    path: &Path,
    gs: &GameState,
    slot: u8,
    format: SaveFormat,
) -> Result<(), SaveError> {
    // Generate an 8-byte padded level name.
    let mut level_name = [0u8; 8];
    let src_bytes = gs.level_name.as_bytes();
    let copy_len = src_bytes.len().min(8);
    level_name[..copy_len].copy_from_slice(&src_bytes[..copy_len]);

    let description = format!("Slot {slot}");

    // We assume skill 2 (Medium) for now since `GameState` does not currently store the skill level.
    // However, the `doom-app` sets skill at level spawn.
    // Let's pass 2 (HMP) to satisfy the engine signature.
    let data = engine_save_with_format(gs, &level_name, 2, &description, format)?;
    std::fs::write(path, &data)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// load_game
// ---------------------------------------------------------------------------

/// Resurrects a fallen game state from the disk.
///
/// Reads the save file at `path`, validates that it matches the active
/// compatibility profile, and returns both the parsed header and the complete
/// [`SaveGame`] payload.
///
/// # The Hero's Journey
///
/// ```rust,no_run
/// # use doom_app::savegame::load_game;
/// # use doom_types::CompatibilityProfile;
/// # use std::path::Path;
/// let save_path = Path::new("save1.dsg");
///
/// // A tragic end... but we can try again!
/// match load_game(save_path, CompatibilityProfile::Extended) {
///     Ok((header, payload)) => {
///         println!("Restoring: {}", core::str::from_utf8(&header.description).unwrap_or("Unknown").trim_end_matches('\0'));
///         // Now apply the payload to your GameState!
///     }
///     Err(e) => eprintln!("The save file is corrupted: {}", e),
/// }
/// ```
///
/// # Errors
/// * Returns [`SaveError::Io`] if the file cannot be read.
/// * Returns [`SaveError::Truncated`] if the binary format is malformed or cut off.
/// * Returns [`SaveError::BadMagic`] if the file header does not match a recognized save format.
/// * Returns [`SaveError::BadVersion`] for an unsupported version number.
/// * Returns [`SaveError::FormatMismatch`] if the file exists but is not valid for the active
///   compatibility profile.
/// * Returns [`SaveError::UnsupportedVanillaDsg`] if strict mode encounters a
///   plausible vanilla header before full vanilla payload support exists.
pub(crate) fn load_game(
    path: &Path,
    compat: CompatibilityProfile,
) -> Result<(doom_game::savegame::SaveHeader, SaveGame), SaveError> {
    load_game_with_format(path, save_format_for_compat(compat))
}

/// Load a game while requiring a specific detected on-disk format.
pub(crate) fn load_game_with_format(
    path: &Path,
    expected_format: SaveFormat,
) -> Result<(doom_game::savegame::SaveHeader, SaveGame), SaveError> {
    let data = std::fs::read(path)?;
    let actual_format = detect_save_format(&data)?;
    if actual_format != expected_format {
        return Err(SaveError::FormatMismatch {
            expected: expected_format,
            actual: actual_format,
        });
    }
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
/// # use doom_types::CompatibilityProfile;
/// # use std::path::Path;
/// let mut current_state = GameState::new("E1M1");
/// let save_path = Path::new("save1.dsg");
///
/// if let Ok((_header, payload)) = load_game(save_path, CompatibilityProfile::Extended) {
///     // Overwrite the current timeline with the saved one.
///     apply_save(&mut current_state, &payload).expect("Failed to apply save!");
/// }
/// ```
///
/// # Errors
/// Currently always returns `Ok(())`, but exists as a `Result` for future-proofing
/// validation logic.
pub(crate) fn apply_save(gs: &mut GameState, payload: &SaveGame) -> Result<(), SaveError> {
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
    use doom_game::{SAVE_MAGIC, SaveFormat};
    use doom_game::{Mobj, PlayerState};
    use doom_types::CompatibilityProfile;
    use doom_types::mobj_kind::MobjKind;
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

    fn vanilla_header_bytes() -> Vec<u8> {
        let mut data = Vec::new();
        let mut description = [0u8; 24];
        description[..12].copy_from_slice(b"Vanilla test");
        data.extend_from_slice(&description);

        let mut version = [0u8; 16];
        version[..11].copy_from_slice(b"version 109");
        data.extend_from_slice(&version);

        data.push(2);
        data.push(1);
        data.push(1);
        data.extend_from_slice(&[1, 0, 0, 0]);
        data.extend_from_slice(&[0x23, 0x01, 0x00]);
        data
    }

    #[test]
    fn extended_profile_writes_and_reads_doomrs_format() {
        let gs = make_gs();
        let path = temp_path("doom_rs_test_extended_profile.dsg");

        save_game(&path, &gs, 3, CompatibilityProfile::Extended)
            .expect("extended save must succeed");

        let data = std::fs::read(&path).expect("must read saved bytes");
        assert_eq!(&data[..4], &SAVE_MAGIC);

        let (_header, payload) =
            load_game(&path, CompatibilityProfile::Extended).expect("extended load must succeed");
        assert_eq!(payload.state.tic_num, gs.tic_num);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn vanilla_strict_save_does_not_fall_back_to_doomrs_bytes() {
        let gs = make_gs();
        let path = temp_path("doom_rs_test_strict_profile.dsg");

        let err = save_game(&path, &gs, 1, CompatibilityProfile::VanillaStrict)
            .expect_err("strict save should fail until vanilla payload support exists");
        assert!(matches!(err, SaveError::UnsupportedVanillaDsg));
        assert!(
            !path.exists(),
            "strict save failure must not leave behind a DoomRs save file"
        );
    }

    #[test]
    fn load_rejects_doomrs_file_when_strict_profile_expects_vanilla() {
        let gs = make_gs();
        let path = temp_path("doom_rs_test_profile_mismatch.dsg");

        save_game_with_format(&path, &gs, 2, SaveFormat::DoomRs).expect("doomrs save must succeed");

        let err = load_game(&path, CompatibilityProfile::VanillaStrict)
            .expect_err("strict load should reject DoomRs save files");
        assert!(matches!(
            err,
            SaveError::FormatMismatch {
                expected: SaveFormat::VanillaDsg,
                actual: SaveFormat::DoomRs
            }
        ));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn strict_profile_reports_unsupported_for_vanilla_header() {
        let path = temp_path("doom_rs_test_strict_profile_vanilla_header.dsg");
        std::fs::write(&path, vanilla_header_bytes()).expect("must write vanilla-like header");

        let err = load_game(&path, CompatibilityProfile::VanillaStrict)
            .expect_err("strict load should reject unimplemented vanilla payloads explicitly");
        assert!(matches!(err, SaveError::UnsupportedVanillaDsg));

        let _ = std::fs::remove_file(&path);
    }

    // -----------------------------------------------------------------------
    // Test 1: header roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn save_load_roundtrip_header() {
        let gs = make_gs();
        let path = temp_path("doom_rs_test_header.bin");

        save_game(&path, &gs, 3, CompatibilityProfile::Extended).expect("save_game must succeed");

        let (header, _payload) =
            load_game(&path, CompatibilityProfile::Extended).expect("load_game must succeed");

        assert_eq!(
            header.magic,
            doom_game::savegame::SAVE_MAGIC,
            "magic must be b\"DRS1\""
        );
        // Description starts with 'Slot 3'.
        let desc = core::str::from_utf8(&header.description)
            .unwrap_or("")
            .trim_end_matches('\0');
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

        save_game(&path, &gs, 0, CompatibilityProfile::Extended).expect("save_game must succeed");

        let (_header, payload) =
            load_game(&path, CompatibilityProfile::Extended).expect("load_game must succeed");

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
        std::fs::write(&path, b"").expect("write must succeed");

        let result = load_game(&path, CompatibilityProfile::Extended);
        assert!(result.is_err(), "loading an empty file must return Err");

        let _ = std::fs::remove_file(&path);
    }
}
