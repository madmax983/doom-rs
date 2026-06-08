//! LMP demo header: 13-byte binary header for the vanilla Doom 1.9 LMP format.
//!
//! The header encodes the demo version, skill/episode/map settings, game flags
//! (deathmatch, respawn, fast, nomonsters), the recording player index, and a
//! bitmask of which player slots are occupied.

/// LMP version byte for vanilla Doom 1.9 (`109` decimal, `0x6D` hex).
pub const LMP_VERSION_1_9: u8 = 109;

/// Sentinel byte that terminates the tic stream in an LMP file.
pub const LMP_TERMINATOR: u8 = 0x80;

/// Size of the LMP header in bytes.
pub const LMP_HEADER_SIZE: usize = 13;

/// 13-byte LMP demo header (vanilla Doom v1.9 layout).
///
/// ```text
/// Byte  0: version (109 = 1.9)
/// Byte  1: skill (0-4)
/// Byte  2: episode (1-3 for Doom1, 1 for Doom2)
/// Byte  3: map (1-9 for Doom1, 1-32 for Doom2)
/// Byte  4: deathmatch (0=coop, 1=dm, 2=altdm)
/// Byte  5: respawn (0 or 1)
/// Byte  6: fast (0 or 1)
/// Byte  7: nomonsters (0 or 1)
/// Byte  8: consoleplayer (0-3)
/// Byte  9: player1present (0 or 1)
/// Byte 10: player2present (0 or 1)
/// Byte 11: player3present (0 or 1)
/// Byte 12: player4present (0 or 1)
/// ```
///
/// ## Examples
/// ```
/// use doom_demo::LmpHeader;
/// let header = LmpHeader::new_singleplayer(3, 1, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LmpHeader {
    /// Demo format version (`LMP_VERSION_1_9` = 109 for v1.9).
    pub version: u8,
    /// Skill level (0-4).
    pub skill: u8,
    /// Episode number (1-3 for Doom 1, 1 for Doom 2).
    pub episode: u8,
    /// Map number within episode (1-9 for Doom 1, 1-32 for Doom 2).
    pub map: u8,
    /// Deathmatch mode: 0 = co-op, 1 = deathmatch, 2 = alt-deathmatch.
    pub deathmatch: u8,
    /// Whether monsters respawn.
    pub respawn: bool,
    /// Whether fast monsters are enabled.
    pub fast: bool,
    /// Whether the nomonsters flag is set.
    pub nomonsters: bool,
    /// Index of the recording player (0-3).
    pub consoleplayer: u8,
    /// Which of the four player slots are occupied.
    pub players_present: [bool; 4],
}

impl LmpHeader {
    /// Create a header for a standard single-player recording.
    ///
    /// Player 0 is present; all flags default to off.
///
/// ## Examples
/// ```
/// use doom_demo::LmpHeader;
/// let header = LmpHeader::new_singleplayer(3, 1, 1);
/// assert_eq!(header.skill, 3);
/// ```
    #[must_use]
    pub const fn new_singleplayer(skill: u8, episode: u8, map: u8) -> Self {
        Self {
            version: LMP_VERSION_1_9,
            skill,
            episode,
            map,
            deathmatch: 0,
            respawn: false,
            fast: false,
            nomonsters: false,
            consoleplayer: 0,
            players_present: [true, false, false, false],
        }
    }

    /// Serialize the header to its 13-byte wire representation.
///
/// ## Examples
/// ```
/// use doom_demo::LmpHeader;
/// let header = LmpHeader::new_singleplayer(3, 1, 1);
/// let bytes = header.to_bytes();
/// assert_eq!(bytes.len(), 13);
/// ```
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        vec![
            self.version,
            self.skill,
            self.episode,
            self.map,
            self.deathmatch,
            u8::from(self.respawn),
            u8::from(self.fast),
            u8::from(self.nomonsters),
            self.consoleplayer,
            u8::from(self.players_present[0]),
            u8::from(self.players_present[1]),
            u8::from(self.players_present[2]),
            u8::from(self.players_present[3]),
        ]
    }

    /// Parse a 13-byte header from the beginning of `data`.
    ///
    /// Returns `None` if the data is shorter than 13 bytes.
///
/// ## Examples
/// ```
/// use doom_demo::LmpHeader;
/// let header = LmpHeader::new_singleplayer(3, 1, 1);
/// let bytes = header.to_bytes();
/// let parsed = LmpHeader::from_bytes(&bytes).unwrap();
/// ```
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < LMP_HEADER_SIZE {
            return None;
        }
        Some(Self {
            version: data[0],
            skill: data[1],
            episode: data[2],
            map: data[3],
            deathmatch: data[4],
            respawn: data[5] != 0,
            fast: data[6] != 0,
            nomonsters: data[7] != 0,
            consoleplayer: data[8],
            players_present: [data[9] != 0, data[10] != 0, data[11] != 0, data[12] != 0],
        })
    }

    /// Count the number of present players.
///
/// ## Examples
/// ```
/// use doom_demo::LmpHeader;
/// let header = LmpHeader::new_singleplayer(3, 1, 1);
/// assert_eq!(header.num_players(), 1);
/// ```
    #[must_use]
    pub fn num_players(&self) -> usize {
        self.players_present.iter().filter(|&&p| p).count()
    }

    /// Bytes per tic in the LMP stream: 4 bytes per present player.
///
/// ## Examples
/// ```
/// use doom_demo::LmpHeader;
/// let header = LmpHeader::new_singleplayer(3, 1, 1);
/// assert_eq!(header.tic_size(), 4);
/// ```
    #[must_use]
    pub fn tic_size(&self) -> usize {
        4 * self.num_players()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> LmpHeader {
        LmpHeader {
            version: LMP_VERSION_1_9,
            skill: 3,
            episode: 1,
            map: 1,
            deathmatch: 0,
            respawn: false,
            fast: false,
            nomonsters: false,
            consoleplayer: 0,
            players_present: [true, false, false, false],
        }
    }

    // Test 1: to_bytes produces 13 bytes
    #[test]
    fn to_bytes_produces_13_bytes() {
        let header = sample_header();
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), LMP_HEADER_SIZE);
    }

    // Test 2: from_bytes roundtrip
    #[test]
    fn from_bytes_roundtrip() {
        let original = sample_header();
        let bytes = original.to_bytes();
        let parsed = LmpHeader::from_bytes(&bytes).expect("parse should succeed");
        assert_eq!(original, parsed);
    }

    // Test 3: from_bytes with too-short data returns None
    #[test]
    fn from_bytes_too_short_returns_none() {
        assert!(LmpHeader::from_bytes(&[0u8; 5]).is_none());
        assert!(LmpHeader::from_bytes(&[]).is_none());
        assert!(LmpHeader::from_bytes(&[0u8; 12]).is_none());
    }

    // Test 4: num_players counts correctly (1 player)
    #[test]
    fn num_players_single_player() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        assert_eq!(header.num_players(), 1);
    }

    // Test 5: num_players counts correctly (4 players)
    #[test]
    fn num_players_four_players() {
        let header = LmpHeader {
            players_present: [true, true, true, true],
            ..sample_header()
        };
        assert_eq!(header.num_players(), 4);
    }

    // Test 6: tic_size is 4 * num_players
    #[test]
    fn tic_size_matches_num_players() {
        let h1 = LmpHeader::new_singleplayer(3, 1, 1);
        assert_eq!(h1.tic_size(), 4);

        let h2 = LmpHeader {
            players_present: [true, true, false, false],
            ..sample_header()
        };
        assert_eq!(h2.tic_size(), 8);

        let h4 = LmpHeader {
            players_present: [true, true, true, true],
            ..sample_header()
        };
        assert_eq!(h4.tic_size(), 16);
    }

    #[test]
    fn new_singleplayer_has_correct_version() {
        let header = LmpHeader::new_singleplayer(2, 1, 3);
        assert_eq!(header.version, LMP_VERSION_1_9);
        assert_eq!(header.skill, 2);
        assert_eq!(header.episode, 1);
        assert_eq!(header.map, 3);
        assert_eq!(header.consoleplayer, 0);
        assert!(header.players_present[0]);
        assert!(!header.players_present[1]);
    }

    #[test]
    fn roundtrip_with_all_flags_set() {
        let header = LmpHeader {
            version: LMP_VERSION_1_9,
            skill: 4,
            episode: 3,
            map: 9,
            deathmatch: 2,
            respawn: true,
            fast: true,
            nomonsters: true,
            consoleplayer: 2,
            players_present: [true, false, true, false],
        };
        let bytes = header.to_bytes();
        let parsed = LmpHeader::from_bytes(&bytes).expect("value must exist in test");
        assert_eq!(header, parsed);
    }

    #[test]
    fn num_players_zero_players() {
        let header = LmpHeader {
            players_present: [false, false, false, false],
            ..sample_header()
        };
        assert_eq!(header.num_players(), 0);
        assert_eq!(header.tic_size(), 0);
    }
}
