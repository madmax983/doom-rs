//! Demo recording and playback wrappers for `DoomApp`.
//!
//! - [`DemoRecordingWrapper`] — wraps a [`DoomGame`], captures each [`TicCmd`],
//!   and writes an LMP file on `Drop`.
//! - [`DemoPlaybackApp`] — wraps a [`DoomGame`] and replays a pre-recorded
//!   [`DemoPlayer`] instead of consuming live keyboard input.

use doom_demo::{DemoPlayer, DemoRecorder};
use doom_renderer::Framebuffer;
use doom_tui::{DoomApp, TicInput};
use doom_types::CompatibilityProfile;
use doom_types::TicCmd;

use crate::DoomGame;
use crate::net_mode::ticinput_to_ticcmd;

// ---------------------------------------------------------------------------
// DemoRecordingWrapper
// ---------------------------------------------------------------------------

/// Wraps [`DoomGame`] and records each tic's [`TicCmd`] to a [`DemoRecorder`].
///
/// When the wrapper is dropped the accumulated demo is written to `save_path`.
///
/// # Examples
/// ```no_run
/// # use doom_app::demo_mode::DemoRecordingWrapper;
/// # use doom_app::DoomGame;
/// # use doom_demo::{DemoRecorder, LmpHeader};
/// # use doom_tui::TicInput;
/// # use doom_tui::DoomApp;
/// # let doom_game: DoomGame = unsafe { std::mem::zeroed() };
/// # let dir = std::env::temp_dir();
/// # let path = dir.join("my_demo.lmp");
/// let header = LmpHeader::new_singleplayer(3, 1, 1);
/// let recorder = DemoRecorder::new(header);
/// let mut wrapper = DemoRecordingWrapper::new(
///     doom_game,
///     recorder,
///     path.clone(),
/// );
///
/// // Pump the event loop...
/// wrapper.tick(TicInput::default());
///
/// // my_demo.lmp is written to disk when `wrapper` goes out of scope.
/// ```
pub(crate) struct DemoRecordingWrapper {
    inner: DoomGame,
    recorder: DemoRecorder,
    save_path: std::path::PathBuf,
    compat: CompatibilityProfile,
}

impl DemoRecordingWrapper {
    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> &DoomGame {
        &self.inner
    }
    /// Create a new recording wrapper.
    #[allow(dead_code)]
    pub(crate) fn new(
        inner: DoomGame,
        recorder: DemoRecorder,
        save_path: std::path::PathBuf,
    ) -> Self {
        Self::new_with_compat(inner, recorder, save_path, CompatibilityProfile::Extended)
    }

    /// Create a new recording wrapper with an explicit compatibility profile.
    pub(crate) fn new_with_compat(
        inner: DoomGame,
        recorder: DemoRecorder,
        save_path: std::path::PathBuf,
        compat: CompatibilityProfile,
    ) -> Self {
        Self {
            inner,
            recorder,
            save_path,
            compat,
        }
    }

    /// Return the compatibility profile used when constructing the wrapper.
    #[allow(dead_code)]
    pub(crate) const fn compat_profile(&self) -> CompatibilityProfile {
        self.compat
    }
}

impl DoomApp for DemoRecordingWrapper {
    fn tick(&mut self, input: TicInput) {
        match self.compat {
            CompatibilityProfile::Extended | CompatibilityProfile::VanillaStrict => {
                // Demo behavior is intentionally shared across profiles for now.
                // The profile is still threaded here so the seam stays explicit.
                let cmd = ticinput_to_ticcmd(input);
                self.recorder.record_tic(&cmd);
                self.inner.tick(input);
            }
        }
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        self.inner.render(fb);
    }

    fn active_palette(&self) -> usize {
        self.inner.active_palette()
    }
}

impl Drop for DemoRecordingWrapper {
    fn drop(&mut self) {
        // Serialize the demo to bytes, then write atomically.
        let bytes = match self.recorder.to_bytes() {
            Ok(b) => b,
            Err(e) => {
                log::error!("Failed to serialise demo: {e}");
                return;
            }
        };
        if let Err(e) = std::fs::write(&self.save_path, &bytes) {
            log::error!("Failed to write demo '{}': {e}", self.save_path.display());
        }
    }
}

// ---------------------------------------------------------------------------
// DemoPlaybackApp
// ---------------------------------------------------------------------------

/// Wraps [`DoomGame`] and replays a [`DemoPlayer`], ignoring live input.
///
/// When the demo is exhausted the last rendered frame stays frozen until the
/// user quits (Q/Esc via the event loop).
///
/// # Examples
/// ```no_run
/// # use doom_app::demo_mode::DemoPlaybackApp;
/// # use doom_app::DoomGame;
/// # use doom_demo::DemoPlayer;
/// # use doom_tui::TicInput;
/// # use doom_tui::DoomApp;
/// # let doom_game: DoomGame = unsafe { std::mem::zeroed() };
/// let demo_bytes = std::fs::read("my_demo.lmp").unwrap();
/// let player = DemoPlayer::parse(&demo_bytes).unwrap();
/// let mut app = DemoPlaybackApp::new(doom_game, player);
///
/// // The game will ignore `TicInput` and use the recorded demo ticks instead.
/// app.tick(TicInput::default());
/// ```
pub(crate) struct DemoPlaybackApp {
    inner: DoomGame,
    player: DemoPlayer,
    compat: CompatibilityProfile,
}

impl DemoPlaybackApp {
    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> &DoomGame {
        &self.inner
    }
    /// Create a new playback app backed by `inner` and `player`.
    #[allow(dead_code)]
    pub(crate) fn new(inner: DoomGame, player: DemoPlayer) -> Self {
        Self::new_with_compat(inner, player, CompatibilityProfile::Extended)
    }

    /// Create a new playback app with an explicit compatibility profile.
    pub(crate) fn new_with_compat(
        inner: DoomGame,
        player: DemoPlayer,
        compat: CompatibilityProfile,
    ) -> Self {
        Self {
            inner,
            player,
            compat,
        }
    }

    /// Return the compatibility profile used when constructing the wrapper.
    #[allow(dead_code)]
    pub(crate) const fn compat_profile(&self) -> CompatibilityProfile {
        self.compat
    }

    /// Feed a single [`TicCmd`] directly to the inner game state, bypassing the
    /// higher-level `DoomGame::tick` path (which would consume a live `TicInput`
    /// and process cheats/saves).
    fn tick_cmd(&mut self, cmd: TicCmd) {
        self.inner.gs.tick(cmd, Some(&mut self.inner.level));
        let status = if self.inner.gs.player.is_dead() {
            super::PlayerStatus::Dead
        } else {
            super::PlayerStatus::Alive
        };
        self.inner.player_view_height =
            super::next_player_view_height(self.inner.player_view_height, status);
        self.inner.tick_weapon_anim();
    }

    /// Returns `true` if the demo has been completely replayed.
    pub(crate) fn is_finished(&self) -> bool {
        self.player.is_finished()
    }
}

impl DoomApp for DemoPlaybackApp {
    fn tick(&mut self, _live_input: TicInput) {
        match self.compat {
            CompatibilityProfile::Extended | CompatibilityProfile::VanillaStrict => {
                // Demo behavior is intentionally shared across profiles for now.
                // The profile is still threaded here so the seam stays explicit.
                if let Some(cmd) = self.player.next_tic() {
                    self.tick_cmd(cmd);
                }
            }
        }
        // Demo exhausted: last frame stays frozen — nothing to do.
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        self.inner.render(fb);
    }

    fn active_palette(&self) -> usize {
        self.inner.active_palette()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal [`Level`] suitable for unit tests.
///
/// Uses a 1×1 blockmap with one sector and no geometry. Mirrors the pattern
/// used in doom-game's movement and specials tests.
#[cfg(test)]
fn make_test_level() -> doom_map::Level {
    use doom_map::{Blockmap, Level, Reject, Sector};

    // 1×1 blockmap with one empty block.
    let mut bm_data = vec![0u8; 14];
    bm_data[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count
    bm_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count
    bm_data[8..10].copy_from_slice(&5u16.to_le_bytes()); // offset[0]
    bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes()); // sentinel
    bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
    let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

    let reject = Reject::parse_lump(&[0u8], 1).expect("reject parse");

    Level {
        name: "TEST".to_owned(),
        things: vec![],
        linedefs: vec![],
        sidedefs: vec![],
        vertexes: vec![],
        segs: vec![],
        ssectors: vec![],
        nodes: vec![],
        sectors: vec![Sector {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_demo::{DemoRecorder, LmpHeader};
    use doom_game::{GameState, Mobj, PlayerState, flags};
    use doom_types::CompatibilityProfile;
    use doom_types::TicCmd;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};
    use std::env;
    use std::sync::Once;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_game_state() -> GameState {
        let mut gs = GameState::new("E1M1");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        mo.radius = Fixed16_16::from_int(16);
        mo.height = Fixed16_16::from_int(56);
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs
    }

    fn make_doom_game() -> DoomGame {
        DoomGame::new(
            make_game_state(),
            make_test_level(),
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            std::collections::HashMap::new(),
        )
    }

    fn make_recorder(n_tics: usize) -> DemoRecorder {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);
        for _ in 0..n_tics {
            rec.record_tic(&TicCmd::default());
        }
        rec
    }

    fn make_player(n_tics: usize) -> DemoPlayer {
        let bytes = {
            let header = LmpHeader::new_singleplayer(3, 1, 1);
            let mut rec = DemoRecorder::new(header);
            for _ in 0..n_tics {
                rec.record_tic(&TicCmd::default());
            }
            rec.to_bytes().expect("to_bytes must succeed")
        };
        DemoPlayer::parse(&bytes).expect("parse must succeed")
    }

    fn make_player_from_bytes(bytes: &[u8]) -> DemoPlayer {
        DemoPlayer::parse(bytes).expect("parse must succeed")
    }

    fn init_trig_tables_once() {
        static INIT: Once = Once::new();
        INIT.call_once(|| unsafe {
            Bam::init_trig_tables();
        });
    }

    // -----------------------------------------------------------------------
    // DemoRecordingWrapper tests
    // -----------------------------------------------------------------------

    #[test]
    fn recording_wrapper_writes_on_drop() {
        let dir = env::temp_dir();
        let path = dir.join("doom_rs_test_recording_wrapper.lmp");

        // Remove any leftover file from a previous run.
        let _ = std::fs::remove_file(&path);

        {
            let game = make_doom_game();
            let recorder = make_recorder(0);
            let mut wrapper = DemoRecordingWrapper::new(game, recorder, path.clone());

            // Record 5 tics.
            for _ in 0..5 {
                wrapper.tick(TicInput::default());
            }
            // Drop triggers write.
        }

        // File must exist and be parseable.
        let bytes = std::fs::read(&path).expect("demo file must exist after drop");
        let player = DemoPlayer::parse(&bytes).expect("written demo must be parseable");
        assert_eq!(
            player.tic_count(),
            5,
            "recorded tic count must match ticks delivered"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn recording_wrapper_records_correct_tic_count() {
        let dir = env::temp_dir();
        let path = dir.join("doom_rs_test_tic_count.lmp");
        let _ = std::fs::remove_file(&path);

        {
            let game = make_doom_game();
            let header = LmpHeader::new_singleplayer(3, 1, 1);
            let recorder = DemoRecorder::new(header);
            let mut wrapper = DemoRecordingWrapper::new(game, recorder, path.clone());

            for _ in 0..10 {
                wrapper.tick(TicInput::default());
            }
        }

        let bytes = std::fs::read(&path).expect("demo file must exist");
        let player = DemoPlayer::parse(&bytes).expect("must parse");
        assert_eq!(player.tic_count(), 10);

        let _ = std::fs::remove_file(&path);
    }

    // -----------------------------------------------------------------------
    // DemoPlaybackApp tests
    // -----------------------------------------------------------------------

    #[test]
    fn playback_app_advances_tics() {
        let game = make_doom_game();
        let player = make_player(3);
        let mut app = DemoPlaybackApp::new(game, player);

        // Feed 3 live ticks — the demo has exactly 3 tics.
        for _ in 0..3 {
            app.tick(TicInput::default());
        }

        // The demo player should now be exhausted.
        assert!(
            app.player.is_finished(),
            "demo player must be finished after 3 ticks"
        );
    }

    #[test]
    fn playback_app_ignores_live_input_after_demo_ends() {
        let game = make_doom_game();
        let player = make_player(2);
        let tic_start = game.gs.tic_num;
        let mut app = DemoPlaybackApp::new(game, player);

        // 2 ticks consumed from demo.
        app.tick(TicInput::default());
        app.tick(TicInput::default());

        let tic_after_demo = app.inner.gs.tic_num;
        assert_eq!(tic_after_demo, tic_start + 2);

        // 2 more ticks: demo is finished, so game state must not advance.
        app.tick(TicInput::default());
        app.tick(TicInput::default());

        assert_eq!(
            app.inner.gs.tic_num, tic_after_demo,
            "tic_num must not advance after demo is exhausted"
        );
    }

    #[test]
    fn playback_app_empty_demo() {
        let game = make_doom_game();
        let player = make_player(0);
        let tic_start = game.gs.tic_num;
        let mut app = DemoPlaybackApp::new(game, player);

        // Even the first tick must be a no-op for an empty demo.
        app.tick(TicInput::default());
        assert_eq!(app.inner.gs.tic_num, tic_start);
        assert!(app.player.is_finished());
    }

    #[test]
    fn playback_app_is_finished_evaluates_correctly() {
        let game = make_doom_game();
        let player = make_player(3); // Demo with 3 tics
        let mut app = DemoPlaybackApp::new(game, player);

        assert!(!app.is_finished(), "demo must not be finished initially");

        app.tick(TicInput::default());
        assert!(!app.is_finished(), "demo must not be finished after 1 tic");

        app.tick(TicInput::default());
        app.tick(TicInput::default());
        assert!(
            app.is_finished(),
            "demo must be finished after consuming all 3 tics"
        );
    }

    #[test]
    fn playback_app_preserves_attack_button_into_game_logic() {
        let game = make_doom_game();
        let player = make_player_from_bytes(&[
            109, 3, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 10, 251, 0x12, 0x01, 0x80,
        ]);
        let mut app =
            DemoPlaybackApp::new_with_compat(game, player, CompatibilityProfile::VanillaStrict);

        app.tick(TicInput::default());

        assert!(
            app.inner.gs.player.attack_down,
            "BT_ATTACK must reach the game state during playback"
        );
    }

    #[test]
    fn playback_app_preserves_use_button_into_game_logic() {
        let game = make_doom_game();
        let player = make_player_from_bytes(&[
            109, 3, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 10, 251, 0x12, 0x02, 0x80,
        ]);
        let mut app =
            DemoPlaybackApp::new_with_compat(game, player, CompatibilityProfile::VanillaStrict);

        app.tick(TicInput::default());

        assert!(
            app.inner.gs.player.use_down,
            "BT_USE must reach the game state during playback"
        );
    }

    #[test]
    fn playback_app_preserves_movement_and_turn_expansion_into_game_state() {
        init_trig_tables_once();

        let bytes = [
            109, 3, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 16, 4, 0x34, 0x00, 0x80,
        ];
        let expected_cmd = TicCmd {
            forward_move: 16,
            side_move: 4,
            angle_turn: 0x3400,
            buttons: 0x00,
            ..Default::default()
        };

        let baseline_game = make_doom_game();
        let playback_game = make_doom_game();
        let mut baseline = baseline_game;
        let mut app = DemoPlaybackApp::new_with_compat(
            playback_game,
            DemoPlayer::from_lmp(&bytes).expect("parse must succeed"),
            CompatibilityProfile::VanillaStrict,
        );

        baseline.gs.tick(expected_cmd, Some(&mut baseline.level));
        app.tick(TicInput::default());

        let baseline_mobj = baseline
            .gs
            .mobjslab
            .get(baseline.gs.player.handle)
            .expect("baseline player mobj must exist");
        let playback_mobj = app
            .inner
            .gs
            .mobjslab
            .get(app.inner.gs.player.handle)
            .expect("playback player mobj must exist");

        assert_eq!(app.compat_profile(), CompatibilityProfile::VanillaStrict);
        assert_eq!(playback_mobj.x, baseline_mobj.x);
        assert_eq!(playback_mobj.y, baseline_mobj.y);
        assert_eq!(playback_mobj.angle, baseline_mobj.angle);
        assert_eq!(playback_mobj.momx, baseline_mobj.momx);
        assert_eq!(playback_mobj.momy, baseline_mobj.momy);
    }

    #[test]
    fn recording_wrapper_keeps_compat_profile() {
        let dir = tempfile::tempdir().expect("tempdir");
        let game = make_doom_game();
        let recorder = make_recorder(0);
        let wrapper = DemoRecordingWrapper::new_with_compat(
            game,
            recorder,
            dir.path()
                .join("doom_rs_unused_recording_wrapper_profile.lmp"),
            CompatibilityProfile::VanillaStrict,
        );

        assert_eq!(
            wrapper.compat_profile(),
            CompatibilityProfile::VanillaStrict
        );
    }

    // -----------------------------------------------------------------------
    // parse_warp_episode_map tests (integration with main.rs helper)
    // -----------------------------------------------------------------------

    #[test]
    fn parse_warp_e1m1() {
        assert_eq!(crate::parse_warp_episode_map("E1M1"), (1, 1));
    }

    #[test]
    fn parse_warp_e3m9() {
        assert_eq!(crate::parse_warp_episode_map("E3M9"), (3, 9));
    }

    #[test]
    fn parse_warp_map03() {
        assert_eq!(crate::parse_warp_episode_map("MAP03"), (1, 3));
    }

    #[test]
    fn parse_warp_map30() {
        assert_eq!(crate::parse_warp_episode_map("MAP30"), (1, 30));
    }

    #[test]
    fn parse_warp_fallback() {
        assert_eq!(crate::parse_warp_episode_map("garbage"), (1, 1));
    }

    #[test]
    fn parse_warp_lowercase() {
        assert_eq!(crate::parse_warp_episode_map("e2m4"), (2, 4));
    }
}
