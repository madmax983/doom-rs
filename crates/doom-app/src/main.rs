//! Doom engine entry point.
//!
//! Usage: doom-app --wad doom1.wad [--warp E1M1]

mod audio_system;
mod cheats;
mod console;
mod demo_mode;
mod net_mode;
mod savegame;

use anyhow::{Context, Result};
use clap::Parser;
use std::net::SocketAddr;
use doom_demo::{DemoPlayer, DemoRecorder, LmpHeader};
use doom_game::{GameState, Mobj, MobjKind, TicCmd, flags};
use doom_map::Level;
use doom_renderer::{FlatCache, Framebuffer, PaletteLut, draw_automap, draw_status_bar, render_level};
use doom_tui::{DoomApp, DoomEventLoop, TicInput};
use doom_types::{Bam, Fixed16_16};
use doom_wad::WadFile;

use audio_system::{AudioSystem, music_lump_for_map, weapon_fire_sfx};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "doom-app", about = "Doom engine (doom-rs)")]
struct Args {
    /// Path to IWAD file (doom1.wad, doom2.wad, freedoom1.wad, etc.)
    #[arg(long)]
    wad: std::path::PathBuf,

    /// Map to load (e.g. E1M1, MAP01). Defaults to E1M1.
    #[arg(long, default_value = "E1M1")]
    warp: String,

    /// Skill level 1-5 (1=ITYTD, 2=HNTR, 3=HMP, 4=UV, 5=NM). Defaults to 3.
    #[arg(long, default_value = "3")]
    skill: u8,

    /// Record gameplay to a .lmp demo file (e.g. --record my.lmp)
    #[arg(long)]
    record: Option<std::path::PathBuf>,

    /// Play back a .lmp demo file instead of live input (e.g. --playdemo my.lmp)
    #[arg(long)]
    playdemo: Option<std::path::PathBuf>,

    /// Run as a relay server on this port (e.g. --server 5029).
    /// Players connect to this address.  Mutually exclusive with --connect,
    /// --record, and --playdemo.
    #[arg(long)]
    server: Option<u16>,

    /// Connect to a relay server for netplay (e.g. --connect 127.0.0.1:5029).
    /// Mutually exclusive with --server, --record, and --playdemo.
    #[arg(long)]
    connect: Option<String>,

    /// Player slot for netplay (1-based, e.g. 1 = player 1).  Defaults to 1.
    #[arg(long, default_value = "1")]
    player: u8,

    /// Total number of players in a netplay session (2-4).  Defaults to 2.
    #[arg(long, default_value = "2")]
    num_players: u8,
}

// ---------------------------------------------------------------------------
// DoomGame — implements DoomApp
// ---------------------------------------------------------------------------

pub(crate) struct DoomGame {
    pub(crate) gs: GameState,
    pub(crate) level: Level,
    cheat_detector: cheats::CheatDetector,
    console: console::Console,
    /// Path used for quick save (F5) and quick load (F9).
    save_path: std::path::PathBuf,
    /// Whether the overhead automap is currently displayed instead of first-person view.
    automap_visible: bool,
    /// Whether the IDDT cheat has toggled full automap reveal.
    automap_full_reveal: bool,
    /// Optional audio subsystem.  `None` when no audio device is available.
    audio: Option<AudioSystem>,
    /// Whether the attack button was held during the previous tic (for edge detection).
    prev_attack_down: bool,
    /// Flat texture cache (floor/ceiling textures loaded from the WAD).
    flat_cache: Option<FlatCache>,
}

impl DoomGame {
    fn new(gs: GameState, level: Level, audio: Option<AudioSystem>, flat_cache: Option<FlatCache>) -> Self {
        Self {
            gs,
            level,
            cheat_detector: cheats::CheatDetector::new(),
            console: console::Console::new(),
            save_path: std::path::PathBuf::from("doom_save.bin"),
            automap_visible: false,
            automap_full_reveal: false,
            audio,
            prev_attack_down: false,
            flat_cache,
        }
    }
}

impl DoomApp for DoomGame {
    fn tick(&mut self, input: TicInput) {
        // Handle console / cheat input before forwarding movement to the
        // game simulation.
        if let Some(ch) = input.console_char {
            if ch == '`' || ch == '~' {
                // Toggle the console overlay on backtick/tilde.
                self.console.toggle();
            } else if self.console.visible {
                // Console is open: feed characters to the input line.
                if ch == '\n' {
                    // Enter: submit the line, try to apply as a cheat.
                    let line = self.console.submit();
                    if !line.is_empty() {
                        let upper = line.to_uppercase();
                        let msg = cheats::apply_cheat(&mut self.gs, &upper);
                        let display = if msg.is_empty() {
                            format!("Unknown command: {line}")
                        } else {
                            msg.to_string()
                        };
                        self.console.print(display);
                    }
                } else {
                    self.console.type_char(ch);
                }
            } else {
                // Console is closed: feed character to the cheat detector.
                if let Some(cheat_name) = self.cheat_detector.feed(ch) {
                    let msg = cheats::apply_cheat(&mut self.gs, cheat_name);
                    // IDDT toggles full automap reveal.
                    if cheat_name == "IDDT" {
                        self.automap_full_reveal = !self.automap_full_reveal;
                    }
                    if !msg.is_empty() {
                        self.console.print(msg.to_string());
                    }
                }
            }
        }

        // Toggle automap on Tab (stateful — each press flips visibility).
        if input.tab_pressed {
            self.automap_visible = !self.automap_visible;
        }

        // Quick save (F5).
        if input.f5_save {
            if let Err(e) = savegame::save_game(&self.save_path, &self.gs, 0) {
                self.console.print(format!("Save failed: {e}"));
            } else {
                self.console.print("Game saved.".to_string());
            }
        }

        // Quick load (F9).
        if input.f9_load {
            match savegame::load_game(&self.save_path) {
                Ok((_header, payload)) => {
                    if let Err(e) = savegame::apply_save(&mut self.gs, &payload) {
                        self.console.print(format!("Load failed: {e}"));
                    } else {
                        self.console.print("Game loaded.".to_string());
                    }
                }
                Err(e) => {
                    self.console.print(format!("Load failed: {e}"));
                }
            }
        }

        let cmd = ticinput_to_ticcmd(input);

        // Capture attack state *before* the tick so we can detect the leading
        // edge (button just pressed, not held from a previous tic).
        let attack_pressed_now = cmd.buttons & doom_tui::buttons::BT_ATTACK != 0;
        let attack_just_fired = attack_pressed_now && !self.prev_attack_down;

        self.gs.tick(cmd, Some(&mut self.level));

        // Emit weapon SFX on the leading edge of the attack button, and only
        // when the player is alive and has enough ammo to fire.
        if attack_just_fired
            && !self.gs.player.is_dead()
            && doom_game::player_can_fire(&self.gs)
        {
            if let Some(ref audio) = self.audio {
                let sfx_id = weapon_fire_sfx(self.gs.player.weapon);
                audio.play_sfx(sfx_id);
            }
        }

        self.prev_attack_down = attack_pressed_now;
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        let handle = self.gs.player.handle;
        let (px, py, angle) = match self.gs.mobjslab.get(handle) {
            Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.angle),
            None => (0, 0, Bam::ZERO),
        };

        let palette = PaletteLut::grayscale();

        if self.automap_visible {
            // Draw the overhead automap.
            // automap_full_reveal (toggled by IDDT) is wired for future use;
            // the current automap implementation already shows all linedefs.
            draw_automap(&self.level, px, py, angle, fb, &palette);
        } else {
            // Draw the first-person 3D view.
            // We pass a grayscale palette; render_level currently ignores it
            // (wall colors are derived from light levels only).
            render_level(&self.level, px, py, angle, fb, &palette, self.flat_cache.as_ref());

            // Draw HUD status bar over the bottom 32 rows.
            let god_mode =
                self.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] > 0;
            draw_status_bar(fb, &self.gs.player, god_mode);
        }
    }

    fn active_palette(&self) -> usize {
        0
    }
}

// ---------------------------------------------------------------------------
// Input conversion
// ---------------------------------------------------------------------------

/// Convert a `TicInput` from doom-tui to a `TicCmd` for doom-game.
///
/// Both structs have identical movement/button fields; this is a direct copy.
pub(crate) fn ticinput_to_ticcmd(input: TicInput) -> TicCmd {
    let mut cmd = TicCmd::default();
    cmd.forward_move = input.forward_move;
    cmd.side_move = input.side_move;
    cmd.angle_turn = input.angle_turn;
    cmd.buttons = input.buttons;
    cmd.chatchar = input.chatchar;
    cmd
}

// ---------------------------------------------------------------------------
// Player spawning
// ---------------------------------------------------------------------------

/// Find player 1 start thing (kind == 1) and spawn a Mobj at that position.
fn spawn_player(gs: &mut GameState, level: &Level) {
    // Thing::kind is the DoomEd type number; 1 = player 1 start.
    let start = level.things.iter().find(|t| t.kind == 1);

    let (x, y, angle_deg) = match start {
        Some(t) => (t.x as i32, t.y as i32, t.angle as u32),
        None => (0, 0, 0u32),
    };

    // Convert degrees (0–359) to 32-bit BAM.
    // BAM full circle = 2^32. One degree = 2^32 / 360 ≈ 11930465.
    let bam_per_degree = (0x1_0000_0000u64 / 360) as u32;
    let angle = Bam(angle_deg.wrapping_mul(bam_per_degree));

    let mut mo = Mobj::new(
        MobjKind::Player,
        Fixed16_16::from_int(x),
        Fixed16_16::from_int(y),
        angle,
    );
    mo.health = 100;
    mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
    mo.radius = Fixed16_16::from_int(16);
    mo.height = Fixed16_16::from_int(56);

    let handle = gs.mobjslab.alloc(mo);
    gs.player = doom_game::PlayerState::pistol_start(handle);
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    // Initialize trig tables (required for sin/cos in the game simulation).
    // SAFETY: called exactly once at startup, single-threaded, before any
    // Bam::sin() or Bam::cos() calls.
    unsafe {
        doom_types::Bam::init_trig_tables();
    }

    let args = Args::parse();

    // Load and parse the WAD file.
    let wad_bytes = std::fs::read(&args.wad)
        .with_context(|| format!("Failed to read WAD file: {}", args.wad.display()))?;

    let wad = WadFile::parse(wad_bytes)
        .with_context(|| format!("Failed to parse WAD: {}", args.wad.display()))?;

    // Build the PLAYPAL blit palette (for terminal RGB conversion).
    let blit_palette = match wad.find_lump_data("PLAYPAL") {
        Some(data) => PaletteLut::from_playpal(data)
            .unwrap_or_else(|_| PaletteLut::grayscale()),
        None => PaletteLut::grayscale(),
    };

    // Parse the requested level.
    let level = Level::from_wad(&wad, &args.warp)
        .with_context(|| format!("Failed to load map {}", args.warp))?;

    // Create game state and spawn the player at the map's player-1-start.
    let mut gs = GameState::new(&args.warp);
    spawn_player(&mut gs, &level);

    // Load flat texture cache (floor/ceiling textures between F_START and F_END).
    let flat_cache = FlatCache::load(&wad);
    let flat_cache = if flat_cache.is_empty() { None } else { Some(flat_cache) };

    // Try to open the audio subsystem.  Returns None in headless/CI environments.
    let audio = AudioSystem::try_open(&wad);

    // Start map music if audio is available.
    if let Some(ref audio) = audio {
        if let Some(music_lump) = music_lump_for_map(&args.warp) {
            if let Some(mus_data) = wad.find_lump_data(&music_lump) {
                audio.start_music(mus_data.to_vec());
            }
        }
    }

    // Validate mutually exclusive mode args: at most one of the four modes.
    let exclusive_modes = [
        args.record.is_some(),
        args.playdemo.is_some(),
        args.server.is_some(),
        args.connect.is_some(),
    ];
    if exclusive_modes.iter().filter(|&&x| x).count() > 1 {
        eprintln!("Error: --record, --playdemo, --server, and --connect are mutually exclusive");
        std::process::exit(1);
    }

    // Server mode: run relay, no game rendering.
    if let Some(port) = args.server {
        let rt = tokio::runtime::Runtime::new()?;
        return rt
            .block_on(net_mode::run_server(port, args.num_players))
            .map_err(|e| anyhow::anyhow!("Server error: {e}"));
    }

    // Build the app.
    let app = DoomGame::new(gs, level, audio, flat_cache);

    // Client (netplay) mode: connect to relay server and run game with net input.
    if let Some(addr_str) = args.connect {
        // Parse server address.
        let server_addr: SocketAddr = addr_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid --connect address '{addr_str}': {e}"))?;

        // Bind a local ephemeral UDP socket.
        let local_addr: SocketAddr = "0.0.0.0:0"
            .parse()
            .expect("hardcoded address is valid");

        let rt = tokio::runtime::Runtime::new()?;
        let client = rt
            .block_on(doom_net::NetClient::connect(local_addr, server_addr, args.player.saturating_sub(1)))
            .map_err(|e| anyhow::anyhow!("Connect failed: {e}"))?;

        let mut net_app = net_mode::NetGameApp::new(app, client, args.player);

        let mut event_loop = DoomEventLoop::new()
            .map_err(|e| anyhow::anyhow!("Failed to initialize terminal: {e}"))?;
        event_loop
            .run(&mut net_app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Event loop error: {e}"))?;

        return Ok(());
    }

    // Start the terminal event loop and run until the user quits (Q or Esc).
    let mut event_loop = DoomEventLoop::new()
        .map_err(|e| anyhow::anyhow!("Failed to initialize terminal: {e}"))?;

    if let Some(demo_path) = args.playdemo {
        // Load and parse the demo file.
        let demo_bytes = std::fs::read(&demo_path)
            .with_context(|| format!("Failed to read demo: {}", demo_path.display()))?;
        let player = DemoPlayer::parse(&demo_bytes)
            .with_context(|| "Failed to parse demo")?;
        let mut playback_app = demo_mode::DemoPlaybackApp::new(app, player);
        event_loop
            .run(&mut playback_app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Event loop error: {e}"))?;
    } else if let Some(record_path) = args.record {
        // Parse episode/map from the --warp argument.
        let (episode, map) = parse_warp_episode_map(&args.warp);
        // Clamp skill to 0-4 (LMP uses 0-based skill internally).
        let skill = args.skill.saturating_sub(1).min(4);
        let header = LmpHeader::new_singleplayer(skill, episode, map);
        let recorder = DemoRecorder::new(header);
        let mut recording_app = demo_mode::DemoRecordingWrapper::new(app, recorder, record_path);
        event_loop
            .run(&mut recording_app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Event loop error: {e}"))?;
    } else {
        let mut app = app;
        event_loop
            .run(&mut app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Event loop error: {e}"))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Warp string parsing
// ---------------------------------------------------------------------------

/// Parse a warp string (e.g. `"E1M1"` or `"MAP03"`) into `(episode, map)`.
///
/// For `E{e}M{m}` format: returns `(e, m)`.
/// For `MAP{xx}` format: returns `(1, xx)`.
/// Falls back to `(1, 1)` on parse failure.
fn parse_warp_episode_map(warp: &str) -> (u8, u8) {
    let upper = warp.to_uppercase();
    if let Some(rest) = upper.strip_prefix('E') {
        // E{e}M{m}
        if let Some(mid) = rest.find('M') {
            let ep_str = &rest[..mid];
            let map_str = &rest[mid + 1..];
            if let (Ok(ep), Ok(map)) = (ep_str.parse::<u8>(), map_str.parse::<u8>()) {
                return (ep, map);
            }
        }
    } else if let Some(rest) = upper.strip_prefix("MAP") {
        if let Ok(map) = rest.parse::<u8>() {
            return (1, map);
        }
    }
    // Fallback
    (1, 1)
}
