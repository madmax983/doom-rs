//! Cheat code detection and application.
//!
//! Cheats are typed during normal gameplay by watching the running stream of
//! keypresses for known sequences (e.g. "iddqd" for god mode).

use doom_game::{GameState, flags};
use doom_types::limits::MAX_AMMO;

// ---------------------------------------------------------------------------
// Cheat definitions
// ---------------------------------------------------------------------------

/// A cheat code definition.
#[derive(Debug, Clone)]
pub struct CheatDef {
    pub name: &'static str,
    /// The keypress sequence to match (lowercase ASCII).
    pub sequence: &'static str,
}

/// All standard Doom cheat codes.
pub const CHEATS: &[CheatDef] = &[
    CheatDef {
        name: "IDDQD",
        sequence: "iddqd",
    },
    CheatDef {
        name: "IDKFA",
        sequence: "idkfa",
    },
    CheatDef {
        name: "IDFA",
        sequence: "idfa",
    },
    CheatDef {
        name: "IDCLIP",
        sequence: "idclip",
    },
    CheatDef {
        name: "IDDT",
        sequence: "iddt",
    },
    CheatDef {
        name: "IDSPISPOPD",
        sequence: "idspispopd",
    },
    CheatDef {
        name: "IDBEHOLDS",
        sequence: "idbeholds",
    },
    CheatDef {
        name: "IDBEHOLDI",
        sequence: "idbeholdi",
    },
    CheatDef {
        name: "IDBEHOLDR",
        sequence: "idbeholdr",
    },
    CheatDef {
        name: "IDBEHOLDA",
        sequence: "idbeholda",
    },
    CheatDef {
        name: "IDBEHOLDV",
        sequence: "idbeholdv",
    },
    CheatDef {
        name: "IDBEHOLDL",
        sequence: "idbeholdl",
    },
];

// ---------------------------------------------------------------------------
// CheatDetector
// ---------------------------------------------------------------------------

/// Watches a stream of keypresses for any registered cheat sequence.
///
/// The detector maintains a rolling buffer of recent printable characters and
/// checks whether the buffer ends with any known cheat sequence after each
/// character is fed in.
pub struct CheatDetector {
    /// Rolling buffer of recent keypresses (max length = longest cheat + 1).
    buffer: String,
    max_len: usize,
}

impl CheatDetector {
    /// Create a new detector sized for the longest registered cheat sequence.
    pub fn new() -> Self {
        let max_len = CHEATS.iter().map(|c| c.sequence.len()).max().unwrap_or(16);
        Self {
            buffer: String::with_capacity(max_len + 1),
            max_len,
        }
    }

    /// Feed one character (lowercased internally).
    ///
    /// Returns the matched cheat name if the buffer now ends with a known
    /// sequence, otherwise returns `None`.
    pub fn feed(&mut self, ch: char) -> Option<&'static str> {
        if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
            self.buffer.push(ch.to_ascii_lowercase());
            if self.buffer.len() > self.max_len {
                let drain_to = self.buffer.len() - self.max_len;
                self.buffer.drain(..drain_to);
            }
            for cheat in CHEATS {
                if self.buffer.ends_with(cheat.sequence) {
                    self.buffer.clear();
                    return Some(cheat.name);
                }
            }
        }
        None
    }

    /// Clear the running buffer (e.g. on menu open or map change).
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for CheatDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Cheat application
// ---------------------------------------------------------------------------

/// Apply a triggered cheat to the game state.
///
/// Returns a static message string to display, or an empty string if the
/// cheat name is not recognised.
pub fn apply_cheat(gs: &mut GameState, cheat_name: &str) -> &'static str {
    match cheat_name {
        "IDDQD" => {
            // God mode: restore health to full.
            // Toggle MF_SHOOTABLE so the engine can't damage the player mobj.
            gs.player.set_health_capped(100, 100);
            if let Some(mo) = gs.mobjslab.get_mut(gs.player.handle) {
                mo.flags ^= flags::MF_SHOOTABLE;
            }
            "Degreelessness mode ON"
        }
        "IDKFA" => {
            // Full weapons, keys, max ammo, and 200% armor.
            gs.player.give_armor(200, 2);
            for slot in gs.player.weapons.iter_mut() {
                *slot = true;
            }
            for (ammo_idx, max_ammo) in MAX_AMMO.iter().copied().enumerate() {
                gs.player.give_ammo(ammo_idx, max_ammo);
            }
            gs.player.keys = 0x3F; // all 6 key bits
            "Very Happy Ammo Added"
        }
        "IDFA" => {
            // Full ammo and armor, no keys.
            gs.player.give_armor(200, 2);
            for slot in gs.player.weapons.iter_mut() {
                *slot = true;
            }
            for (ammo_idx, max_ammo) in MAX_AMMO.iter().copied().enumerate() {
                gs.player.give_ammo(ammo_idx, max_ammo);
            }
            "Ammo (no keys) Added"
        }
        "IDCLIP" | "IDSPISPOPD" => {
            // Toggle no-clip on the player mobj.
            if let Some(mo) = gs.mobjslab.get_mut(gs.player.handle) {
                mo.flags ^= flags::MF_NOCLIP;
                if mo.flags & flags::MF_NOCLIP != 0 {
                    "No Clipping Mode ON"
                } else {
                    "No Clipping Mode OFF"
                }
            } else {
                "No Clipping Mode"
            }
        }
        "IDDT" => {
            // Full automap reveal — automap state tracked externally.
            "Map Revealed"
        }
        "IDBEHOLDS" => {
            use doom_game::player::powers;
            gs.player.powers[powers::PW_STRENGTH] = 1;
            "Berserk!"
        }
        "IDBEHOLDI" => {
            use doom_game::player::powers;
            gs.player.powers[powers::PW_INVISIBILITY] = 60 * 35;
            "Partial Invisibility"
        }
        "IDBEHOLDR" => {
            use doom_game::player::powers;
            gs.player.powers[powers::PW_IRONFEET] = 60 * 35;
            "Radiation Shielding Suit"
        }
        "IDBEHOLDA" => {
            use doom_game::player::powers;
            gs.player.powers[powers::PW_ALLMAP] = 1;
            "Computer Area Map"
        }
        "IDBEHOLDV" => {
            use doom_game::player::powers;
            gs.player.powers[powers::PW_INVULNERABILITY] = 30 * 35;
            "Invulnerability"
        }
        "IDBEHOLDL" => {
            use doom_game::player::powers;
            gs.player.powers[powers::PW_INFRARED] = 120 * 35;
            "Light Amplification Visor"
        }
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // CheatDetector tests
    // -----------------------------------------------------------------------

    #[test]
    fn cheat_detector_detects_iddqd() {
        let mut det = CheatDetector::new();
        let chars = ['i', 'd', 'd', 'q', 'd'];
        let results: Vec<_> = chars.iter().map(|&c| det.feed(c)).collect();
        assert_eq!(results[4], Some("IDDQD"));
        assert!(results[..4].iter().all(|r| r.is_none()));
    }

    #[test]
    fn cheat_detector_ignores_partial() {
        let mut det = CheatDetector::new();
        for &c in &['i', 'd', 'd', 'q'] {
            assert!(det.feed(c).is_none(), "partial sequence must not trigger");
        }
    }

    #[test]
    fn cheat_detector_detects_mid_noise() {
        // Noise characters before the valid sequence must not prevent detection.
        let mut det = CheatDetector::new();
        det.feed('x');
        det.feed('y');
        det.feed('i');
        det.feed('d');
        det.feed('d');
        det.feed('q');
        let result = det.feed('d');
        assert_eq!(result, Some("IDDQD"));
    }

    #[test]
    fn cheat_detector_clears_after_match() {
        let mut det = CheatDetector::new();
        // First detection.
        for &c in &['i', 'd', 'd', 'q'] {
            det.feed(c);
        }
        assert_eq!(det.feed('d'), Some("IDDQD"));
        // Feed the same sequence again — must still detect.
        for &c in &['i', 'd', 'd', 'q'] {
            assert!(det.feed(c).is_none());
        }
        assert_eq!(det.feed('d'), Some("IDDQD"));
    }

    // -----------------------------------------------------------------------
    // apply_cheat tests
    // -----------------------------------------------------------------------

    fn make_test_gs() -> GameState {
        use doom_game::{Mobj, PlayerState};
        use doom_types::mobj_kind::MobjKind;
        use doom_types::{Bam, Fixed16_16};

        let mut gs = GameState::new("E1M1");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs
    }

    #[test]
    fn apply_cheat_iddqd_sets_health() {
        let mut gs = make_test_gs();
        // Damage the player first.
        gs.player.apply_damage(50);
        assert_eq!(gs.player.health(), 50);
        apply_cheat(&mut gs, "IDDQD");
        assert_eq!(gs.player.health(), 100);
    }

    #[test]
    fn apply_cheat_idkfa_gives_all_weapons() {
        let mut gs = make_test_gs();
        apply_cheat(&mut gs, "IDKFA");
        assert!(
            gs.player.weapons.iter().all(|&w| w),
            "IDKFA must set all weapon slots"
        );
    }

    #[test]
    fn cheat_iddt_returns_map_revealed() {
        let mut gs = make_test_gs();
        let msg = apply_cheat(&mut gs, "IDDT");
        assert_eq!(msg, "Map Revealed", "IDDT must return 'Map Revealed'");
    }

    #[test]
    fn havoc_test_apply_cheat_invalid_string() {
        let mut gs = make_test_gs();
        let msg = apply_cheat(&mut gs, "NOT_A_REAL_CHEAT");
        assert_eq!(msg, "");
    }
}
