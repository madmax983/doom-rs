//! Cheat code detection and application.
//!
//! Tracks typed characters in a ring buffer and matches against classic Doom
//! cheat sequences. When a cheat is detected, `apply_cheat` modifies the
//! `GameState` accordingly.
//!
//! # Supported cheats
//! | Code       | Sequence     | Effect                             |
//! |------------|--------------|------------------------------------|
//! | GodMode    | `iddqd`      | Toggle invincibility               |
//! | IDKFA      | `idkfa`      | All weapons, ammo, and keys        |
//! | IDFA       | `idfa`       | All weapons and ammo (no keys)     |
//! | NoClip     | `idclip`     | Toggle noclip (Doom 2)             |
//! | NoClip     | `idspispopd` | Toggle noclip (Doom 1)             |
//! | IDBEHOLDX  | `idbeholdX`  | Toggle power-up X (V/S/I/R/A/L)   |
//! | LevelWarp  | `idclev`     | Level warp (caller reads digits)   |
//! | MusicChange| `idmus`      | Change music (caller reads digits) |

use doom_types::limits::{MAX_AMMO, NUM_AMMO};

use crate::player::powers;
use crate::state::GameState;

// ---------------------------------------------------------------------------
// Cheat sequences (lowercase)
// ---------------------------------------------------------------------------

/// God mode: IDDQD
pub const CHEAT_GOD: &[u8] = b"iddqd";
/// All weapons + ammo + keys: IDKFA
pub const CHEAT_IDKFA: &[u8] = b"idkfa";
/// All weapons + ammo (no keys): IDFA
pub const CHEAT_IDFA: &[u8] = b"idfa";
/// Noclip (Doom 2): IDCLIP
pub const CHEAT_NOCLIP: &[u8] = b"idclip";
/// Noclip (Doom 1): IDSPISPOPD
pub const CHEAT_NOCLIP_DOOM1: &[u8] = b"idspispopd";
/// IDBEHOLD prefix (followed by V, S, I, R, A, or L)
pub const CHEAT_BEHOLD_PREFIX: &[u8] = b"idbehold";
/// Level warp prefix: IDCLEV (followed by 2 digits)
pub const CHEAT_LEVELWARP: &[u8] = b"idclev";
/// Music change prefix: IDMUS (followed by 2 digits)
pub const CHEAT_MUSIC: &[u8] = b"idmus";

/// Duration for timed power-ups: 60 seconds at 35 tics/sec.
const POWER_DURATION_60S: u32 = 60 * 35;
/// Duration for light amplification: 120 seconds at 35 tics/sec.
const POWER_DURATION_120S: u32 = 120 * 35;

// ---------------------------------------------------------------------------
// CheatCode enum
// ---------------------------------------------------------------------------

/// Recognized cheat codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheatCode {
    /// IDDQD -- toggle god mode.
    GodMode,
    /// IDKFA -- give all weapons, ammo, and keys.
    AllWeaponsAmmoKeys,
    /// IDFA -- give all weapons and ammo.
    AllWeaponsAmmo,
    /// IDCLIP / IDSPISPOPD -- toggle noclip.
    NoClip,
    /// IDBEHOLDV -- toggle invulnerability.
    Invulnerability,
    /// IDBEHOLDS -- toggle strength / berserk.
    Strength,
    /// IDBEHOLDI -- toggle invisibility.
    Invisibility,
    /// IDBEHOLDR -- toggle radiation suit.
    RadSuit,
    /// IDBEHOLDA -- toggle all-map.
    AllMap,
    /// IDBEHOLDL -- toggle light amplification.
    LightAmp,
    /// IDCLEV + 2 digits -- warp to level.
    LevelWarp,
    /// IDMUS + 2 digits -- change music.
    MusicChange,
}

// ---------------------------------------------------------------------------
// CheatBuffer
// ---------------------------------------------------------------------------

/// Ring buffer of recently typed characters for cheat detection.
///
/// Characters are pushed one at a time as the player types. After each push
/// the caller should invoke `check_cheats` to see if a cheat sequence has
/// been completed.
#[derive(Debug, Clone)]
pub struct CheatBuffer {
    /// Ring buffer of recent characters.
    buffer: [u8; 32],
    /// Number of characters currently stored (capped at 32).
    len: usize,
}

impl CheatBuffer {
    /// Create a new, empty cheat buffer.
    pub fn new() -> Self {
        Self {
            buffer: [0u8; 32],
            len: 0,
        }
    }

    /// Push a character into the buffer.
    ///
    /// If the buffer is full (32 characters), the oldest character is
    /// discarded (ring-buffer semantics).
    pub fn push(&mut self, ch: u8) {
        if self.len < 32 {
            self.buffer[self.len] = ch;
            self.len += 1;
        } else {
            // Shift left by 1, drop oldest.
            self.buffer.copy_within(1..32, 0);
            self.buffer[31] = ch;
        }
    }

    /// Check whether the buffer ends with the given byte sequence.
    ///
    /// Returns `true` if the last `code.len()` characters in the buffer
    /// match `code` exactly.
    pub fn check(&self, code: &[u8]) -> bool {
        if code.is_empty() || self.len < code.len() {
            return false;
        }
        let start = self.len - code.len();
        self.buffer[start..self.len] == *code
    }

    /// Reset the buffer, discarding all stored characters.
    pub fn clear(&mut self) {
        self.buffer = [0u8; 32];
        self.len = 0;
    }
}

impl Default for CheatBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Check the buffer against all known cheat sequences.
///
/// Returns the first match found, checking longest sequences first to avoid
/// prefix conflicts (e.g. `idspispopd` is checked before `idfa`).
pub fn check_cheats(buffer: &CheatBuffer) -> Option<CheatCode> {
    // IDBEHOLD suffixes (9 chars each) -- check before shorter prefixes.
    if buffer.check(b"idbeholdv") {
        return Some(CheatCode::Invulnerability);
    }
    if buffer.check(b"idbeholds") {
        return Some(CheatCode::Strength);
    }
    if buffer.check(b"idbeholdi") {
        return Some(CheatCode::Invisibility);
    }
    if buffer.check(b"idbeholdr") {
        return Some(CheatCode::RadSuit);
    }
    if buffer.check(b"idbeholda") {
        return Some(CheatCode::AllMap);
    }
    if buffer.check(b"idbeholdl") {
        return Some(CheatCode::LightAmp);
    }

    // IDSPISPOPD (10 chars) -- check before shorter IDCLIP.
    if buffer.check(CHEAT_NOCLIP_DOOM1) {
        return Some(CheatCode::NoClip);
    }

    // IDCLIP (6 chars)
    if buffer.check(CHEAT_NOCLIP) {
        return Some(CheatCode::NoClip);
    }

    // IDCLEV (6 chars) -- level warp prefix (digits handled by caller).
    if buffer.check(CHEAT_LEVELWARP) {
        return Some(CheatCode::LevelWarp);
    }

    // IDDQD (5 chars)
    if buffer.check(CHEAT_GOD) {
        return Some(CheatCode::GodMode);
    }

    // IDKFA (5 chars)
    if buffer.check(CHEAT_IDKFA) {
        return Some(CheatCode::AllWeaponsAmmoKeys);
    }

    // IDMUS (5 chars) -- music change prefix (digits handled by caller).
    if buffer.check(CHEAT_MUSIC) {
        return Some(CheatCode::MusicChange);
    }

    // IDFA (4 chars)
    if buffer.check(CHEAT_IDFA) {
        return Some(CheatCode::AllWeaponsAmmo);
    }

    None
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

/// Apply the given cheat code to the game state.
///
/// Returns `true` if the cheat was successfully applied.
pub fn apply_cheat(gs: &mut GameState, code: CheatCode) -> bool {
    match code {
        CheatCode::GodMode => {
            gs.player.god_mode = !gs.player.god_mode;
            if gs.player.god_mode {
                gs.set_player_health_capped(100, 100);
            }
            true
        }

        CheatCode::AllWeaponsAmmoKeys => {
            // All weapons.
            for slot in gs.player.weapons.iter_mut() {
                *slot = true;
            }
            // Max ammo.
            for i in 0..NUM_AMMO {
                gs.player.max_ammo[i] = MAX_AMMO[i];
                gs.player.give_ammo(i, MAX_AMMO[i]);
            }
            // All 6 keys.
            gs.player.keys = 0x3F;
            true
        }

        CheatCode::AllWeaponsAmmo => {
            // All weapons.
            for slot in gs.player.weapons.iter_mut() {
                *slot = true;
            }
            // Max ammo.
            for i in 0..NUM_AMMO {
                gs.player.max_ammo[i] = MAX_AMMO[i];
                gs.player.give_ammo(i, MAX_AMMO[i]);
            }
            // Explicitly do NOT give keys.
            true
        }

        CheatCode::NoClip => {
            gs.player.noclip = !gs.player.noclip;
            true
        }

        CheatCode::Invulnerability => {
            toggle_power(
                &mut gs.player.powers[powers::PW_INVULNERABILITY],
                POWER_DURATION_60S,
            );
            true
        }

        CheatCode::Strength => {
            // Berserk is permanent (tics = 1 means "active indefinitely").
            toggle_power(&mut gs.player.powers[powers::PW_STRENGTH], 1);
            true
        }

        CheatCode::Invisibility => {
            toggle_power(
                &mut gs.player.powers[powers::PW_INVISIBILITY],
                POWER_DURATION_60S,
            );
            true
        }

        CheatCode::RadSuit => {
            toggle_power(
                &mut gs.player.powers[powers::PW_IRONFEET],
                POWER_DURATION_60S,
            );
            true
        }

        CheatCode::AllMap => {
            // AllMap is permanent (tics = 1 means "active indefinitely").
            toggle_power(&mut gs.player.powers[powers::PW_ALLMAP], 1);
            true
        }

        CheatCode::LightAmp => {
            toggle_power(
                &mut gs.player.powers[powers::PW_INFRARED],
                POWER_DURATION_120S,
            );
            true
        }

        CheatCode::LevelWarp | CheatCode::MusicChange => {
            // Caller handles the actual warp / music switch using trailing digits.
            true
        }
    }
}

/// Toggle a power-up: if currently active set to 0, otherwise set to `duration`.
fn toggle_power(power: &mut u32, duration: u32) {
    if *power != 0 {
        *power = 0;
    } else {
        *power = duration;
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Return the classic Doom HUD message for a cheat code.
pub fn cheat_message(code: CheatCode) -> &'static str {
    match code {
        CheatCode::GodMode => "Degreelessness Mode On",
        CheatCode::AllWeaponsAmmoKeys => "Very Happy Ammo Added",
        CheatCode::AllWeaponsAmmo => "Ammo (no strafe) Added",
        CheatCode::NoClip => "No Clipping Mode ON",
        CheatCode::Invulnerability => "Invulnerability",
        CheatCode::Strength => "Berserk!",
        CheatCode::Invisibility => "Partial Invisibility",
        CheatCode::RadSuit => "Radiation Shielding Suit On",
        CheatCode::AllMap => "Computer Area Map",
        CheatCode::LightAmp => "Light Amplification Visor On",
        CheatCode::LevelWarp => "Changing Level...",
        CheatCode::MusicChange => "Music Change",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;
    use doom_types::limits::{MAX_AMMO, NUM_AMMO, NUM_WEAPONS};

    // -- Helper: feed a string into a buffer ---
    fn feed(buf: &mut CheatBuffer, s: &[u8]) {
        for &ch in s {
            buf.push(ch);
        }
    }

    // -----------------------------------------------------------------------
    // CheatBuffer basics (tests 1-6)
    // -----------------------------------------------------------------------

    #[test]
    fn cheat_buffer_new_is_empty() {
        let buf = CheatBuffer::new();
        assert_eq!(buf.len, 0);
        assert!(buf.buffer.iter().all(|&b| b == 0));
    }

    #[test]
    fn push_adds_characters() {
        let mut buf = CheatBuffer::new();
        buf.push(b'i');
        buf.push(b'd');
        assert_eq!(buf.len, 2);
        assert_eq!(buf.buffer[0], b'i');
        assert_eq!(buf.buffer[1], b'd');
    }

    #[test]
    fn check_matches_exact_sequence() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"iddqd");
        assert!(buf.check(b"iddqd"));
    }

    #[test]
    fn check_returns_false_for_partial_match() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"iddq");
        assert!(!buf.check(b"iddqd"));
    }

    #[test]
    fn check_returns_false_for_wrong_sequence() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"hello");
        assert!(!buf.check(b"iddqd"));
    }

    #[test]
    fn clear_resets_buffer() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"iddqd");
        buf.clear();
        assert_eq!(buf.len, 0);
        assert!(!buf.check(b"iddqd"));
    }

    // -----------------------------------------------------------------------
    // check_cheats detection (tests 7-11, 23-24)
    // -----------------------------------------------------------------------

    #[test]
    fn check_cheats_detects_god_mode() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"iddqd");
        assert_eq!(check_cheats(&buf), Some(CheatCode::GodMode));
    }

    #[test]
    fn check_cheats_detects_idkfa() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idkfa");
        assert_eq!(check_cheats(&buf), Some(CheatCode::AllWeaponsAmmoKeys));
    }

    #[test]
    fn check_cheats_detects_idfa() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idfa");
        assert_eq!(check_cheats(&buf), Some(CheatCode::AllWeaponsAmmo));
    }

    #[test]
    fn check_cheats_detects_noclip() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idclip");
        assert_eq!(check_cheats(&buf), Some(CheatCode::NoClip));
    }

    #[test]
    fn check_cheats_returns_none_for_random_chars() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"xyzzy");
        assert_eq!(check_cheats(&buf), None);
    }

    #[test]
    fn check_cheats_idbehold_v() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idbeholdv");
        assert_eq!(check_cheats(&buf), Some(CheatCode::Invulnerability));
    }

    #[test]
    fn check_cheats_idbehold_s() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idbeholds");
        assert_eq!(check_cheats(&buf), Some(CheatCode::Strength));
    }

    #[test]
    fn check_cheats_idbehold_i() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idbeholdi");
        assert_eq!(check_cheats(&buf), Some(CheatCode::Invisibility));
    }

    #[test]
    fn check_cheats_idbehold_r() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idbeholdr");
        assert_eq!(check_cheats(&buf), Some(CheatCode::RadSuit));
    }

    #[test]
    fn check_cheats_idbehold_a() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idbeholda");
        assert_eq!(check_cheats(&buf), Some(CheatCode::AllMap));
    }

    #[test]
    fn check_cheats_idbehold_l() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idbeholdl");
        assert_eq!(check_cheats(&buf), Some(CheatCode::LightAmp));
    }

    #[test]
    fn check_cheats_idspispopd_returns_noclip() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idspispopd");
        assert_eq!(check_cheats(&buf), Some(CheatCode::NoClip));
    }

    #[test]
    fn check_cheats_detects_levelwarp() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idclev");
        assert_eq!(check_cheats(&buf), Some(CheatCode::LevelWarp));
    }

    #[test]
    fn check_cheats_detects_music() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"idmus");
        assert_eq!(check_cheats(&buf), Some(CheatCode::MusicChange));
    }

    // -----------------------------------------------------------------------
    // apply_cheat effects (tests 12-20)
    // -----------------------------------------------------------------------

    #[test]
    fn apply_god_mode_toggles_flag() {
        let mut gs = GameState::new("E1M1");
        assert!(!gs.player.god_mode);
        apply_cheat(&mut gs, CheatCode::GodMode);
        assert!(gs.player.god_mode);
        apply_cheat(&mut gs, CheatCode::GodMode);
        assert!(!gs.player.god_mode);
    }

    #[test]
    fn apply_god_mode_sets_health_to_100() {
        let mut gs = GameState::new("E1M1");
        gs.player.apply_damage(70); // health = 30
        assert_eq!(gs.player.health(), 30);
        apply_cheat(&mut gs, CheatCode::GodMode);
        assert_eq!(gs.player.health(), 100);
    }

    #[test]
    fn apply_idkfa_gives_all_weapons() {
        let mut gs = GameState::new("E1M1");
        apply_cheat(&mut gs, CheatCode::AllWeaponsAmmoKeys);
        for i in 0..NUM_WEAPONS {
            assert!(
                gs.player.weapons[i],
                "weapon slot {} should be true after IDKFA",
                i
            );
        }
    }

    #[test]
    fn apply_idkfa_gives_all_keys() {
        let mut gs = GameState::new("E1M1");
        apply_cheat(&mut gs, CheatCode::AllWeaponsAmmoKeys);
        assert_eq!(gs.player.keys, 0x3F);
    }

    #[test]
    fn apply_idkfa_maxes_all_ammo() {
        let mut gs = GameState::new("E1M1");
        apply_cheat(&mut gs, CheatCode::AllWeaponsAmmoKeys);
        for i in 0..NUM_AMMO {
            assert_eq!(
                gs.player.ammo(i),
                MAX_AMMO[i],
                "ammo[{}] should be maxed after IDKFA",
                i
            );
        }
    }

    #[test]
    fn apply_idfa_gives_weapons_ammo_but_not_keys() {
        let mut gs = GameState::new("E1M1");
        apply_cheat(&mut gs, CheatCode::AllWeaponsAmmo);
        for i in 0..NUM_WEAPONS {
            assert!(
                gs.player.weapons[i],
                "weapon slot {} should be true after IDFA",
                i
            );
        }
        for i in 0..NUM_AMMO {
            assert_eq!(
                gs.player.ammo(i),
                MAX_AMMO[i],
                "ammo[{}] should be maxed after IDFA",
                i
            );
        }
        // Keys must NOT be affected.
        assert_eq!(
            gs.player.keys, 0,
            "keys must be 0 after IDFA (no keys given)"
        );
    }

    #[test]
    fn apply_noclip_toggles_flag() {
        let mut gs = GameState::new("E1M1");
        assert!(!gs.player.noclip);
        apply_cheat(&mut gs, CheatCode::NoClip);
        assert!(gs.player.noclip);
        apply_cheat(&mut gs, CheatCode::NoClip);
        assert!(!gs.player.noclip);
    }

    #[test]
    fn apply_invulnerability_toggles_power() {
        let mut gs = GameState::new("E1M1");
        assert_eq!(gs.player.powers[powers::PW_INVULNERABILITY], 0);
        apply_cheat(&mut gs, CheatCode::Invulnerability);
        assert_eq!(gs.player.powers[powers::PW_INVULNERABILITY], 60 * 35);
        apply_cheat(&mut gs, CheatCode::Invulnerability);
        assert_eq!(gs.player.powers[powers::PW_INVULNERABILITY], 0);
    }

    #[test]
    fn apply_allmap_toggles_power() {
        let mut gs = GameState::new("E1M1");
        assert_eq!(gs.player.powers[powers::PW_ALLMAP], 0);
        apply_cheat(&mut gs, CheatCode::AllMap);
        assert_eq!(gs.player.powers[powers::PW_ALLMAP], 1);
        apply_cheat(&mut gs, CheatCode::AllMap);
        assert_eq!(gs.player.powers[powers::PW_ALLMAP], 0);
    }

    // -----------------------------------------------------------------------
    // Messages (test 21)
    // -----------------------------------------------------------------------

    #[test]
    fn cheat_message_returns_nonempty_for_each_code() {
        let codes = [
            CheatCode::GodMode,
            CheatCode::AllWeaponsAmmoKeys,
            CheatCode::AllWeaponsAmmo,
            CheatCode::NoClip,
            CheatCode::Invulnerability,
            CheatCode::Strength,
            CheatCode::Invisibility,
            CheatCode::RadSuit,
            CheatCode::AllMap,
            CheatCode::LightAmp,
            CheatCode::LevelWarp,
            CheatCode::MusicChange,
        ];
        for code in &codes {
            let msg = cheat_message(*code);
            assert!(
                !msg.is_empty(),
                "cheat_message({:?}) must not be empty",
                code
            );
        }
    }

    // -----------------------------------------------------------------------
    // Edge cases (test 22)
    // -----------------------------------------------------------------------

    #[test]
    fn buffer_overflow_does_not_panic() {
        let mut buf = CheatBuffer::new();
        // Push way more than 32 characters.
        for i in 0..100u8 {
            buf.push(b'a' + (i % 26));
        }
        assert_eq!(buf.len, 32);
        // Still functional -- feed a cheat at the end.
        feed(&mut buf, b"iddqd");
        assert_eq!(check_cheats(&buf), Some(CheatCode::GodMode));
    }

    // -----------------------------------------------------------------------
    // Additional toggle tests
    // -----------------------------------------------------------------------

    #[test]
    fn apply_strength_toggles_power() {
        let mut gs = GameState::new("E1M1");
        assert_eq!(gs.player.powers[powers::PW_STRENGTH], 0);
        apply_cheat(&mut gs, CheatCode::Strength);
        assert_eq!(gs.player.powers[powers::PW_STRENGTH], 1);
        apply_cheat(&mut gs, CheatCode::Strength);
        assert_eq!(gs.player.powers[powers::PW_STRENGTH], 0);
    }

    #[test]
    fn apply_invisibility_toggles_power() {
        let mut gs = GameState::new("E1M1");
        assert_eq!(gs.player.powers[powers::PW_INVISIBILITY], 0);
        apply_cheat(&mut gs, CheatCode::Invisibility);
        assert_eq!(gs.player.powers[powers::PW_INVISIBILITY], 60 * 35);
        apply_cheat(&mut gs, CheatCode::Invisibility);
        assert_eq!(gs.player.powers[powers::PW_INVISIBILITY], 0);
    }

    #[test]
    fn apply_radsuit_toggles_power() {
        let mut gs = GameState::new("E1M1");
        assert_eq!(gs.player.powers[powers::PW_IRONFEET], 0);
        apply_cheat(&mut gs, CheatCode::RadSuit);
        assert_eq!(gs.player.powers[powers::PW_IRONFEET], 60 * 35);
        apply_cheat(&mut gs, CheatCode::RadSuit);
        assert_eq!(gs.player.powers[powers::PW_IRONFEET], 0);
    }

    #[test]
    fn apply_lightamp_toggles_power() {
        let mut gs = GameState::new("E1M1");
        assert_eq!(gs.player.powers[powers::PW_INFRARED], 0);
        apply_cheat(&mut gs, CheatCode::LightAmp);
        assert_eq!(gs.player.powers[powers::PW_INFRARED], 120 * 35);
        apply_cheat(&mut gs, CheatCode::LightAmp);
        assert_eq!(gs.player.powers[powers::PW_INFRARED], 0);
    }

    #[test]
    fn apply_level_warp_returns_true() {
        let mut gs = GameState::new("E1M1");
        assert!(apply_cheat(&mut gs, CheatCode::LevelWarp));
    }

    #[test]
    fn apply_music_change_returns_true() {
        let mut gs = GameState::new("E1M1");
        assert!(apply_cheat(&mut gs, CheatCode::MusicChange));
    }

    #[test]
    fn check_with_prefix_chars_before_cheat() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"xyziddqd");
        assert_eq!(check_cheats(&buf), Some(CheatCode::GodMode));
    }

    #[test]
    fn empty_buffer_check_returns_false() {
        let buf = CheatBuffer::new();
        assert!(!buf.check(b"iddqd"));
        assert_eq!(check_cheats(&buf), None);
    }

    #[test]
    fn check_empty_code_returns_false() {
        let mut buf = CheatBuffer::new();
        feed(&mut buf, b"iddqd");
        assert!(!buf.check(b""));
    }

    #[test]
    fn god_mode_disable_does_not_change_health() {
        let mut gs = GameState::new("E1M1");
        // Enable god mode (health -> 100).
        apply_cheat(&mut gs, CheatCode::GodMode);
        assert!(gs.player.god_mode);
        assert_eq!(gs.player.health(), 100);
        // Disable god mode -- health stays at 100, not reset again.
        apply_cheat(&mut gs, CheatCode::GodMode);
        assert!(!gs.player.god_mode);
        assert_eq!(gs.player.health(), 100);
    }

    #[test]
    fn default_cheat_buffer_is_empty() {
        let buf = CheatBuffer::default();
        assert_eq!(buf.len, 0);
    }
}
