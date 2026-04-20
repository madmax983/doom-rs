//! Switch texture handling and key checking for linedef activation.
//!
//! Manages:
//! - Switch texture toggling when a switch-type linedef is activated.
//! - Clearing linedef specials after one-shot triggers fire.
//! - Key possession checks for locked doors.

use crate::state::GameState;

// ---------------------------------------------------------------------------
// Key types
// ---------------------------------------------------------------------------

/// The six key types in Doom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// Blue keycard.
    BlueCard,
    /// Red keycard.
    RedCard,
    /// Yellow keycard.
    YellowCard,
    /// Blue skull key.
    BlueSkull,
    /// Red skull key.
    RedSkull,
    /// Yellow skull key.
    YellowSkull,
}

impl KeyType {
    /// Convert to the player key bitmask constant.
    pub fn to_bitmask(self) -> u8 {
        match self {
            KeyType::BlueCard => crate::player::KEY_BLUE_CARD,
            KeyType::RedCard => crate::player::KEY_RED_CARD,
            KeyType::YellowCard => crate::player::KEY_YELLOW_CARD,
            KeyType::BlueSkull => crate::player::KEY_BLUE_SKULL,
            KeyType::RedSkull => crate::player::KEY_RED_SKULL,
            KeyType::YellowSkull => crate::player::KEY_YELLOW_SKULL,
        }
    }

    /// Return all six key types.
    pub fn all() -> [KeyType; 6] {
        [
            KeyType::BlueCard,
            KeyType::RedCard,
            KeyType::YellowCard,
            KeyType::BlueSkull,
            KeyType::RedSkull,
            KeyType::YellowSkull,
        ]
    }
}

// ---------------------------------------------------------------------------
// Key checking
// ---------------------------------------------------------------------------

/// Check if the player has the required key.
pub fn player_has_key(gs: &GameState, key: KeyType) -> bool {
    gs.player.has_key(key.to_bitmask())
}

// ---------------------------------------------------------------------------
// Switch texture toggling
// ---------------------------------------------------------------------------

/// Switch texture pairs: `(off_name, on_name)` where off = `SW1xxx`,
/// on = `SW2xxx`.
///
/// When a switch linedef is activated, the wall texture is swapped from one
/// variant to the other.
///
/// These are the 29 standard Doom switch texture pairs.
pub const SWITCH_PAIRS: [([u8; 8], [u8; 8]); 29] = [
    (*b"SW1BRCOM", *b"SW2BRCOM"),
    (*b"SW1BRN1\0", *b"SW2BRN1\0"),
    (*b"SW1BRN2\0", *b"SW2BRN2\0"),
    (*b"SW1BRNGN", *b"SW2BRNGN"),
    (*b"SW1BROWN", *b"SW2BROWN"),
    (*b"SW1COMM\0", *b"SW2COMM\0"),
    (*b"SW1COMP\0", *b"SW2COMP\0"),
    (*b"SW1DIRT\0", *b"SW2DIRT\0"),
    (*b"SW1EXIT\0", *b"SW2EXIT\0"),
    (*b"SW1GRAY\0", *b"SW2GRAY\0"),
    (*b"SW1GRAY1", *b"SW2GRAY1"),
    (*b"SW1METAL", *b"SW2METAL"),
    (*b"SW1PIPE\0", *b"SW2PIPE\0"),
    (*b"SW1SLAD\0", *b"SW2SLAD\0"),
    (*b"SW1STARG", *b"SW2STARG"),
    (*b"SW1STON1", *b"SW2STON1"),
    (*b"SW1STON2", *b"SW2STON2"),
    (*b"SW1STONE", *b"SW2STONE"),
    (*b"SW1STRTN", *b"SW2STRTN"),
    (*b"SW1BLUE\0", *b"SW2BLUE\0"),
    (*b"SW1CMT\0\0", *b"SW2CMT\0\0"),
    (*b"SW1GARG\0", *b"SW2GARG\0"),
    (*b"SW1GSTON", *b"SW2GSTON"),
    (*b"SW1HOT\0\0", *b"SW2HOT\0\0"),
    (*b"SW1LION\0", *b"SW2LION\0"),
    (*b"SW1SATYR", *b"SW2SATYR"),
    (*b"SW1SKIN\0", *b"SW2SKIN\0"),
    (*b"SW1VINE\0", *b"SW2VINE\0"),
    (*b"SW1WOOD\0", *b"SW2WOOD\0"),
];

/// Check if a texture name matches one side of a switch pair and toggle it.
///
/// Checks the provided `texture` against all known switch pairs. If found,
/// returns `Some(opposite)` with the alternate texture name. Returns `None`
/// if the texture is not a known switch texture.
pub fn find_switch_opposite(texture: &[u8; 8]) -> Option<[u8; 8]> {
    for (off, on) in &SWITCH_PAIRS {
        if texture == off {
            return Some(*on);
        }
        if texture == on {
            return Some(*off);
        }
    }
    None
}

/// Toggle the switch texture on the front (right) sidedef of a linedef.
///
/// Checks the upper, middle, and lower textures of the right sidedef against
/// known switch pairs. If a match is found, swaps to the alternate texture.
///
/// Returns `true` if a texture was toggled.
pub fn toggle_switch_texture(level: &mut doom_map::Level, linedef_index: usize) -> bool {
    let Some(ld) = level.linedefs.get(linedef_index) else {
        return false;
    };
    let sd_idx = ld.right_sidedef as usize;
    let Some(sd) = level.sidedefs.get(sd_idx) else {
        return false;
    };

    // Check middle texture first (most common for switches).
    if let Some(opposite) = find_switch_opposite(&sd.middle_texture) {
        level.sidedefs[sd_idx].middle_texture = opposite;
        return true;
    }
    // Check upper texture.
    if let Some(opposite) = find_switch_opposite(&sd.upper_texture) {
        level.sidedefs[sd_idx].upper_texture = opposite;
        return true;
    }
    // Check lower texture.
    if let Some(opposite) = find_switch_opposite(&sd.lower_texture) {
        level.sidedefs[sd_idx].lower_texture = opposite;
        return true;
    }

    false
}

/// Clear a linedef's special after a once-trigger activates.
pub fn clear_linedef_special(level: &mut doom_map::Level, linedef_index: usize) {
    if let Some(ld) = level.linedefs.get_mut(linedef_index) {
        ld.special = 0;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::Mobj;
    use crate::state::GameState;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    fn make_gs_with_player() -> GameState {
        let mut gs = GameState::new("TEST");
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let handle = gs.mobjslab.alloc(mo);
        gs.player.handle = handle;
        gs
    }

    // -----------------------------------------------------------------------
    // Tests: player_has_key
    // -----------------------------------------------------------------------

    #[test]
    fn player_has_blue_card() {
        let mut gs = make_gs_with_player();
        gs.player.give_key(crate::player::KEY_BLUE_CARD);
        assert!(player_has_key(&gs, KeyType::BlueCard));
    }

    #[test]
    fn player_missing_red_card() {
        let gs = make_gs_with_player();
        assert!(!player_has_key(&gs, KeyType::RedCard));
    }

    #[test]
    fn player_has_yellow_card() {
        let mut gs = make_gs_with_player();
        gs.player.give_key(crate::player::KEY_YELLOW_CARD);
        assert!(player_has_key(&gs, KeyType::YellowCard));
    }

    #[test]
    fn player_has_blue_skull() {
        let mut gs = make_gs_with_player();
        gs.player.give_key(crate::player::KEY_BLUE_SKULL);
        assert!(player_has_key(&gs, KeyType::BlueSkull));
    }

    #[test]
    fn player_has_red_skull() {
        let mut gs = make_gs_with_player();
        gs.player.give_key(crate::player::KEY_RED_SKULL);
        assert!(player_has_key(&gs, KeyType::RedSkull));
    }

    #[test]
    fn player_has_yellow_skull() {
        let mut gs = make_gs_with_player();
        gs.player.give_key(crate::player::KEY_YELLOW_SKULL);
        assert!(player_has_key(&gs, KeyType::YellowSkull));
    }

    #[test]
    fn all_six_key_types_work() {
        let mut gs = make_gs_with_player();
        for key in KeyType::all() {
            assert!(
                !player_has_key(&gs, key),
                "{:?} should not be held initially",
                key
            );
            gs.player.give_key(key.to_bitmask());
            assert!(
                player_has_key(&gs, key),
                "{:?} should be held after giving",
                key
            );
        }
    }

    #[test]
    fn key_type_to_bitmask_distinct() {
        let masks: Vec<u8> = KeyType::all().iter().map(|k| k.to_bitmask()).collect();
        // All masks should be distinct and non-zero.
        for &m in &masks {
            assert_ne!(m, 0);
        }
        for i in 0..masks.len() {
            for j in (i + 1)..masks.len() {
                assert_ne!(masks[i], masks[j], "bitmasks must be distinct");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Tests: find_switch_opposite
    // -----------------------------------------------------------------------

    #[test]
    fn known_switch_toggles() {
        let opposite = find_switch_opposite(b"SW1EXIT\0");
        assert_eq!(opposite, Some(*b"SW2EXIT\0"));
    }

    #[test]
    fn unknown_texture_returns_none() {
        let opposite = find_switch_opposite(b"STARTAN3");
        assert_eq!(opposite, None);
    }

    #[test]
    fn toggle_is_bidirectional() {
        // SW1 -> SW2
        assert_eq!(find_switch_opposite(b"SW1COMP\0"), Some(*b"SW2COMP\0"));
        // SW2 -> SW1
        assert_eq!(find_switch_opposite(b"SW2COMP\0"), Some(*b"SW1COMP\0"));
    }

    #[test]
    fn all_switch_pairs_bidirectional() {
        for (off, on) in &SWITCH_PAIRS {
            assert_eq!(
                find_switch_opposite(off),
                Some(*on),
                "off->on failed for {:?}",
                core::str::from_utf8(off)
            );
            assert_eq!(
                find_switch_opposite(on),
                Some(*off),
                "on->off failed for {:?}",
                core::str::from_utf8(on)
            );
        }
    }

    // -----------------------------------------------------------------------
    // Tests: toggle_switch_texture
    // -----------------------------------------------------------------------

    fn make_switch_level(middle_tex: [u8; 8]) -> doom_map::Level {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).unwrap();

        let reject = doom_map::Reject::parse_lump(&[0u8], 1).unwrap();
        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 29,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: doom_map::SIDEDEF_NONE,
            }],
            sidedefs: vec![doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: middle_tex,
                sector: 0,
            }],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: 0 },
                doom_map::Vertex { x: 64, y: 0 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            }],
            reject,
            blockmap,
        }
    }

    #[test]
    fn toggle_switch_texture_swaps_middle() {
        let mut level = make_switch_level(*b"SW1EXIT\0");
        let result = toggle_switch_texture(&mut level, 0);
        assert!(result, "should find and toggle switch texture");
        assert_eq!(level.sidedefs[0].middle_texture, *b"SW2EXIT\0");
    }

    #[test]
    fn toggle_switch_texture_unknown_returns_false() {
        let mut level = make_switch_level(*b"STARTAN3");
        let result = toggle_switch_texture(&mut level, 0);
        assert!(!result, "unknown texture should not toggle");
    }

    #[test]
    fn toggle_switch_texture_round_trip() {
        let mut level = make_switch_level(*b"SW1EXIT\0");
        toggle_switch_texture(&mut level, 0);
        assert_eq!(level.sidedefs[0].middle_texture, *b"SW2EXIT\0");
        toggle_switch_texture(&mut level, 0);
        assert_eq!(level.sidedefs[0].middle_texture, *b"SW1EXIT\0");
    }

    // -----------------------------------------------------------------------
    // Tests: clear_linedef_special
    // -----------------------------------------------------------------------

    #[test]
    fn clear_linedef_special_zeroes_out() {
        let mut level = make_switch_level(*b"SW1EXIT\0");
        assert_ne!(level.linedefs[0].special, 0);
        clear_linedef_special(&mut level, 0);
        assert_eq!(level.linedefs[0].special, 0);
    }

    #[test]
    fn clear_linedef_special_out_of_bounds_is_safe() {
        let mut level = make_switch_level(*b"SW1EXIT\0");
        // Should not panic.
        clear_linedef_special(&mut level, 999);
    }
}
