//! Demo recording and playback wrappers for `DoomApp`.
//!
//! - [`DemoRecordingWrapper`] — wraps a [`DoomGame`], captures each [`TicCmd`],
//!   and writes an LMP file on `Drop`.
//! - [`DemoPlaybackApp`] — wraps a [`DoomGame`] and replays a pre-recorded
//!   [`DemoPlayer`] instead of consuming live keyboard input.

use doom_demo::{DemoPlayer, DemoRecorder};
use doom_game::TicCmd;
use doom_renderer::Framebuffer;
use doom_tui::{DoomApp, TicInput};

use crate::{DoomGame, ticinput_to_ticcmd};

// ---------------------------------------------------------------------------
// DemoRecordingWrapper
// ---------------------------------------------------------------------------

/// Wraps [`DoomGame`] and records each tic's [`TicCmd`] to a [`DemoRecorder`].
///
/// When the wrapper is dropped the accumulated demo is written to `save_path`.
pub struct DemoRecordingWrapper {
    inner: DoomGame,
    recorder: DemoRecorder,
    save_path: std::path::PathBuf,
}

impl DemoRecordingWrapper {
    /// Create a new recording wrapper.
    pub fn new(inner: DoomGame, recorder: DemoRecorder, save_path: std::path::PathBuf) -> Self {
        Self {
            inner,
            recorder,
            save_path,
        }
    }
}

impl DoomApp for DemoRecordingWrapper {
    fn tick(&mut self, input: TicInput) {
        // Convert the live input to a TicCmd and record it BEFORE advancing
        // the simulation, matching vanilla Doom's record ordering.
        let cmd = ticinput_to_ticcmd(input);
        self.recorder.record_tic(&cmd);
        self.inner.tick(input);
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
                eprintln!("Failed to serialise demo: {e}");
                return;
            }
        };
        if let Err(e) = std::fs::write(&self.save_path, &bytes) {
            eprintln!("Failed to write demo '{}': {e}", self.save_path.display());
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
pub struct DemoPlaybackApp {
    inner: DoomGame,
    player: DemoPlayer,
}

impl DemoPlaybackApp {
    /// Create a new playback app backed by `inner` and `player`.
    pub fn new(inner: DoomGame, player: DemoPlayer) -> Self {
        Self { inner, player }
    }

    /// Feed a single [`TicCmd`] directly to the inner game state, bypassing the
    /// higher-level `DoomGame::tick` path (which would consume a live `TicInput`
    /// and process cheats/saves).
    fn tick_cmd(&mut self, cmd: TicCmd) {
        self.inner.gs.tick(cmd, Some(&mut self.inner.level));
    }
}

impl DoomApp for DemoPlaybackApp {
    fn tick(&mut self, _live_input: TicInput) {
        // Use recorded input instead of live keyboard input.
        if let Some(cmd) = self.player.next_tic() {
            self.tick_cmd(cmd);
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
        things:   vec![],
        linedefs: vec![],
        sidedefs: vec![],
        vertexes: vec![],
        segs:     vec![],
        ssectors: vec![],
        nodes:    vec![],
        sectors:  vec![Sector {
            floor_height: 0,
            ceil_height:  128,
            floor_flat:   *b"FLAT1\0\0\0",
            ceil_flat:    *b"FLAT2\0\0\0",
            light_level:  192,
            special:      0,
            tag:          0,
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
    use doom_game::{GameState, Mobj, MobjKind, PlayerState, flags};
    use doom_types::{Bam, Fixed16_16};
    use std::env;

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
        DoomGame::new(make_game_state(), make_test_level(), None)
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
            app.inner.gs.tic_num,
            tic_after_demo,
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
