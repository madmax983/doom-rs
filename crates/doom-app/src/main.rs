//! Doom engine entry point.
//!
//! Usage: doom-app --wad doom1.wad [--warp E1M1]

mod cheats;
mod console;

use anyhow::{Context, Result};
use clap::Parser;
use doom_game::{GameState, Mobj, MobjKind, TicCmd, flags};
use doom_map::Level;
use doom_renderer::{Framebuffer, PaletteLut, render_level};
use doom_tui::{DoomApp, DoomEventLoop, TicInput};
use doom_types::{Bam, Fixed16_16};
use doom_wad::WadFile;

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
}

// ---------------------------------------------------------------------------
// DoomGame — implements DoomApp
// ---------------------------------------------------------------------------

struct DoomGame {
    gs: GameState,
    level: Level,
    cheat_detector: cheats::CheatDetector,
    console: console::Console,
}

impl DoomGame {
    fn new(gs: GameState, level: Level) -> Self {
        Self {
            gs,
            level,
            cheat_detector: cheats::CheatDetector::new(),
            console: console::Console::new(),
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
                    // Apply the cheat; message reserved for future HUD display.
                    let _msg = cheats::apply_cheat(&mut self.gs, cheat_name);
                }
            }
        }

        let cmd = ticinput_to_ticcmd(input);
        self.gs.tick(cmd, Some(&mut self.level));
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        let handle = self.gs.player.handle;
        let (px, py, angle) = match self.gs.mobjslab.get(handle) {
            Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.angle),
            None => (0, 0, Bam::ZERO),
        };
        // We pass a grayscale palette; render_level currently ignores it
        // (wall colors are derived from light levels only).
        let palette = PaletteLut::grayscale();
        render_level(&self.level, px, py, angle, fb, &palette);
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
fn ticinput_to_ticcmd(input: TicInput) -> TicCmd {
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

    // Build the app.
    let mut app = DoomGame::new(gs, level);

    // Start the terminal event loop and run until the user quits (Q or Esc).
    let mut event_loop = DoomEventLoop::new()
        .map_err(|e| anyhow::anyhow!("Failed to initialize terminal: {e}"))?;

    event_loop
        .run(&mut app, &blit_palette)
        .map_err(|e| anyhow::anyhow!("Event loop error: {e}"))?;

    Ok(())
}
