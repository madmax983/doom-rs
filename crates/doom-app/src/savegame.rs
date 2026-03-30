//! Save/load game state to/from disk.
//!
//! Uses a flat binary format: a fixed-size [`SaveHeader`] followed by a
//! [`SavePayload`] containing the essential player and world state needed
//! to resume a session.  Both structs derive `bincode::Encode`/`Decode`
//! using bincode 2.x with `bincode::config::standard()`.
//!
//! # File layout
//! ```text
//! [SaveHeader (bincode)] [SavePayload (bincode)]
//! ```
//!
//! # Quick save/load slots
//! Slot numbers 0-7 map to file names managed by the caller.  This module
//! only cares about the [`SaveHeader::slot`] field written into the file.

use doom_game::GameState;
use doom_types::{Bam, Fixed16_16};
use std::path::Path;

// ---------------------------------------------------------------------------
// SaveHeader
// ---------------------------------------------------------------------------

/// Fixed-layout header written at the start of every doom-rs save file.
#[derive(Debug, bincode::Encode, bincode::Decode)]
pub struct SaveHeader {
    /// Magic bytes — must equal [`SaveHeader::MAGIC`] (`b"DOOM"`).
    pub magic: [u8; 4],
    /// Save file format version — must equal [`SaveHeader::VERSION`] (`1`).
    pub version: u32,
    /// Save slot (0-7).
    pub slot: u8,
    /// Null-padded ASCII description string (max 23 visible characters).
    pub description: [u8; 24],
    /// Game tic at the time the save was written.
    pub tic_num: u32,
}

impl SaveHeader {
    /// Magic bytes identifying a doom-rs save file.
    pub const MAGIC: [u8; 4] = *b"DOOM";

    /// Current save file format version.
    pub const VERSION: u32 = 1;

    /// Create a new header for the given slot and tic number.
    ///
    /// `desc` is truncated to 23 bytes if longer; the remainder is
    /// null-padded to fill the 24-byte fixed field.
    pub fn new(slot: u8, tic_num: u32, desc: &str) -> Self {
        let mut description = [0u8; 24];
        let mut len = 0;
        for ch in desc.chars() {
            if len + ch.len_utf8() > 23 {
                break;
            }
            len += ch.len_utf8();
        }
        let bytes = desc.as_bytes();
        description[..len].copy_from_slice(&bytes[..len]);
        Self {
            magic: Self::MAGIC,
            version: Self::VERSION,
            slot,
            description,
            tic_num,
        }
    }

    /// Return the description as a `&str`, trimmed at the first null byte.
    ///
    /// Returns `"?"` if the bytes are not valid UTF-8.
    #[cfg(test)]
    pub fn description_str(&self) -> &str {
        let end = self.description.iter().position(|&b| b == 0).unwrap_or(24);
        std::str::from_utf8(&self.description[..end]).unwrap_or("?")
    }
}

// ---------------------------------------------------------------------------
// SavePayload
// ---------------------------------------------------------------------------

/// Minimal game state needed to resume from a save point.
///
/// Rather than serializing the full [`GameState`] (which includes the entire
/// `MobjSlab` arena and is non-trivially serializable), we capture the fields
/// the player directly experiences.  On load, the world is re-initialized to
/// map defaults and only player inventory/position is restored.
#[derive(Debug, bincode::Encode, bincode::Decode)]
pub struct SavePayload {
    /// Game tic at save time (mirrors [`SaveHeader::tic_num`]).
    pub tic_num: u32,

    /// RNG table index (0-255) for deterministic replay continuation.
    pub rng_index: u32,

    // --- Player position (extracted from the player Mobj) ---
    /// Player X position in map units (truncated from Fixed16_16).
    pub player_x: i32,
    /// Player Y position in map units (truncated from Fixed16_16).
    pub player_y: i32,
    /// Player facing angle as a raw 32-bit BAM value.
    pub player_angle: u32,

    // --- Inventory ---
    /// Player health points.
    pub player_health: i32,
    /// Player armor points.
    pub player_armor: i32,
    /// Armor type (0 = none, 1 = green, 2 = blue).
    pub player_armor_type: u8,
    /// Ammo pools: [Bullets, Shells, Cells, Rockets] (NUM_AMMO = 4).
    pub player_ammo: [u32; 4],
    /// Bitmask of owned weapons (bit i = `weapons[i]` is true).
    pub player_weapons: u64,
    /// Collected key bitmask.
    pub player_keys: u8,
    /// Currently active weapon index (as `WeaponType as u8`).
    pub active_weapon: u8,

    // --- Statistics (placeholder) ---
    /// Kill count at save time.
    pub kill_count: u32,
    /// Item count at save time.
    pub item_count: u32,
    /// Secret count at save time.
    pub secret_count: u32,
}

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
    #[error("encode error: {0}")]
    Encode(String),

    /// Failed to decode bytes into game state.
    #[error("decode error: {0}")]
    Decode(String),

    /// File does not start with the `b"DOOM"` magic bytes.
    #[error("invalid save file magic")]
    BadMagic,

    /// Save file has an unsupported version number.
    #[error("unsupported save version: {0}")]
    BadVersion(u32),
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
/// Returns [`SaveError::Encode`] if bincode encoding fails (should not happen
/// for well-formed data).
pub fn save_game(path: &Path, gs: &GameState, slot: u8) -> Result<(), SaveError> {
    let header = SaveHeader::new(slot, gs.tic_num, &format!("Slot {slot}"));
    let payload = build_payload(gs);

    let config = bincode::config::standard();

    let mut buf = Vec::new();
    bincode::encode_into_std_write(&header, &mut buf, config)
        .map_err(|e| SaveError::Encode(e.to_string()))?;
    bincode::encode_into_std_write(&payload, &mut buf, config)
        .map_err(|e| SaveError::Encode(e.to_string()))?;

    std::fs::write(path, &buf)?;
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
/// Returns [`SaveError::BadMagic`] if the file lacks the `b"DOOM"` signature.
/// Returns [`SaveError::BadVersion`] for an unsupported version number.
pub fn load_game(path: &Path) -> Result<(SaveHeader, SavePayload), SaveError> {
    let data = std::fs::read(path)?;
    let config = bincode::config::standard();

    let (header, consumed): (SaveHeader, usize) =
        bincode::decode_from_slice(&data, config).map_err(|e| SaveError::Decode(e.to_string()))?;

    if header.magic != SaveHeader::MAGIC {
        return Err(SaveError::BadMagic);
    }
    if header.version != SaveHeader::VERSION {
        return Err(SaveError::BadVersion(header.version));
    }

    let (payload, _): (SavePayload, usize) = bincode::decode_from_slice(&data[consumed..], config)
        .map_err(|e| SaveError::Decode(e.to_string()))?;

    Ok((header, payload))
}

// ---------------------------------------------------------------------------
// apply_save
// ---------------------------------------------------------------------------

/// Apply a loaded [`SavePayload`] to `gs`, restoring inventory and position.
///
/// The player `Mobj` position is updated if the player handle is still valid.
/// Call this after re-initializing the level geometry (which re-spawns the
/// player Mobj so the handle is valid).
///
/// # Errors
/// Currently always returns `Ok(())`.  The `Result` return is reserved for
/// future validation (e.g. level mismatch checks).
pub fn apply_save(gs: &mut GameState, payload: &SavePayload) -> Result<(), SaveError> {
    // Restore simulation state.
    gs.tic_num = payload.tic_num;
    gs.rng.set_index(payload.rng_index);

    // Restore statistics.
    gs.stats.kill_count = payload.kill_count;
    gs.stats.item_count = payload.item_count;
    gs.stats.secret_count = payload.secret_count;

    // Restore player health.
    // `set_health_capped` clamps to [0, cap]; use a large cap to restore exact value.
    gs.player.set_health_capped(payload.player_health, i32::MAX);

    // Restore armor.
    // `give_armor` only increases armor (no-op if payload <= current).
    // Since apply_save is called on a freshly-spawned pistol-start state
    // (armor == 0), this always sets the correct value.
    gs.player
        .give_armor(payload.player_armor, payload.player_armor_type);

    // Restore ammo: drain each pool then refill to the saved value.
    for i in 0..4usize {
        let current = gs.player.ammo(i);
        gs.player.use_ammo(i, current);
        gs.player.give_ammo(i, payload.player_ammo[i]);
    }

    // Restore keys.
    gs.player.keys = payload.player_keys;

    // Restore weapons from bitmask.
    for i in 0..9usize {
        gs.player.weapons[i] = (payload.player_weapons >> i) & 1 == 1;
    }

    // Restore active weapon.
    if let Some(wt) = doom_game::WeaponType::from_num(payload.active_weapon as usize) {
        gs.player.weapon = wt;
        doom_game::setup_psprites(&mut gs.player);
    }

    // Restore player Mobj position if the handle is valid.
    if let Some(mo) = gs.mobjslab.get_mut(gs.player.handle) {
        mo.x = Fixed16_16::from_int(payload.player_x);
        mo.y = Fixed16_16::from_int(payload.player_y);
        mo.angle = Bam(payload.player_angle);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// build_payload (private)
// ---------------------------------------------------------------------------

/// Extract the save payload from a live `GameState`.
fn build_payload(gs: &GameState) -> SavePayload {
    // Extract player mobj position; fall back to (0,0,0) if handle is invalid.
    let (px, py, pangle) = gs
        .mobjslab
        .get(gs.player.handle)
        .map(|mo| (mo.x.to_int(), mo.y.to_int(), mo.angle.0))
        .unwrap_or((0, 0, 0));

    // Build weapons bitmask: bit i is set if `player.weapons[i]` is true.
    let weapons_mask: u64 =
        gs.player.weapons.iter().enumerate().fold(
            0u64,
            |acc, (i, &has)| if has { acc | (1u64 << i) } else { acc },
        );

    // Collect ammo pools 0-3.
    let player_ammo = [
        gs.player.ammo(0),
        gs.player.ammo(1),
        gs.player.ammo(2),
        gs.player.ammo(3),
    ];

    SavePayload {
        tic_num: gs.tic_num,
        rng_index: gs.rng.index(),
        player_x: px,
        player_y: py,
        player_angle: pangle,
        player_health: gs.player.health(),
        player_armor: gs.player.armor(),
        player_armor_type: gs.player.armor_type,
        player_ammo,
        player_weapons: weapons_mask,
        player_keys: gs.player.keys,
        active_weapon: gs.player.weapon as u8,
        kill_count: gs.stats.kill_count,
        item_count: gs.stats.item_count,
        secret_count: gs.stats.secret_count,
    }
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

        assert_eq!(header.magic, SaveHeader::MAGIC, "magic must be b\"DOOM\"");
        assert_eq!(header.version, SaveHeader::VERSION, "version must be 1");
        assert_eq!(header.slot, 3, "slot must round-trip");
        assert_eq!(header.tic_num, 42, "tic_num must round-trip");

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

        assert_eq!(payload.tic_num, gs.tic_num, "payload tic_num must match gs");
        assert_eq!(
            payload.rng_index,
            gs.rng.index(),
            "rng_index must round-trip"
        );
        assert_eq!(
            payload.player_health,
            gs.player.health(),
            "health must round-trip"
        );
        assert_eq!(payload.player_x, 100, "player_x must round-trip");
        assert_eq!(payload.player_y, 200, "player_y must round-trip");
        assert_eq!(
            payload.player_angle, 0x4000_0000,
            "player_angle must round-trip"
        );

        // Clean up.
        let _ = std::fs::remove_file(&path);
    }

    // -----------------------------------------------------------------------
    // Test 3: bad magic returns SaveError::BadMagic
    // -----------------------------------------------------------------------

    #[test]
    fn load_bad_magic_errors() {
        let path = temp_path("doom_rs_test_bad_magic.bin");

        // Write a header with wrong magic using bincode so the format is valid
        // except for the magic bytes.
        let bad_header = SaveHeader {
            magic: *b"XXXX",
            version: SaveHeader::VERSION,
            slot: 0,
            description: [0u8; 24],
            tic_num: 0,
        };
        let payload = SavePayload {
            tic_num: 0,
            rng_index: 0,
            player_x: 0,
            player_y: 0,
            player_angle: 0,
            player_health: 100,
            player_armor: 0,
            player_armor_type: 0,
            player_ammo: [0; 4],
            player_weapons: 0,
            player_keys: 0,
            active_weapon: 1,
            kill_count: 0,
            item_count: 0,
            secret_count: 0,
        };
        let config = bincode::config::standard();
        let mut buf = Vec::new();
        bincode::encode_into_std_write(&bad_header, &mut buf, config).unwrap();
        bincode::encode_into_std_write(&payload, &mut buf, config).unwrap();
        std::fs::write(&path, &buf).unwrap();

        let result = load_game(&path);
        assert!(
            matches!(result, Err(SaveError::BadMagic)),
            "expected BadMagic, got: {result:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    // -----------------------------------------------------------------------
    // Test 4: empty file returns Err (not panic)
    // -----------------------------------------------------------------------

    #[test]
    fn load_empty_file_errors() {
        let path = temp_path("doom_rs_test_empty.bin");
        std::fs::write(&path, b"").unwrap();

        let result = load_game(&path);
        assert!(result.is_err(), "loading an empty file must return Err");

        let _ = std::fs::remove_file(&path);
    }

    // -----------------------------------------------------------------------
    // Test 5: description string roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn save_header_description() {
        let header = SaveHeader::new(0, 42, "My Save");
        assert_eq!(
            header.description_str(),
            "My Save",
            "description_str must return the exact input"
        );
        assert_eq!(header.tic_num, 42);
        assert_eq!(header.slot, 0);
    }

    // -----------------------------------------------------------------------
    // Bonus: description truncates at 23 bytes
    // -----------------------------------------------------------------------

    #[test]
    fn save_header_description_truncated() {
        let long_desc = "A".repeat(40);
        let header = SaveHeader::new(1, 0, &long_desc);
        let s = header.description_str();
        assert!(
            s.len() <= 23,
            "description must be truncated to at most 23 bytes"
        );
    }

    // -----------------------------------------------------------------------
    // Bonus: apply_save restores tic_num and rng_index
    // -----------------------------------------------------------------------

    #[test]
    fn apply_save_restores_tic_and_rng() {
        let gs_orig = make_gs();
        let path = temp_path("doom_rs_test_apply.bin");

        save_game(&path, &gs_orig, 0).expect("save must succeed");
        let (_header, payload) = load_game(&path).expect("load must succeed");

        // Create a fresh GameState (simulates restarting the level).
        let mut gs_new = make_gs();
        gs_new.tic_num = 999;
        for _ in 0..50 {
            gs_new.rng.next_byte();
        }

        apply_save(&mut gs_new, &payload).expect("apply must succeed");

        assert_eq!(gs_new.tic_num, gs_orig.tic_num, "tic_num must be restored");
        assert_eq!(
            gs_new.rng.index(),
            gs_orig.rng.index(),
            "rng index must be restored"
        );

        let _ = std::fs::remove_file(&path);
    }
}
