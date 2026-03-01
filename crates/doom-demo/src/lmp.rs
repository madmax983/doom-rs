//! LMP binary format: header parsing/serialisation and per-tic wire entries.
//!
//! The vanilla Doom v1.9 LMP format is:
//! - 13-byte header (version, skill, episode, map, flags, player presence)
//! - N × 4-byte tic entries (one per tic per present player)
//! - 1-byte sentinel `0x80`

use doom_game::TicCmd;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// LMP version byte for vanilla Doom 1.9.
pub const LMP_VERSION: u8 = 0x6F; // 111

/// Sentinel byte that terminates the tic stream.
pub const DEMO_SENTINEL: u8 = 0x80;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur while parsing or writing LMP demo data.
#[derive(Debug, thiserror::Error)]
pub enum DemoError {
    /// The byte slice or stream was shorter than expected.
    #[error("data too short")]
    TooShort,
    /// The demo file reports an unsupported version number.
    #[error("invalid demo version: {0}")]
    BadVersion(u8),
    /// The tic stream ended without a sentinel byte.
    #[error("demo ended unexpectedly")]
    UnexpectedEof,
    /// An I/O error occurred while writing the demo.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// LmpHeader
// ---------------------------------------------------------------------------

/// 13-byte LMP demo header (vanilla Doom v1.9 layout).
#[derive(Debug, Clone, PartialEq)]
pub struct LmpHeader {
    /// Demo format version (`LMP_VERSION` = 0x6F for v1.9).
    pub version: u8,
    /// Skill level (0–4).
    pub skill: u8,
    /// Episode number (1–3).
    pub episode: u8,
    /// Map number within episode (1–9).
    pub map: u8,
    /// Deathmatch mode: 0 = co-op, 1 = dm, 2 = alt-dm.
    pub deathmatch: u8,
    /// Whether monsters respawn (respawn = true).
    pub respawn: bool,
    /// Whether fast monsters are enabled.
    pub fast: bool,
    /// Whether the nomonsters flag is set.
    pub nomonsters: bool,
    /// Index of the recording player (0–3).
    pub consoleplayer: u8,
    /// Which of the four player slots are occupied.
    pub player_present: [bool; 4],
}

impl LmpHeader {
    /// Create a header for a standard single-player recording.
    ///
    /// Player 0 is present; all flags default to off.
    pub fn new_singleplayer(skill: u8, episode: u8, map: u8) -> Self {
        Self {
            version: LMP_VERSION,
            skill,
            episode,
            map,
            deathmatch: 0,
            respawn: false,
            fast: false,
            nomonsters: false,
            consoleplayer: 0,
            player_present: [true, false, false, false],
        }
    }

    /// Parse a 13-byte header from the beginning of `data`.
    ///
    /// # Errors
    /// Returns [`DemoError::TooShort`] if `data.len() < 13`.
    pub fn parse(data: &[u8]) -> Result<Self, DemoError> {
        if data.len() < 13 {
            return Err(DemoError::TooShort);
        }
        Ok(Self {
            version: data[0],
            skill: data[1],
            episode: data[2],
            map: data[3],
            deathmatch: data[4],
            respawn: data[5] != 0,
            fast: data[6] != 0,
            nomonsters: data[7] != 0,
            consoleplayer: data[8],
            player_present: [
                data[9] != 0,
                data[10] != 0,
                data[11] != 0,
                data[12] != 0,
            ],
        })
    }

    /// Serialise the header to its 13-byte wire representation.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 13] {
        [
            self.version,
            self.skill,
            self.episode,
            self.map,
            self.deathmatch,
            self.respawn as u8,
            self.fast as u8,
            self.nomonsters as u8,
            self.consoleplayer,
            self.player_present[0] as u8,
            self.player_present[1] as u8,
            self.player_present[2] as u8,
            self.player_present[3] as u8,
        ]
    }
}

// ---------------------------------------------------------------------------
// LmpTicEntry
// ---------------------------------------------------------------------------

/// 4-byte per-player per-tic entry in the LMP stream.
///
/// The LMP format stores angle as a single `i8` (the high byte of the
/// 16-bit `TicCmd::angle_turn` field).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LmpTicEntry {
    /// Forward/backward movement (-128..127).
    pub forward_move: i8,
    /// Lateral strafe (-128..127).
    pub side_move: i8,
    /// Angle delta, truncated to i8 (high byte of the 16-bit BAM delta).
    pub angle_turn: i8,
    /// Button bitfield (`bt::BT_*` flags).
    pub buttons: u8,
}

impl LmpTicEntry {
    /// Convert a [`TicCmd`] to its LMP wire representation.
    ///
    /// `angle_turn` is stored as the high byte of the i16 field, matching
    /// vanilla Doom's LMP serialisation.
    pub fn from_ticcmd(cmd: &TicCmd) -> Self {
        Self {
            forward_move: cmd.forward_move,
            side_move: cmd.side_move,
            // Vanilla LMP stores the high byte of the 16-bit angle field.
            angle_turn: (cmd.angle_turn >> 8) as i8,
            buttons: cmd.buttons,
        }
    }

    /// Reconstruct a [`TicCmd`] from this LMP entry.
    ///
    /// The i8 angle is sign-extended back to i16 and shifted into the high byte.
    #[must_use]
    pub fn to_ticcmd(&self) -> TicCmd {
        let mut cmd = TicCmd::default();
        cmd.forward_move = self.forward_move;
        cmd.side_move = self.side_move;
        cmd.angle_turn = (self.angle_turn as i16) << 8;
        cmd.buttons = self.buttons;
        cmd
    }

    /// Serialise this entry to its 4-byte wire representation.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 4] {
        [
            self.forward_move as u8,
            self.side_move as u8,
            self.angle_turn as u8,
            self.buttons,
        ]
    }

    /// Parse a 4-byte slice into an [`LmpTicEntry`].
    ///
    /// # Errors
    /// Returns [`DemoError::TooShort`] if `b.len() < 4`.
    pub fn from_bytes(b: &[u8]) -> Result<Self, DemoError> {
        if b.len() < 4 {
            return Err(DemoError::TooShort);
        }
        Ok(Self {
            forward_move: b[0] as i8,
            side_move: b[1] as i8,
            angle_turn: b[2] as i8,
            buttons: b[3],
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_game::TicCmd;

    #[test]
    fn lmp_header_roundtrip() {
        let original = LmpHeader::new_singleplayer(3, 1, 1);
        let bytes = original.to_bytes();
        let parsed = LmpHeader::parse(&bytes).expect("parse should succeed");
        assert_eq!(original, parsed);
    }

    #[test]
    fn lmp_header_too_short_errors() {
        let result = LmpHeader::parse(&[0u8; 5]);
        assert!(
            matches!(result, Err(DemoError::TooShort)),
            "expected TooShort, got {result:?}"
        );
    }

    #[test]
    fn lmp_tic_entry_roundtrip() {
        let entry = LmpTicEntry {
            forward_move: 50,
            side_move: -10,
            angle_turn: 3,
            buttons: 1,
        };
        let bytes = entry.to_bytes();
        let parsed = LmpTicEntry::from_bytes(&bytes).expect("parse should succeed");
        assert_eq!(entry, parsed);
    }

    #[test]
    fn lmp_ticcmd_conversion() {
        let cmd = TicCmd::default();
        let entry = LmpTicEntry::from_ticcmd(&cmd);
        let _back: TicCmd = entry.to_ticcmd();
        // Conversion must not panic; default TicCmd is all-zeros.
        assert_eq!(entry.forward_move, 0);
        assert_eq!(entry.side_move, 0);
        assert_eq!(entry.angle_turn, 0);
        assert_eq!(entry.buttons, 0);
    }
}
