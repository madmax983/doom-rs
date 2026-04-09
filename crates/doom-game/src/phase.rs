//! Game phase state machine and level transition logic.
//!
//! `GamePhaseController` drives the top-level flow:
//! `TitleScreen` -> `Playing` -> `Intermission` -> `Playing` / `Finale` -> `TitleScreen`.
//!
//! `MapId` encodes the current map identifier and knows how to compute the
//! next map for both Doom 1 (ExMy) and Doom 2 (MAPxx) progression.

use crate::intermission::{self, IntermissionStats};
use crate::state::{ExitRequest, GameState};

// ---------------------------------------------------------------------------
// MapId
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MapFormat {
    Doom1,
    Doom2,
}

/// Identifies a specific map in Doom 1 (episode+map) or Doom 2 (map only).
///
/// For Doom 1: `episode` is 1-3, `map` is 1-9.
/// For Doom 2: `episode` remains `1` for demo/header compatibility, `map` is
/// 1-32, and `format` disambiguates `MAP01` from `E1M1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapId {
    /// Episode number (1-3 for Doom 1, 1 for Doom 2).
    pub episode: u8,
    /// Map number within the episode (1-9 for Doom 1, 1-32 for Doom 2).
    pub map: u8,
    format: MapFormat,
}

impl MapId {
    /// Create a new MapId.
    pub fn new(episode: u8, map: u8) -> Self {
        Self {
            episode,
            map,
            format: MapFormat::Doom1,
        }
    }

    /// Create a Doom 2 MapId (episode is always 1).
    pub fn doom2(map: u8) -> Self {
        Self {
            episode: 1,
            map,
            format: MapFormat::Doom2,
        }
    }

    /// Parse a canonical map name such as `E1M1` or `MAP01`.
    pub fn from_name(level_name: &str) -> Option<Self> {
        let upper = level_name.trim().to_ascii_uppercase();
        if let Some(rest) = upper.strip_prefix('E') {
            if let Some(mid) = rest.find('M') {
                let episode = rest[..mid].parse::<u8>().ok()?;
                let map = rest[mid + 1..].parse::<u8>().ok()?;
                return Some(Self::new(episode, map));
            }
        }

        if let Some(rest) = upper.strip_prefix("MAP") {
            let map = rest.parse::<u8>().ok()?;
            return Some(Self::doom2(map));
        }

        None
    }

    /// Returns `true` if this is a Doom 2 map.
    pub fn is_doom2(&self) -> bool {
        matches!(self.format, MapFormat::Doom2)
    }

    /// Compute the next map after this one.
    ///
    /// `secret_exit`: if `true`, the player found the secret exit.
    ///
    /// Returns `None` if this is the final map of the episode/game.
    ///
    /// # Doom 1 progression
    /// - E1M1..E1M7 -> E1M2..E1M8 (linear)
    /// - E1M3 + secret -> E1M9
    /// - E1M9 -> E1M4
    /// - E1M8 -> None (episode end)
    /// - Same pattern for E2 and E3
    ///
    /// # Doom 2 progression
    /// - MAP01..MAP29 -> MAP02..MAP30 (linear, except below)
    /// - MAP15 + secret -> MAP31
    /// - MAP31 + secret -> MAP32
    /// - MAP31 normal -> MAP16
    /// - MAP32 normal -> MAP16
    /// - MAP30 -> None (game end)
    pub fn next_map(&self, secret_exit: bool) -> Option<MapId> {
        if self.is_doom2() {
            return self.next_map_doom2(secret_exit);
        }

        // Doom 1: episode 1-3, maps 1-9
        if self.episode >= 1 && self.episode <= 3 {
            return self.next_map_doom1(secret_exit);
        }

        None
    }

    /// Next map for Doom 1 (ExMy format).
    fn next_map_doom1(&self, secret_exit: bool) -> Option<MapId> {
        let ep = self.episode;
        let map = self.map;

        // Secret exits: ExM3 -> ExM9
        if secret_exit && map == 3 {
            return Some(MapId::new(ep, 9));
        }

        // ExM9 (secret level) always returns to ExM4
        if map == 9 {
            return Some(MapId::new(ep, 4));
        }

        // ExM8 is the episode finale
        if map == 8 {
            return None;
        }

        // Normal linear progression: ExMn -> ExM(n+1)
        if (1..=7).contains(&map) {
            return Some(MapId::new(ep, map + 1));
        }

        None
    }

    /// Next map for Doom 2 (MAPxx format).
    fn next_map_doom2(&self, secret_exit: bool) -> Option<MapId> {
        let map = self.map;

        // MAP30 is the final map
        if map == 30 {
            return None;
        }

        // MAP15 + secret -> MAP31
        if map == 15 && secret_exit {
            return Some(MapId::doom2(31));
        }

        // MAP31 + secret -> MAP32
        if map == 31 && secret_exit {
            return Some(MapId::doom2(32));
        }

        // MAP31 normal -> MAP16
        if map == 31 {
            return Some(MapId::doom2(16));
        }

        // MAP32 -> MAP16
        if map == 32 {
            return Some(MapId::doom2(16));
        }

        // Normal linear progression
        if (1..=29).contains(&map) {
            return Some(MapId::doom2(map + 1));
        }

        None
    }

    /// Returns `true` if this is the final map of the episode (Doom 1) or game (Doom 2).
    pub fn is_final_map(&self) -> bool {
        if self.is_doom2() {
            return self.map == 30;
        }
        // Doom 1: ExM8 ends the episode
        self.map == 8
    }

    /// Look up the par time for this map (in tics, 35 tics = 1 second).
    ///
    /// Delegates to `intermission::par_time` using the map name.
    pub fn par_time(&self) -> u32 {
        intermission::par_time(&self.map_name())
    }

    /// Return the canonical map name string (e.g., "E1M1" or "MAP01").
    pub fn map_name(&self) -> String {
        if self.is_doom2() {
            format!("MAP{:02}", self.map)
        } else {
            format!("E{}M{}", self.episode, self.map)
        }
    }
}

// ---------------------------------------------------------------------------
// GamePhase
// ---------------------------------------------------------------------------

/// Top-level game phase.
///
/// The game cycles through these phases:
/// `TitleScreen` -> `Playing` -> `Intermission` -> `Playing` ...
/// When the final map is completed, `Intermission` transitions to `Finale`,
/// and `Finale` transitions back to `TitleScreen`.
#[derive(Debug, Clone)]
pub enum GamePhase {
    /// Title screen / demo playback.
    TitleScreen,
    /// Active gameplay.
    Playing,
    /// Intermission tally screen between levels.
    Intermission {
        /// End-of-level statistics.
        stats: IntermissionStats,
        /// The map the player will advance to next.
        next_map: MapId,
    },
    /// End-of-episode/game text crawl.
    Finale {
        /// Index into the finale text (characters revealed so far).
        text_index: usize,
        /// Tic counter for the finale sequence.
        tic: u32,
    },
}

// ---------------------------------------------------------------------------
// GamePhaseController
// ---------------------------------------------------------------------------

/// Tic threshold before intermission auto-advances (about 10 seconds).
const INTERMISSION_AUTO_ADVANCE_TICS: u32 = 350;

/// Tic threshold before finale auto-advances to title screen (about 15 seconds).
const FINALE_AUTO_ADVANCE_TICS: u32 = 525;

/// Drives the top-level game phase state machine.
///
/// Each game tic, the caller invokes `tick()` which inspects `GameState`
/// for exit requests and manages phase transitions.
#[derive(Debug, Clone)]
pub struct GamePhaseController {
    /// Current game phase.
    phase: GamePhase,
    /// Current map the player is on (or was on, during intermission).
    current_map: MapId,
    /// Tic counter for the current phase (reset on phase transitions).
    phase_tic: u32,
    /// Set by `request_skip()` — the player wants to skip intermission/finale.
    skip_requested: bool,
    /// When a new map needs loading, this is set to `Some(map_id)`.
    /// The caller reads it via `should_load_map()` and clears it via `clear_load_request()`.
    pending_load: Option<MapId>,
}

impl GamePhaseController {
    /// Create a new controller starting in the `Playing` phase on `start_map`.
    pub fn new(start_map: MapId) -> Self {
        Self {
            phase: GamePhase::Playing,
            current_map: start_map,
            phase_tic: 0,
            skip_requested: false,
            pending_load: None,
        }
    }

    /// Create a new controller starting at the title screen.
    pub fn new_at_title() -> Self {
        Self {
            phase: GamePhase::TitleScreen,
            current_map: MapId::new(1, 1),
            phase_tic: 0,
            skip_requested: false,
            pending_load: None,
        }
    }

    /// Return a reference to the current game phase.
    pub fn phase(&self) -> &GamePhase {
        &self.phase
    }

    /// Return the current (or most recent) map.
    pub fn current_map(&self) -> MapId {
        self.current_map
    }

    /// Return the phase tic counter.
    pub fn phase_tic(&self) -> u32 {
        self.phase_tic
    }

    /// The main driver: advance the phase state machine by one tic.
    ///
    /// In `Playing`: checks `game_state.exit_request` and transitions to
    /// `Intermission` (or `Finale` for final maps) when an exit is triggered.
    ///
    /// In `Intermission`: increments `phase_tic`. After a threshold or
    /// player skip, transitions to `Playing` (setting `pending_load`).
    ///
    /// In `Finale`: increments `phase_tic`. After a threshold or skip,
    /// transitions to `TitleScreen`.
    ///
    /// In `TitleScreen`: waits for external input (via `advance_to_playing`).
    pub fn tick(&mut self, game_state: &mut GameState) {
        self.phase_tic += 1;

        match &self.phase {
            GamePhase::Playing => {
                self.tick_playing(game_state);
            }
            GamePhase::Intermission { .. } => {
                self.tick_intermission();
            }
            GamePhase::Finale { .. } => {
                self.tick_finale();
            }
            GamePhase::TitleScreen => {
                // Title screen just waits; advance_to_playing() transitions out.
            }
        }
    }

    /// Handle the `Playing` phase: check for exit requests.
    fn tick_playing(&mut self, game_state: &mut GameState) {
        if let Some(exit_req) = game_state.exit_request.take() {
            let secret_exit = exit_req == ExitRequest::Secret;
            let stats = game_state.compute_intermission_stats();

            if self.current_map.is_final_map() && !secret_exit {
                // Final map -> Finale
                self.phase = GamePhase::Finale {
                    text_index: 0,
                    tic: 0,
                };
                self.phase_tic = 0;
                self.skip_requested = false;
            } else if let Some(next) = self.current_map.next_map(secret_exit) {
                // Normal/secret exit -> Intermission
                self.phase = GamePhase::Intermission {
                    stats,
                    next_map: next,
                };
                self.phase_tic = 0;
                self.skip_requested = false;
            } else {
                // No next map (shouldn't happen if is_final_map is correct,
                // but handle gracefully) -> Finale
                self.phase = GamePhase::Finale {
                    text_index: 0,
                    tic: 0,
                };
                self.phase_tic = 0;
                self.skip_requested = false;
            }
        }
    }

    /// Handle the `Intermission` phase: auto-advance or skip.
    fn tick_intermission(&mut self) {
        if self.skip_requested || self.phase_tic >= INTERMISSION_AUTO_ADVANCE_TICS {
            // Extract next_map before transitioning
            let next_map = match &self.phase {
                GamePhase::Intermission { next_map, .. } => *next_map,
                _ => unreachable!(),
            };
            self.current_map = next_map;
            self.pending_load = Some(next_map);
            self.phase = GamePhase::Playing;
            self.phase_tic = 0;
            self.skip_requested = false;
        }
    }

    /// Handle the `Finale` phase: auto-advance or skip.
    fn tick_finale(&mut self) {
        // Update the finale tic counter inside the enum
        if let GamePhase::Finale { tic, text_index } = &mut self.phase {
            *tic += 1;
            // Reveal one character every 3 tics
            if *tic % 3 == 0 {
                *text_index += 1;
            }
        }

        if self.skip_requested || self.phase_tic >= FINALE_AUTO_ADVANCE_TICS {
            self.phase = GamePhase::TitleScreen;
            self.phase_tic = 0;
            self.skip_requested = false;
        }
    }

    /// Transition from `Intermission` or `TitleScreen` to `Playing`.
    ///
    /// When called from `TitleScreen`, sets `pending_load` to `current_map`
    /// so the caller knows to load that map.
    pub fn advance_to_playing(&mut self) {
        match &self.phase {
            GamePhase::TitleScreen => {
                self.pending_load = Some(self.current_map);
                self.phase = GamePhase::Playing;
                self.phase_tic = 0;
                self.skip_requested = false;
            }
            GamePhase::Intermission { next_map, .. } => {
                let next = *next_map;
                self.current_map = next;
                self.pending_load = Some(next);
                self.phase = GamePhase::Playing;
                self.phase_tic = 0;
                self.skip_requested = false;
            }
            _ => {
                // Already playing or in finale — no-op.
            }
        }
    }

    /// Signal that the player pressed Use/Fire during intermission or finale.
    pub fn request_skip(&mut self) {
        self.skip_requested = true;
    }

    /// Returns `Some(map_id)` when a new map needs to be loaded.
    ///
    /// The caller should load the map and then call `clear_load_request()`.
    pub fn should_load_map(&self) -> Option<MapId> {
        self.pending_load
    }

    /// Clear the pending load request after the map has been loaded.
    pub fn clear_load_request(&mut self) {
        self.pending_load = None;
    }

    /// Start a new game on the given map.
    ///
    /// Resets the controller to `Playing` phase on the given map.
    pub fn start_new_game(&mut self, map: MapId) {
        self.current_map = map;
        self.phase = GamePhase::Playing;
        self.phase_tic = 0;
        self.skip_requested = false;
        self.pending_load = Some(map);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::Mobj;
    use crate::player::PlayerState;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    // =======================================================================
    // MapId tests
    // =======================================================================

    // --- next_map: Doom 1 Episode 1 ---

    #[test]
    fn next_map_e1m1_normal() {
        let id = MapId::new(1, 1);
        assert_eq!(id.next_map(false), Some(MapId::new(1, 2)));
    }

    #[test]
    fn next_map_e1m2_normal() {
        let id = MapId::new(1, 2);
        assert_eq!(id.next_map(false), Some(MapId::new(1, 3)));
    }

    #[test]
    fn next_map_e1m3_normal() {
        let id = MapId::new(1, 3);
        assert_eq!(id.next_map(false), Some(MapId::new(1, 4)));
    }

    #[test]
    fn next_map_e1m3_secret() {
        let id = MapId::new(1, 3);
        assert_eq!(id.next_map(true), Some(MapId::new(1, 9)));
    }

    #[test]
    fn next_map_e1m4_normal() {
        let id = MapId::new(1, 4);
        assert_eq!(id.next_map(false), Some(MapId::new(1, 5)));
    }

    #[test]
    fn next_map_e1m5_normal() {
        let id = MapId::new(1, 5);
        assert_eq!(id.next_map(false), Some(MapId::new(1, 6)));
    }

    #[test]
    fn next_map_e1m6_normal() {
        let id = MapId::new(1, 6);
        assert_eq!(id.next_map(false), Some(MapId::new(1, 7)));
    }

    #[test]
    fn next_map_e1m7_normal() {
        let id = MapId::new(1, 7);
        assert_eq!(id.next_map(false), Some(MapId::new(1, 8)));
    }

    #[test]
    fn next_map_e1m8_ends_episode() {
        let id = MapId::new(1, 8);
        assert_eq!(id.next_map(false), None);
    }

    #[test]
    fn next_map_e1m9_returns_to_e1m4() {
        let id = MapId::new(1, 9);
        assert_eq!(id.next_map(false), Some(MapId::new(1, 4)));
    }

    // --- next_map: Doom 1 Episode 2 ---

    #[test]
    fn next_map_e2m1_normal() {
        let id = MapId::new(2, 1);
        assert_eq!(id.next_map(false), Some(MapId::new(2, 2)));
    }

    #[test]
    fn next_map_e2m3_secret() {
        let id = MapId::new(2, 3);
        assert_eq!(id.next_map(true), Some(MapId::new(2, 9)));
    }

    #[test]
    fn next_map_e2m8_ends_episode() {
        let id = MapId::new(2, 8);
        assert_eq!(id.next_map(false), None);
    }

    #[test]
    fn next_map_e2m9_returns_to_e2m4() {
        let id = MapId::new(2, 9);
        assert_eq!(id.next_map(false), Some(MapId::new(2, 4)));
    }

    // --- next_map: Doom 1 Episode 3 ---

    #[test]
    fn next_map_e3m1_normal() {
        let id = MapId::new(3, 1);
        assert_eq!(id.next_map(false), Some(MapId::new(3, 2)));
    }

    #[test]
    fn next_map_e3m3_secret() {
        let id = MapId::new(3, 3);
        assert_eq!(id.next_map(true), Some(MapId::new(3, 9)));
    }

    #[test]
    fn next_map_e3m8_ends_episode() {
        let id = MapId::new(3, 8);
        assert_eq!(id.next_map(false), None);
    }

    #[test]
    fn next_map_e3m9_returns_to_e3m4() {
        let id = MapId::new(3, 9);
        assert_eq!(id.next_map(false), Some(MapId::new(3, 4)));
    }

    // --- next_map: Doom 2 ---

    #[test]
    fn next_map_map01_normal() {
        let id = MapId::doom2(1);
        assert_eq!(id.next_map(false), Some(MapId::doom2(2)));
    }

    #[test]
    fn next_map_map14_normal() {
        let id = MapId::doom2(14);
        assert_eq!(id.next_map(false), Some(MapId::doom2(15)));
    }

    #[test]
    fn next_map_map15_normal() {
        let id = MapId::doom2(15);
        assert_eq!(id.next_map(false), Some(MapId::doom2(16)));
    }

    #[test]
    fn next_map_map15_secret() {
        let id = MapId::doom2(15);
        assert_eq!(id.next_map(true), Some(MapId::doom2(31)));
    }

    #[test]
    fn next_map_map16_normal() {
        let id = MapId::doom2(16);
        assert_eq!(id.next_map(false), Some(MapId::doom2(17)));
    }

    #[test]
    fn next_map_map29_normal() {
        let id = MapId::doom2(29);
        assert_eq!(id.next_map(false), Some(MapId::doom2(30)));
    }

    #[test]
    fn next_map_map30_ends_game() {
        let id = MapId::doom2(30);
        assert_eq!(id.next_map(false), None);
    }

    #[test]
    fn next_map_map31_normal_goes_to_map16() {
        let id = MapId::doom2(31);
        assert_eq!(id.next_map(false), Some(MapId::doom2(16)));
    }

    #[test]
    fn next_map_map31_secret_goes_to_map32() {
        let id = MapId::doom2(31);
        assert_eq!(id.next_map(true), Some(MapId::doom2(32)));
    }

    #[test]
    fn next_map_map32_goes_to_map16() {
        let id = MapId::doom2(32);
        assert_eq!(id.next_map(false), Some(MapId::doom2(16)));
    }

    // --- is_final_map ---

    #[test]
    fn is_final_map_e1m8() {
        assert!(MapId::new(1, 8).is_final_map());
    }

    #[test]
    fn is_final_map_e2m8() {
        assert!(MapId::new(2, 8).is_final_map());
    }

    #[test]
    fn is_final_map_e3m8() {
        assert!(MapId::new(3, 8).is_final_map());
    }

    #[test]
    fn is_final_map_map30() {
        assert!(MapId::doom2(30).is_final_map());
    }

    #[test]
    fn is_not_final_map_e1m1() {
        assert!(!MapId::new(1, 1).is_final_map());
    }

    #[test]
    fn is_not_final_map_e1m9() {
        assert!(!MapId::new(1, 9).is_final_map());
    }

    #[test]
    fn is_not_final_map_map15() {
        assert!(!MapId::doom2(15).is_final_map());
    }

    #[test]
    fn is_not_final_map_map31() {
        assert!(!MapId::doom2(31).is_final_map());
    }

    // --- par_time ---

    #[test]
    fn par_time_e1m1_lookup() {
        assert_eq!(MapId::new(1, 1).par_time(), 30 * 35);
    }

    #[test]
    fn par_time_e1m9_lookup() {
        assert_eq!(MapId::new(1, 9).par_time(), 165 * 35);
    }

    #[test]
    fn par_time_e2m1_lookup() {
        assert_eq!(MapId::new(2, 1).par_time(), 90 * 35);
    }

    #[test]
    fn par_time_map10_lookup() {
        // MAP10 par = 90 seconds
        assert_eq!(MapId::doom2(10).par_time(), 90 * 35);
    }

    #[test]
    fn par_time_map30_lookup() {
        // MAP30 par = 180 seconds
        assert_eq!(MapId::doom2(30).par_time(), 180 * 35);
    }

    // --- map_name ---

    #[test]
    fn map_name_e1m1() {
        assert_eq!(MapId::new(1, 1).map_name(), "E1M1");
    }

    #[test]
    fn map_name_e3m9() {
        assert_eq!(MapId::new(3, 9).map_name(), "E3M9");
    }

    #[test]
    fn map_name_map01() {
        assert_eq!(MapId::doom2(1).map_name(), "MAP01");
    }

    #[test]
    fn map_name_map10() {
        assert_eq!(MapId::doom2(10).map_name(), "MAP10");
    }

    #[test]
    fn map_name_map31() {
        assert_eq!(MapId::doom2(31).map_name(), "MAP31");
    }

    // =======================================================================
    // GamePhaseController tests
    // =======================================================================

    /// Helper: create a GameState with a player for testing.
    fn make_test_game_state(level_name: &str) -> GameState {
        let mut gs = GameState::new(level_name);
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs
    }

    #[test]
    fn controller_starts_in_playing() {
        let ctrl = GamePhaseController::new(MapId::new(1, 1));
        assert!(matches!(ctrl.phase(), GamePhase::Playing));
        assert_eq!(ctrl.current_map(), MapId::new(1, 1));
    }

    #[test]
    fn controller_starts_at_title() {
        let ctrl = GamePhaseController::new_at_title();
        assert!(matches!(ctrl.phase(), GamePhase::TitleScreen));
    }

    #[test]
    fn playing_to_intermission_on_normal_exit() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs);

        assert!(matches!(ctrl.phase(), GamePhase::Intermission { .. }));
        assert!(gs.exit_request.is_none(), "exit_request must be consumed");
    }

    #[test]
    fn playing_to_intermission_on_secret_exit() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 3));
        let mut gs = make_test_game_state("E1M3");
        gs.exit_request = Some(ExitRequest::Secret);

        ctrl.tick(&mut gs);

        match ctrl.phase() {
            GamePhase::Intermission { next_map, .. } => {
                assert_eq!(*next_map, MapId::new(1, 9));
            }
            _ => panic!("expected Intermission phase"),
        }
    }

    #[test]
    fn intermission_next_map_is_correct() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs);

        match ctrl.phase() {
            GamePhase::Intermission { next_map, .. } => {
                assert_eq!(*next_map, MapId::new(1, 2));
            }
            _ => panic!("expected Intermission phase"),
        }
    }

    #[test]
    fn intermission_stats_are_captured() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.player.kill_count = 10;
        gs.stats.total_kills = 20;
        gs.stats.level_time = 350;
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs);

        match ctrl.phase() {
            GamePhase::Intermission { stats, .. } => {
                assert_eq!(stats.kills, 10);
                assert_eq!(stats.total_kills, 20);
                assert_eq!(stats.time_tics, 350);
            }
            _ => panic!("expected Intermission"),
        }
    }

    #[test]
    fn phase_tic_increments_during_intermission() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs); // transitions to Intermission, phase_tic = 0
        let tic_after_transition = ctrl.phase_tic();

        ctrl.tick(&mut gs); // phase_tic = 1
        assert!(ctrl.phase_tic() > tic_after_transition);
    }

    #[test]
    fn skip_during_intermission_advances_to_playing() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs); // -> Intermission
        assert!(matches!(ctrl.phase(), GamePhase::Intermission { .. }));

        ctrl.request_skip();
        ctrl.tick(&mut gs); // should advance to Playing

        assert!(matches!(ctrl.phase(), GamePhase::Playing));
        assert_eq!(ctrl.current_map(), MapId::new(1, 2));
    }

    #[test]
    fn should_load_map_after_intermission_skip() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs); // -> Intermission
        ctrl.request_skip();
        ctrl.tick(&mut gs); // -> Playing

        assert_eq!(ctrl.should_load_map(), Some(MapId::new(1, 2)));
    }

    #[test]
    fn clear_load_request() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs);
        ctrl.request_skip();
        ctrl.tick(&mut gs);

        assert!(ctrl.should_load_map().is_some());
        ctrl.clear_load_request();
        assert!(ctrl.should_load_map().is_none());
    }

    #[test]
    fn final_map_goes_to_finale() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 8));
        let mut gs = make_test_game_state("E1M8");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs);

        assert!(matches!(ctrl.phase(), GamePhase::Finale { .. }));
    }

    #[test]
    fn finale_transitions_to_title_screen_on_skip() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 8));
        let mut gs = make_test_game_state("E1M8");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs); // -> Finale
        assert!(matches!(ctrl.phase(), GamePhase::Finale { .. }));

        ctrl.request_skip();
        ctrl.tick(&mut gs); // -> TitleScreen

        assert!(matches!(ctrl.phase(), GamePhase::TitleScreen));
    }

    #[test]
    fn title_screen_advance_to_playing() {
        let mut ctrl = GamePhaseController::new_at_title();
        ctrl.advance_to_playing();

        assert!(matches!(ctrl.phase(), GamePhase::Playing));
        assert!(ctrl.should_load_map().is_some());
    }

    #[test]
    fn start_new_game() {
        let mut ctrl = GamePhaseController::new_at_title();
        ctrl.start_new_game(MapId::new(2, 1));

        assert!(matches!(ctrl.phase(), GamePhase::Playing));
        assert_eq!(ctrl.current_map(), MapId::new(2, 1));
        assert_eq!(ctrl.should_load_map(), Some(MapId::new(2, 1)));
    }

    #[test]
    fn exit_request_consumed_after_transition() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs);

        assert!(gs.exit_request.is_none());
    }

    #[test]
    fn no_exit_request_stays_in_playing() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");

        ctrl.tick(&mut gs);
        ctrl.tick(&mut gs);
        ctrl.tick(&mut gs);

        assert!(matches!(ctrl.phase(), GamePhase::Playing));
    }

    #[test]
    fn intermission_auto_advances_after_threshold() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs); // -> Intermission, phase_tic = 0

        // Tick until auto-advance threshold
        for _ in 0..INTERMISSION_AUTO_ADVANCE_TICS {
            ctrl.tick(&mut gs);
        }

        assert!(
            matches!(ctrl.phase(), GamePhase::Playing),
            "should auto-advance to Playing after threshold"
        );
    }

    #[test]
    fn finale_auto_advances_after_threshold() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 8));
        let mut gs = make_test_game_state("E1M8");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs); // -> Finale, phase_tic = 0

        for _ in 0..FINALE_AUTO_ADVANCE_TICS {
            ctrl.tick(&mut gs);
        }

        assert!(
            matches!(ctrl.phase(), GamePhase::TitleScreen),
            "should auto-advance to TitleScreen after threshold"
        );
    }

    #[test]
    fn finale_text_index_increments() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 8));
        let mut gs = make_test_game_state("E1M8");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs); // -> Finale

        // Tick a few times to see text_index increment
        for _ in 0..9 {
            ctrl.tick(&mut gs);
        }

        match ctrl.phase() {
            GamePhase::Finale { text_index, .. } => {
                assert!(*text_index > 0, "text_index should have incremented");
            }
            _ => panic!("expected Finale"),
        }
    }

    #[test]
    fn doom2_map30_goes_to_finale() {
        let mut ctrl = GamePhaseController::new(MapId::doom2(30));
        let mut gs = make_test_game_state("MAP30");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs);

        assert!(matches!(ctrl.phase(), GamePhase::Finale { .. }));
    }

    #[test]
    fn doom2_map15_secret_to_map31() {
        let mut ctrl = GamePhaseController::new(MapId::doom2(15));
        let mut gs = make_test_game_state("MAP15");
        gs.exit_request = Some(ExitRequest::Secret);

        ctrl.tick(&mut gs);

        match ctrl.phase() {
            GamePhase::Intermission { next_map, .. } => {
                assert_eq!(*next_map, MapId::doom2(31));
            }
            _ => panic!("expected Intermission"),
        }
    }

    #[test]
    fn doom2_map01_normal_exit_goes_to_map02() {
        let mut ctrl = GamePhaseController::new(MapId::doom2(1));
        let mut gs = make_test_game_state("MAP01");
        gs.exit_request = Some(ExitRequest::Normal);

        ctrl.tick(&mut gs);

        match ctrl.phase() {
            GamePhase::Intermission { next_map, .. } => {
                assert_eq!(*next_map, MapId::doom2(2));
            }
            _ => panic!("expected Intermission"),
        }
    }

    #[test]
    fn phase_tic_resets_on_transition() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");

        // Tick a few times in Playing
        for _ in 0..5 {
            ctrl.tick(&mut gs);
        }

        gs.exit_request = Some(ExitRequest::Normal);
        ctrl.tick(&mut gs); // -> Intermission

        // phase_tic should be reset (it's 0, then incremented to 1 on next tick)
        // After the transition tick, phase_tic was reset to 0 then incremented by 1
        // Actually: tick increments first, then checks exit. So phase_tic is some value,
        // but then reset to 0 during transition. Let's verify it resets.
        assert_eq!(ctrl.phase_tic(), 0);
    }

    #[test]
    fn secret_exit_on_non_secret_map_is_normal() {
        // Secret exit on E1M1 (which doesn't have a special secret exit)
        // should just go to E1M2 (same as normal, since only E1M3 has secret routing)
        let id = MapId::new(1, 1);
        assert_eq!(id.next_map(true), Some(MapId::new(1, 2)));
    }

    #[test]
    fn e1m8_secret_exit_still_goes_to_secret() {
        // E1M8 with secret exit — no secret route from E1M8
        // but is_final_map is true and secret_exit bypasses finale
        let mut ctrl = GamePhaseController::new(MapId::new(1, 8));
        let mut gs = make_test_game_state("E1M8");
        gs.exit_request = Some(ExitRequest::Secret);

        ctrl.tick(&mut gs);

        // E1M8 is final map, but secret_exit=true, so next_map_doom1 returns None
        // (map == 8 -> None regardless of secret). So it should go to Finale.
        // Actually wait: is_final_map check has `!secret_exit` guard.
        // Since secret_exit is true, it won't go to Finale.
        // Instead, next_map(true) for E1M8 returns None (map==8 -> None).
        // So the else branch fires: no next map -> Finale.
        assert!(matches!(ctrl.phase(), GamePhase::Finale { .. }));
    }

    #[test]
    fn multiple_level_transitions() {
        // Play through E1M1 -> E1M2 -> E1M3
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");

        // Exit E1M1
        gs.exit_request = Some(ExitRequest::Normal);
        ctrl.tick(&mut gs);
        assert!(matches!(ctrl.phase(), GamePhase::Intermission { .. }));
        ctrl.request_skip();
        ctrl.tick(&mut gs);
        assert!(matches!(ctrl.phase(), GamePhase::Playing));
        assert_eq!(ctrl.current_map(), MapId::new(1, 2));
        ctrl.clear_load_request();

        // Exit E1M2
        gs.exit_request = Some(ExitRequest::Normal);
        ctrl.tick(&mut gs);
        assert!(matches!(ctrl.phase(), GamePhase::Intermission { .. }));
        ctrl.request_skip();
        ctrl.tick(&mut gs);
        assert!(matches!(ctrl.phase(), GamePhase::Playing));
        assert_eq!(ctrl.current_map(), MapId::new(1, 3));
    }

    #[test]
    fn advance_to_playing_from_intermission() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        let mut gs = make_test_game_state("E1M1");
        gs.exit_request = Some(ExitRequest::Normal);
        ctrl.tick(&mut gs); // -> Intermission

        ctrl.advance_to_playing();

        assert!(matches!(ctrl.phase(), GamePhase::Playing));
        assert_eq!(ctrl.current_map(), MapId::new(1, 2));
        assert_eq!(ctrl.should_load_map(), Some(MapId::new(1, 2)));
    }

    #[test]
    fn advance_to_playing_noop_when_already_playing() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));
        ctrl.advance_to_playing(); // should be a no-op

        assert!(matches!(ctrl.phase(), GamePhase::Playing));
        assert!(ctrl.should_load_map().is_none());
    }

    #[test]
    fn map_id_equality() {
        assert_eq!(MapId::new(1, 1), MapId::new(1, 1));
        assert_ne!(MapId::new(1, 1), MapId::new(1, 2));
        assert_ne!(MapId::new(1, 1), MapId::new(2, 1));
        assert_ne!(MapId::new(1, 1), MapId::doom2(1));
    }

    #[test]
    fn map_id_copy() {
        let a = MapId::new(1, 1);
        let b = a; // Copy
        assert_eq!(a, b);
    }

    #[test]
    fn doom2_linear_progression_map20_to_map21() {
        let id = MapId::doom2(20);
        assert_eq!(id.next_map(false), Some(MapId::doom2(21)));
    }

    #[test]
    fn doom2_map32_secret_exit_still_goes_to_map16() {
        // MAP32 doesn't have a further secret — secret_exit is ignored
        let id = MapId::doom2(32);
        assert_eq!(id.next_map(true), Some(MapId::doom2(16)));
    }

    #[test]
    fn parse_map_name_distinguishes_doom1_from_doom2() {
        assert_eq!(MapId::from_name("E1M1"), Some(MapId::new(1, 1)));
        assert_eq!(MapId::from_name("MAP01"), Some(MapId::doom2(1)));
        assert_eq!(MapId::from_name("map09"), Some(MapId::doom2(9)));
    }

    #[test]
    #[should_panic(expected = "internal error: entered unreachable code")]
    fn tick_intermission_unreachable_panic() {
        let mut ctrl = GamePhaseController::new(MapId::new(1, 1));

        // Force state into Playing while skip is requested.
        // This triggers the first branch of tick_intermission but fails the match
        ctrl.phase = GamePhase::Playing;
        ctrl.skip_requested = true;
        ctrl.tick_intermission();
    }
}
