//! Intermission (level-exit) statistics and par time tables.
//!
//! When a level exit is triggered, `GameState::compute_intermission_stats()`
//! snapshots the player's kill/item/secret counts alongside the level totals
//! to produce an `IntermissionStats` suitable for displaying a tally screen.

use crate::state::GameState;

// ---------------------------------------------------------------------------
// IntermissionStats
// ---------------------------------------------------------------------------

/// End-of-level statistics for the intermission / tally screen.
///
/// All counts are raw values; percentage display is the caller's
/// responsibility (avoiding division-by-zero when totals are 0).
#[derive(Debug, Clone, Default)]
pub struct IntermissionStats {
    /// Monsters killed by the player.
    pub kills: u32,
    /// Total killable monsters in the level.
    pub total_kills: u32,
    /// Items picked up by the player.
    pub items: u32,
    /// Total collectable items in the level.
    pub total_items: u32,
    /// Secret sectors discovered by the player.
    pub secrets: u32,
    /// Total secret sectors in the level.
    pub total_secrets: u32,
    /// Time spent in the level (in tics, 35 tics = 1 second).
    pub time_tics: u32,
    /// Par time for the level (in tics, 35 tics = 1 second).
    pub par_time_tics: u32,
}

impl GameState {
    /// Build an `IntermissionStats` from the current game state.
    ///
    /// Call this when the player triggers a level exit, before loading
    /// the next map.
    pub fn compute_intermission_stats(&self) -> IntermissionStats {
        IntermissionStats {
            kills: self.player.kill_count,
            total_kills: self.stats.total_kills,
            items: self.player.item_count,
            total_items: self.stats.total_items,
            secrets: self.player.secret_count,
            total_secrets: self.stats.total_secrets,
            time_tics: self.stats.level_time,
            par_time_tics: par_time(&self.level_name),
        }
    }
}

// ---------------------------------------------------------------------------
// Par times
// ---------------------------------------------------------------------------

/// Par times for Doom 1 Episode 1 maps (E1M1 through E1M9), in seconds.
/// Index 0 = E1M1, index 8 = E1M9.
const DOOM1_E1_PAR_SECS: [u32; 9] = [30, 75, 120, 90, 165, 180, 180, 30, 165];

/// Par times for Doom 1 Episode 2 maps (E2M1 through E2M9), in seconds.
const DOOM1_E2_PAR_SECS: [u32; 9] = [90, 90, 90, 120, 90, 360, 240, 30, 170];

/// Par times for Doom 1 Episode 3 maps (E3M1 through E3M9), in seconds.
const DOOM1_E3_PAR_SECS: [u32; 9] = [90, 45, 90, 150, 90, 90, 165, 30, 135];

/// Par times for Doom 2 maps (MAP01 through MAP32), in seconds.
const DOOM2_PAR_SECS: [u32; 32] = [
    30, 90, 120, 120, 90, 150, 120, 120, 270, 90, // MAP01-MAP10
    210, 150, 150, 150, 210, 150, 420, 150, 210, 150, // MAP11-MAP20
    240, 150, 180, 150, 150, 300, 330, 420, 300, 180, // MAP21-MAP30
    120, 30, // MAP31-MAP32
];

/// Look up the par time for a level by name, returned in tics (35/sec).
///
/// Recognizes `E<ep>M<map>` (Doom 1) and `MAP<nn>` (Doom 2) patterns.
/// Returns 0 for unrecognized map names.
pub fn par_time(level_name: &str) -> u32 {
    let name = level_name.trim().to_uppercase();

    // Doom 1 format: ExMy
    if name.len() == 4 && name.as_bytes()[0] == b'E' && name.as_bytes()[2] == b'M' {
        if name.is_ascii() {
            let ep = (name.as_bytes()[1] as char).to_digit(10);
            let map = (name.as_bytes()[3] as char).to_digit(10);
            if let (Some(ep), Some(map)) = (ep, map) {
                let table = match ep {
                    1 => Some(&DOOM1_E1_PAR_SECS[..]),
                    2 => Some(&DOOM1_E2_PAR_SECS[..]),
                    3 => Some(&DOOM1_E3_PAR_SECS[..]),
                    _ => None,
                };
                if let Some(table) = table {
                    if (1..=9).contains(&map) {
                        return table[(map - 1) as usize] * 35;
                    }
                }
            }
        }
        return 0;
    }

    // Doom 2 format: MAPxx
    if name.len() == 5 && name.starts_with("MAP") {
        if let Ok(num) = name[3..].parse::<usize>() {
            if (1..=32).contains(&num) {
                return DOOM2_PAR_SECS[num - 1] * 35;
            }
        }
        return 0;
    }

    0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::Mobj;
    use crate::player::PlayerState;
    use crate::state::GameState;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    /// Helper: create a `GameState` with a live player and some stats.
    fn make_gs_with_stats(
        kills: u32,
        total_kills: u32,
        items: u32,
        total_items: u32,
        secrets: u32,
        total_secrets: u32,
        level_time: u32,
        level_name: &str,
    ) -> GameState {
        let mut gs = GameState::new(level_name);
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs.player.kill_count = kills;
        gs.player.item_count = items;
        gs.player.secret_count = secrets;
        gs.stats.total_kills = total_kills;
        gs.stats.total_items = total_items;
        gs.stats.total_secrets = total_secrets;
        gs.stats.level_time = level_time;
        gs
    }

    // --- compute_intermission_stats ---

    #[test]
    fn intermission_stats_correct_kills() {
        let gs = make_gs_with_stats(10, 20, 0, 0, 0, 0, 0, "E1M1");
        let stats = gs.compute_intermission_stats();
        assert_eq!(stats.kills, 10);
        assert_eq!(stats.total_kills, 20);
    }

    #[test]
    fn intermission_stats_correct_items() {
        let gs = make_gs_with_stats(0, 0, 5, 15, 0, 0, 0, "E1M1");
        let stats = gs.compute_intermission_stats();
        assert_eq!(stats.items, 5);
        assert_eq!(stats.total_items, 15);
    }

    #[test]
    fn intermission_stats_correct_secrets() {
        let gs = make_gs_with_stats(0, 0, 0, 0, 3, 5, 0, "E1M1");
        let stats = gs.compute_intermission_stats();
        assert_eq!(stats.secrets, 3);
        assert_eq!(stats.total_secrets, 5);
    }

    #[test]
    fn intermission_stats_zero_totals_no_panic() {
        // Zero totals must not cause division-by-zero or panic.
        let gs = make_gs_with_stats(0, 0, 0, 0, 0, 0, 0, "E1M1");
        let stats = gs.compute_intermission_stats();
        assert_eq!(stats.kills, 0);
        assert_eq!(stats.total_kills, 0);
        assert_eq!(stats.items, 0);
        assert_eq!(stats.total_items, 0);
        assert_eq!(stats.secrets, 0);
        assert_eq!(stats.total_secrets, 0);
    }

    #[test]
    fn intermission_stats_includes_level_time() {
        let gs = make_gs_with_stats(0, 0, 0, 0, 0, 0, 350, "E1M1");
        let stats = gs.compute_intermission_stats();
        assert_eq!(stats.time_tics, 350);
    }

    #[test]
    fn intermission_stats_includes_par_time() {
        let gs = make_gs_with_stats(0, 0, 0, 0, 0, 0, 0, "E1M1");
        let stats = gs.compute_intermission_stats();
        // E1M1 par = 30 seconds * 35 = 1050 tics
        assert_eq!(stats.par_time_tics, 1050);
    }

    // --- par_time ---

    #[test]
    fn par_time_e1m1() {
        assert_eq!(par_time("E1M1"), 30 * 35);
    }

    #[test]
    fn par_time_e1m9() {
        assert_eq!(par_time("E1M9"), 165 * 35);
    }

    #[test]
    fn par_time_e2m1() {
        assert_eq!(par_time("E2M1"), 90 * 35);
    }

    #[test]
    fn par_time_e3m5() {
        assert_eq!(par_time("E3M5"), 90 * 35);
    }

    #[test]
    fn par_time_map01() {
        assert_eq!(par_time("MAP01"), 30 * 35);
    }

    #[test]
    fn par_time_map32() {
        assert_eq!(par_time("MAP32"), 30 * 35);
    }

    #[test]
    fn par_time_unknown_returns_zero() {
        assert_eq!(par_time("UNKNOWN"), 0);
        assert_eq!(par_time("E4M1"), 0);
        assert_eq!(par_time("MAP00"), 0);
        assert_eq!(par_time("MAP33"), 0);
        assert_eq!(par_time(""), 0);
    }

    #[test]
    fn par_time_case_insensitive() {
        assert_eq!(par_time("e1m1"), 30 * 35);
        assert_eq!(par_time("map01"), 30 * 35);
    }
}
