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
use doom_demo::{DemoPlayer, DemoRecorder, LmpHeader};
use doom_game::{GameState, Mobj, MobjKind, TicCmd, flags, init_scrolling_walls, init_conveyors};
use doom_game::cheats as game_cheats;
use doom_game::dehacked::DehPatch;
use doom_game::{MOBJINFO, STATES};
use doom_map::Level;
use doom_renderer::{AnimState, AutomapState, ColormapCache, FlatCache, Framebuffer, PaletteFlash, PaletteLut, SpriteCache, SwitchList, TextureCache, draw_automap_ex, draw_status_bar, draw_weapon_sprite, render_level, render_things};
use doom_renderer::IDENTITY_COLORMAP;
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

    /// Path to DeHackEd (.deh) patch file to apply.
    #[arg(long)]
    deh: Option<String>,

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
    /// Chatchar-based cheat buffer (doom-game's ring-buffer cheat detector).
    /// Processes raw `chatchar` bytes from TicInput for classic Doom cheat entry.
    cheat_buffer: game_cheats::CheatBuffer,
    /// Timed cheat message overlay: (message text, remaining tics).
    /// Displayed at the top of the screen and ticks down each frame.
    cheat_message: Option<(String, u32)>,
    console: console::Console,
    /// Path used for quick save (F5) and quick load (F9).
    save_path: std::path::PathBuf,
    /// Stateful automap with zoom/pan/follow support.
    automap: AutomapState,
    /// Whether the IDDT cheat has toggled full automap reveal.
    automap_full_reveal: bool,
    /// Optional audio subsystem.  `None` when no audio device is available.
    audio: Option<AudioSystem>,
    /// Whether the attack button was held during the previous tic (for edge detection).
    prev_attack_down: bool,
    /// Flat texture cache (floor/ceiling textures loaded from the WAD).
    flat_cache: Option<FlatCache>,
    /// Wall texture cache (TEXTURE1/TEXTURE2 composed textures from the WAD).
    tex_cache: Option<TextureCache>,
    /// Sprite frame cache (loaded from S_START..S_END).
    sprite_cache: Option<SpriteCache>,
    /// Colormap cache (COLORMAP lump, 34 × 256 bytes for light-level shading).
    colormap_cache: Option<ColormapCache>,
    /// Animated texture state (flat + wall animation sequences, ticked per tic).
    anim_state: AnimState,
    /// Palette flash controller (pain/pickup/rad-suit full-screen tints).
    palette_flash: PaletteFlash,
    /// Switch texture pair lookup (SW1xxx <-> SW2xxx bidirectional).
    switch_list: SwitchList,
    /// Player health from the previous tic — used to detect damage for pain flash.
    prev_health: i32,
}

impl DoomGame {
    fn new(
        mut gs: GameState,
        level: Level,
        audio: Option<AudioSystem>,
        flat_cache: Option<FlatCache>,
        tex_cache: Option<TextureCache>,
        sprite_cache: Option<SpriteCache>,
        colormap_cache: Option<ColormapCache>,
    ) -> Self {
        // Initialize scrolling wall and conveyor belt specials from level linedefs.
        init_scrolling_walls(&mut gs, &level);
        init_conveyors(&mut gs, &level);

        // Capture initial player health for pain-flash delta detection.
        let initial_health = gs.player.health();

        Self {
            gs,
            level,
            cheat_detector: cheats::CheatDetector::new(),
            cheat_buffer: game_cheats::CheatBuffer::new(),
            cheat_message: None,
            console: console::Console::new(),
            save_path: std::path::PathBuf::from("doom_save.bin"),
            automap: AutomapState::new(),
            automap_full_reveal: false,
            audio,
            prev_attack_down: false,
            flat_cache,
            tex_cache,
            sprite_cache,
            colormap_cache,
            anim_state: AnimState::new(),
            palette_flash: PaletteFlash::new(),
            switch_list: SwitchList::new(),
            prev_health: initial_health,
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
                        self.cheat_message = Some((msg.to_string(), 105)); // 3 sec @ 35 tics/sec
                        self.console.print(msg.to_string());
                    }
                }
            }
        }

        // Toggle automap on Tab (stateful — each press flips visibility).
        if input.tab_pressed {
            self.automap.toggle();
        }

        // Feed typed characters from chatchar to the ring-buffer cheat detector.
        // This is the classic Doom cheat entry path: typed characters during
        // gameplay are matched against known sequences (iddqd, idkfa, etc.).
        if input.chatchar != 0 {
            self.cheat_buffer.push(input.chatchar);
            if let Some(code) = game_cheats::check_cheats(&self.cheat_buffer) {
                game_cheats::apply_cheat(&mut self.gs, code);
                let msg = game_cheats::cheat_message(code).to_string();
                self.cheat_message = Some((msg, 105)); // 3 seconds at 35 tics/sec
                self.cheat_buffer.clear();
            }
        }

        // Tick down cheat message timer.
        if let Some((_, ref mut tics)) = self.cheat_message {
            if *tics > 0 {
                *tics -= 1;
            } else {
                self.cheat_message = None;
            }
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

        // Advance animated texture state (flat and wall animations).
        self.anim_state.tick();

        // Advance palette flash timer (pain/pickup/rad-suit tints).
        self.palette_flash.tick();

        // Detect player damage and trigger a pain flash.
        {
            let cur_health = self.gs.player.health();
            if cur_health < self.prev_health {
                let damage = self.prev_health - cur_health;
                // Pain palette indices 1-8 (increasing red tint).
                // Simple formula: one palette step per 8 HP lost, clamped.
                let palette = ((damage / 8) as usize).clamp(1, 8);
                self.palette_flash.trigger(palette, 12);
            }
            self.prev_health = cur_health;
        }

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

        // Update automap center to follow the player position.
        if self.automap.active {
            if let Some(mo) = self.gs.mobjslab.get(self.gs.player.handle) {
                self.automap.update_center(mo.x.to_int(), mo.y.to_int());
            }
        }
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        let handle = self.gs.player.handle;
        let (px, py, angle) = match self.gs.mobjslab.get(handle) {
            Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.angle),
            None => (0, 0, Bam::ZERO),
        };

        let palette = PaletteLut::grayscale();

        if self.automap.active {
            // Draw the overhead automap using the stateful AutomapState
            // (supports zoom/pan/follow). automap_full_reveal (toggled by
            // IDDT) is wired for future use; the current automap
            // implementation already shows all linedefs.
            draw_automap_ex(fb, &self.level, &self.automap, px, py, angle);

            // Draw status bar over the bottom of the automap so it is
            // always visible (matching original Doom behaviour).
            let god_mode =
                self.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] > 0;
            draw_status_bar(fb, &self.gs.player, god_mode);
        } else {
            // Draw the first-person 3D view.
            // We pass a grayscale palette; render_level currently ignores it
            // (wall colors are derived from light levels only).
            let z_buf = render_level(&self.level, px, py, angle, fb, &palette, self.flat_cache.as_ref(), self.tex_cache.as_ref(), self.colormap_cache.as_ref(), None);

            // Project level Things as billboard sprites (painter's algorithm,
            // back-to-front). Must run after render_level so walls are already
            // drawn into the framebuffer. Z-buffer clips sprites behind walls.
            if let Some(ref cache) = self.sprite_cache {
                render_things(
                    &self.level,
                    Fixed16_16::from_int(px),
                    Fixed16_16::from_int(py),
                    angle,
                    fb,
                    cache,
                    Some(&z_buf),
                );
            }

            // Draw weapon sprite overlay (pistol idle frame A).
            if let Some(ref cache) = self.sprite_cache {
                draw_weapon_sprite(fb, b"PISGA0\0\0", cache, &IDENTITY_COLORMAP);
            }

            // Draw HUD status bar over the bottom 32 rows.
            let god_mode =
                self.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] > 0;
            draw_status_bar(fb, &self.gs.player, god_mode);
        }

        // Draw cheat message overlay at the top of the screen (if active).
        if let Some((ref msg, _)) = self.cheat_message {
            draw_cheat_message_overlay(fb, msg);
        }
    }

    fn active_palette(&self) -> usize {
        self.palette_flash.active_palette()
    }
}

// ---------------------------------------------------------------------------
// Cheat message overlay
// ---------------------------------------------------------------------------

/// Draw a cheat message string at the top of the framebuffer.
///
/// Uses a minimal 4x6 bitmap font to render ASCII text. Each character cell
/// is 5 pixels wide (4px glyph + 1px spacing). The message is rendered in
/// yellow (palette index 231) on a black (palette index 0) background bar.
fn draw_cheat_message_overlay(fb: &mut Framebuffer, msg: &str) {
    const FB_W: usize = 320;
    const CHAR_W: usize = 5; // 4px glyph + 1px gap
    const CHAR_H: usize = 7; // 6px glyph + 1px gap
    const TOP_Y: usize = 2;
    const COLOR_MSG: u8 = 231; // yellow
    const COLOR_MSG_BG: u8 = 0; // black

    // Draw a background bar across the top of the screen.
    let bar_h = CHAR_H + 2; // 1px padding top+bottom
    for y in 0..bar_h {
        let row_start = y * FB_W;
        let row_end = row_start + FB_W;
        if row_end <= fb.data.len() {
            fb.data[row_start..row_end].fill(COLOR_MSG_BG);
        }
    }

    // Center the message horizontally.
    let msg_width_px = msg.len() * CHAR_W;
    let start_x = if msg_width_px < FB_W {
        (FB_W - msg_width_px) / 2
    } else {
        0
    };

    for (ci, ch) in msg.chars().enumerate() {
        let glyph = mini_glyph(ch);
        let cx = start_x + ci * CHAR_W;
        for (row, &bits) in glyph.iter().enumerate() {
            let sy = TOP_Y + row;
            if sy >= 200 { break; }
            for col in 0..4 {
                if bits & (1 << (3 - col)) != 0 {
                    let sx = cx + col;
                    if sx < FB_W {
                        fb.set_pixel(sx, sy, COLOR_MSG);
                    }
                }
            }
        }
    }
}

/// Return a 4-wide, 6-tall bitmap for an ASCII character.
///
/// Each entry is a row; bits 3..0 map to columns left-to-right.
/// Only uppercase letters, digits, spaces, and a few punctuation marks
/// are defined; everything else maps to a solid block.
fn mini_glyph(ch: char) -> [u8; 6] {
    match ch.to_ascii_uppercase() {
        'A' => [0b0110, 0b1001, 0b1111, 0b1001, 0b1001, 0b0000],
        'B' => [0b1110, 0b1001, 0b1110, 0b1001, 0b1110, 0b0000],
        'C' => [0b0111, 0b1000, 0b1000, 0b1000, 0b0111, 0b0000],
        'D' => [0b1110, 0b1001, 0b1001, 0b1001, 0b1110, 0b0000],
        'E' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1111, 0b0000],
        'F' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1000, 0b0000],
        'G' => [0b0111, 0b1000, 0b1011, 0b1001, 0b0111, 0b0000],
        'H' => [0b1001, 0b1001, 0b1111, 0b1001, 0b1001, 0b0000],
        'I' => [0b1110, 0b0100, 0b0100, 0b0100, 0b1110, 0b0000],
        'J' => [0b0001, 0b0001, 0b0001, 0b1001, 0b0110, 0b0000],
        'K' => [0b1001, 0b1010, 0b1100, 0b1010, 0b1001, 0b0000],
        'L' => [0b1000, 0b1000, 0b1000, 0b1000, 0b1111, 0b0000],
        'M' => [0b1001, 0b1111, 0b1111, 0b1001, 0b1001, 0b0000],
        'N' => [0b1001, 0b1101, 0b1011, 0b1001, 0b1001, 0b0000],
        'O' => [0b0110, 0b1001, 0b1001, 0b1001, 0b0110, 0b0000],
        'P' => [0b1110, 0b1001, 0b1110, 0b1000, 0b1000, 0b0000],
        'Q' => [0b0110, 0b1001, 0b1001, 0b1010, 0b0101, 0b0000],
        'R' => [0b1110, 0b1001, 0b1110, 0b1010, 0b1001, 0b0000],
        'S' => [0b0111, 0b1000, 0b0110, 0b0001, 0b1110, 0b0000],
        'T' => [0b1110, 0b0100, 0b0100, 0b0100, 0b0100, 0b0000],
        'U' => [0b1001, 0b1001, 0b1001, 0b1001, 0b0110, 0b0000],
        'V' => [0b1001, 0b1001, 0b1001, 0b0110, 0b0110, 0b0000],
        'W' => [0b1001, 0b1001, 0b1111, 0b1111, 0b1001, 0b0000],
        'X' => [0b1001, 0b0110, 0b0110, 0b0110, 0b1001, 0b0000],
        'Y' => [0b1001, 0b1001, 0b0110, 0b0100, 0b0100, 0b0000],
        'Z' => [0b1111, 0b0010, 0b0100, 0b1000, 0b1111, 0b0000],
        '0' => [0b0110, 0b1001, 0b1001, 0b1001, 0b0110, 0b0000],
        '1' => [0b0100, 0b1100, 0b0100, 0b0100, 0b1110, 0b0000],
        '2' => [0b0110, 0b1001, 0b0010, 0b0100, 0b1111, 0b0000],
        '3' => [0b1110, 0b0001, 0b0110, 0b0001, 0b1110, 0b0000],
        '4' => [0b1001, 0b1001, 0b1111, 0b0001, 0b0001, 0b0000],
        '5' => [0b1111, 0b1000, 0b1110, 0b0001, 0b1110, 0b0000],
        '6' => [0b0110, 0b1000, 0b1110, 0b1001, 0b0110, 0b0000],
        '7' => [0b1111, 0b0001, 0b0010, 0b0100, 0b0100, 0b0000],
        '8' => [0b0110, 0b1001, 0b0110, 0b1001, 0b0110, 0b0000],
        '9' => [0b0110, 0b1001, 0b0111, 0b0001, 0b0110, 0b0000],
        ' ' => [0b0000, 0b0000, 0b0000, 0b0000, 0b0000, 0b0000],
        '!' => [0b0100, 0b0100, 0b0100, 0b0000, 0b0100, 0b0000],
        '.' => [0b0000, 0b0000, 0b0000, 0b0000, 0b0100, 0b0000],
        '(' => [0b0010, 0b0100, 0b0100, 0b0100, 0b0010, 0b0000],
        ')' => [0b0100, 0b0010, 0b0010, 0b0010, 0b0100, 0b0000],
        _   => [0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b0000],
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

    // Apply DeHackEd patch if one was specified.
    if let Some(ref deh_path) = args.deh {
        let contents = std::fs::read_to_string(deh_path)
            .with_context(|| format!("Failed to read DeHackEd file: {deh_path}"))?;
        let patch = DehPatch::parse(&contents)
            .map_err(|e| anyhow::anyhow!("DeHackEd parse error: {e}"))?;
        let mut mobjinfo_vec: Vec<_> = MOBJINFO.to_vec();
        let mut states_vec: Vec<_> = STATES.to_vec();
        let count = patch
            .apply(&mut mobjinfo_vec, &mut states_vec)
            .map_err(|e| anyhow::anyhow!("DeHackEd apply error: {e}"))?;
        eprintln!("DeHackEd: applied {count} modification(s) from {deh_path}");
    }

    // Load flat texture cache (floor/ceiling textures between F_START and F_END).
    let flat_cache = FlatCache::load(&wad);
    let flat_cache = if flat_cache.is_empty() { None } else { Some(flat_cache) };

    // Load wall texture cache (TEXTURE1/TEXTURE2 composed textures).
    let tex_cache = {
        let cache = TextureCache::load(&wad);
        if cache.is_empty() { None } else { Some(cache) }
    };

    // Load sprite cache (sprite frames between S_START and S_END).
    let sprite_cache = {
        let cache = SpriteCache::load(&wad);
        if cache.is_empty() { None } else { Some(cache) }
    };

    // Load colormap cache (COLORMAP lump: 34 × 256 bytes, light-level shading).
    let colormap_cache = Some(ColormapCache::from_wad_file(&wad));

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

    // Server mode: spin up a relay server that forwards tic packets between
    // connected clients. No WAD-based game loop is required on the server.
    if let Some(port) = args.server {
        return net_mode::run_server(port);
    }

    // Build the app.
    let app = DoomGame::new(gs, level, audio, flat_cache, tex_cache, sprite_cache, colormap_cache);

    // Client (netplay) mode: wrap DoomGame in a NetGameApp for network-aware input.
    if let Some(ref addr_str) = args.connect {
        let client = doom_net::NetClient::connect(addr_str, 0)
            .map_err(|e| anyhow::anyhow!("Failed to connect to server {addr_str}: {e}"))?;
        let mut net_app = net_mode::NetGameApp::new(app, client);

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_game::{GameState, Mobj, MobjKind, PlayerState, flags};
    use doom_game::cheats as game_cheats;
    use doom_map::{Blockmap, Level, Reject, Sector};
    use doom_renderer::{Framebuffer, StatusBarData};
    use doom_types::{Bam, Fixed16_16};

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    /// Build a minimal `Level` suitable for unit tests.
    pub(crate) fn make_test_level() -> Level {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
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

    pub(crate) fn make_game_state() -> GameState {
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

    pub(crate) fn make_doom_game() -> DoomGame {
        DoomGame::new(
            make_game_state(),
            make_test_level(),
            None, None, None, None, None,
        )
    }

    // -----------------------------------------------------------------------
    // Test 1: AutomapState toggle works through DoomGame
    // -----------------------------------------------------------------------

    #[test]
    fn automap_toggle_via_tab_pressed() {
        let mut game = make_doom_game();
        assert!(!game.automap.active, "automap must start inactive");

        // Simulate Tab press.
        let mut input = TicInput::default();
        input.tab_pressed = true;
        game.tick(input);
        assert!(game.automap.active, "automap must be active after first Tab");

        // Tab again.
        let mut input2 = TicInput::default();
        input2.tab_pressed = true;
        game.tick(input2);
        assert!(!game.automap.active, "automap must be inactive after second Tab");
    }

    // -----------------------------------------------------------------------
    // Test 2: AutomapState follows player position
    // -----------------------------------------------------------------------

    #[test]
    fn automap_follows_player_position() {
        let mut game = make_doom_game();

        // Open the automap.
        let mut input = TicInput::default();
        input.tab_pressed = true;
        game.tick(input);

        // The player was spawned at (0, 0), so the automap center should track there.
        assert!(
            game.automap.center_x.abs() < 1.0,
            "automap center_x should track player at origin"
        );
        assert!(
            game.automap.center_y.abs() < 1.0,
            "automap center_y should track player at origin"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: CheatBuffer integration (from doom-game)
    // -----------------------------------------------------------------------

    #[test]
    fn cheat_buffer_detects_god_mode() {
        let mut buf = game_cheats::CheatBuffer::new();
        for &ch in b"iddqd" {
            buf.push(ch);
        }
        let result = game_cheats::check_cheats(&buf);
        assert_eq!(
            result,
            Some(game_cheats::CheatCode::GodMode),
            "CheatBuffer must detect IDDQD sequence"
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: CheatBuffer clears after cheat detection
    // -----------------------------------------------------------------------

    #[test]
    fn cheat_buffer_clears_after_detection() {
        let mut buf = game_cheats::CheatBuffer::new();
        for &ch in b"iddqd" {
            buf.push(ch);
        }
        assert!(game_cheats::check_cheats(&buf).is_some());
        buf.clear();
        assert!(
            game_cheats::check_cheats(&buf).is_none(),
            "CheatBuffer must return None after clear"
        );
    }

    // -----------------------------------------------------------------------
    // Test 5: StatusBarData from default PlayerState
    // -----------------------------------------------------------------------

    #[test]
    fn statusbar_data_from_default_player() {
        let gs = make_game_state();
        let data = StatusBarData::from_player(&gs.player);

        // Pistol start: health 100, armor 0, weapon = pistol (1).
        assert_eq!(data.health, 100, "pistol start health must be 100");
        assert_eq!(data.armor, 0, "pistol start armor must be 0");
        assert_eq!(data.keys, 0, "pistol start keys must be 0");
        // Weapon 1 = pistol (bullet ammo).
        assert_eq!(data.ready_weapon, 1, "pistol start weapon must be 1 (pistol)");
    }

    // -----------------------------------------------------------------------
    // Test 6: Cheat message appears and expires
    // -----------------------------------------------------------------------

    #[test]
    fn cheat_message_expires_after_ticking() {
        let mut game = make_doom_game();
        // Manually set a cheat message.
        game.cheat_message = Some(("Test message".to_string(), 3));

        // Tick 3 times to expire the message.
        for _ in 0..3 {
            game.tick(TicInput::default());
        }
        // After 3 ticks the tics should have reached 0 and on the next tick
        // it should be cleared.
        game.tick(TicInput::default());
        assert!(
            game.cheat_message.is_none(),
            "cheat message must be None after expiring"
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: DoomGame starts with correct defaults
    // -----------------------------------------------------------------------

    #[test]
    fn doom_game_new_defaults() {
        let game = make_doom_game();
        assert!(!game.automap.active, "automap must start inactive");
        assert!(!game.automap_full_reveal, "automap_full_reveal must start false");
        assert!(game.cheat_message.is_none(), "cheat_message must start None");
    }

    // -----------------------------------------------------------------------
    // Test 8: draw_cheat_message_overlay does not panic
    // -----------------------------------------------------------------------

    #[test]
    fn draw_cheat_message_overlay_does_not_panic() {
        let mut fb = Framebuffer::new();
        draw_cheat_message_overlay(&mut fb, "Degreelessness Mode On");
        // Verify something was drawn (the background bar at minimum).
        let has_non_zero = fb.data[..320 * 9].iter().any(|&b| b != 0);
        assert!(
            has_non_zero,
            "cheat overlay must draw visible pixels in the top rows"
        );
    }

    // -----------------------------------------------------------------------
    // Test 9: draw_cheat_message_overlay with empty string
    // -----------------------------------------------------------------------

    #[test]
    fn draw_cheat_message_overlay_empty_string_no_panic() {
        let mut fb = Framebuffer::new();
        draw_cheat_message_overlay(&mut fb, "");
        // Empty message: only background bar drawn (all black), no crash.
    }

    // -----------------------------------------------------------------------
    // Test 10: mini_glyph returns expected patterns
    // -----------------------------------------------------------------------

    #[test]
    fn mini_glyph_space_is_blank() {
        let glyph = mini_glyph(' ');
        assert!(
            glyph.iter().all(|&row| row == 0),
            "space glyph must be all zeros"
        );
    }

    // -----------------------------------------------------------------------
    // Test 11: mini_glyph letter has non-zero rows
    // -----------------------------------------------------------------------

    #[test]
    fn mini_glyph_letter_a_has_pixels() {
        let glyph = mini_glyph('A');
        let non_zero_rows = glyph.iter().filter(|&&row| row != 0).count();
        assert!(
            non_zero_rows >= 4,
            "letter A glyph must have at least 4 non-zero rows, got {non_zero_rows}"
        );
    }

    // -----------------------------------------------------------------------
    // Test 12: chatchar-based cheat integration via DoomGame tick
    // -----------------------------------------------------------------------

    #[test]
    fn chatchar_cheat_sets_god_mode() {
        let mut game = make_doom_game();
        assert!(!game.gs.player.god_mode, "god_mode must start false");

        // Feed "iddqd" via chatchar one character per tic.
        for &ch in b"iddqd" {
            let mut input = TicInput::default();
            input.chatchar = ch;
            game.tick(input);
        }

        assert!(
            game.gs.player.god_mode,
            "god_mode must be true after typing iddqd via chatchar"
        );
        assert!(
            game.cheat_message.is_some(),
            "cheat_message must be set after cheat activation"
        );
    }

    // ===================================================================
    // Tests for AnimState, PaletteFlash, SwitchList, scrolling walls
    // ===================================================================

    // -----------------------------------------------------------------------
    // Test 13: DoomGame creates with AnimState initialized
    // -----------------------------------------------------------------------

    #[test]
    fn doom_game_creates_with_anim_state_initialized() {
        let game = make_doom_game();
        // AnimState should start at tic 0 with all standard sequences loaded.
        assert_eq!(
            game.anim_state.tic_count(),
            0,
            "AnimState tic_count must start at 0"
        );
        assert!(
            !game.anim_state.sequences().is_empty(),
            "AnimState must have animation sequences loaded"
        );
    }

    // -----------------------------------------------------------------------
    // Test 14: DoomGame creates with PaletteFlash initialized
    // -----------------------------------------------------------------------

    #[test]
    fn doom_game_creates_with_palette_flash_initialized() {
        let game = make_doom_game();
        assert_eq!(
            game.palette_flash.active_palette(),
            0,
            "PaletteFlash must start at palette 0 (no flash)"
        );
        assert_eq!(
            game.palette_flash.remaining(),
            0,
            "PaletteFlash remaining must be 0 at creation"
        );
    }

    // -----------------------------------------------------------------------
    // Test 15: active_palette returns 0 by default (no flash)
    // -----------------------------------------------------------------------

    #[test]
    fn active_palette_returns_zero_by_default() {
        let game = make_doom_game();
        assert_eq!(
            game.active_palette(),
            0,
            "active_palette() must return 0 when no flash is active"
        );
    }

    // -----------------------------------------------------------------------
    // Test 16: PaletteFlash tick integration (trigger -> non-zero -> decays)
    // -----------------------------------------------------------------------

    #[test]
    fn palette_flash_tick_integration() {
        let mut game = make_doom_game();

        // Manually trigger a pain flash (palette 4, 3 tics duration).
        game.palette_flash.trigger(4, 3);
        assert_eq!(
            game.active_palette(),
            4,
            "active_palette must be 4 after trigger"
        );

        // Tick 3 times (palette_flash.tick is called inside game.tick).
        for _ in 0..3 {
            game.tick(TicInput::default());
        }

        // After 3 tics the flash should have expired back to 0.
        assert_eq!(
            game.active_palette(),
            0,
            "active_palette must return to 0 after flash duration expires"
        );
    }

    // -----------------------------------------------------------------------
    // Test 17: AnimState tick integration (tic count advances)
    // -----------------------------------------------------------------------

    #[test]
    fn anim_state_tick_integration() {
        let mut game = make_doom_game();
        assert_eq!(game.anim_state.tic_count(), 0);

        // Each game.tick() should advance anim_state by one.
        game.tick(TicInput::default());
        assert_eq!(
            game.anim_state.tic_count(),
            1,
            "AnimState tic_count must advance by 1 after one tick"
        );

        for _ in 0..9 {
            game.tick(TicInput::default());
        }
        assert_eq!(
            game.anim_state.tic_count(),
            10,
            "AnimState tic_count must be 10 after 10 ticks"
        );
    }

    // -----------------------------------------------------------------------
    // Test 18: Scrolling walls initialized during DoomGame::new
    // -----------------------------------------------------------------------

    #[test]
    fn scrolling_walls_initialized_during_new() {
        // Our test level has no linedefs with special 48/85, so scrolling_walls
        // should be empty. The important thing is that init_scrolling_walls ran
        // without panicking.
        let game = make_doom_game();
        let offset = game.gs.get_scroll_offset(0);
        assert_eq!(
            offset,
            (0, 0),
            "get_scroll_offset must return (0, 0) for a level with no scrolling walls"
        );
    }

    // -----------------------------------------------------------------------
    // Test 19: Pain flash triggers on health decrease
    // -----------------------------------------------------------------------

    #[test]
    fn pain_flash_triggers_on_health_decrease() {
        let mut game = make_doom_game();
        assert_eq!(game.active_palette(), 0, "no flash initially");

        // Simulate damage by directly reducing the mobj's health and the
        // player's health (mirroring what damage_mobj does in the game).
        let handle = game.gs.player.handle;
        if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
            mo.health = 70; // was 100, so 30 damage
        }
        game.gs.player.set_health_capped(70, 100);

        // prev_health is 100 (captured at creation). After tick, the game
        // should detect health dropped 100 -> 70 = 30 damage.
        game.tick(TicInput::default());

        // The flash should now be active. 30 / 8 = 3, clamped to [1,8] = 3.
        assert_eq!(
            game.palette_flash.active_palette(),
            3,
            "pain flash palette should be 3 for 30 damage (30/8 = 3)"
        );
        assert!(
            game.palette_flash.remaining() > 0,
            "pain flash should have remaining tics"
        );
    }

    // -----------------------------------------------------------------------
    // Test 20: SwitchList initialized with pairs
    // -----------------------------------------------------------------------

    #[test]
    fn switch_list_initialized_with_pairs() {
        let game = make_doom_game();
        assert_eq!(
            game.switch_list.len(),
            29,
            "SwitchList must have 29 standard Doom switch pairs"
        );
        assert!(
            !game.switch_list.is_empty(),
            "SwitchList must not be empty"
        );
        // Verify bidirectional lookup works.
        assert!(
            game.switch_list.get_opposite(b"SW1EXIT\0").is_some(),
            "SW1EXIT should have an opposite in the switch list"
        );
    }

    // -----------------------------------------------------------------------
    // Test 21: Pain flash does NOT trigger when health stays the same
    // -----------------------------------------------------------------------

    #[test]
    fn no_pain_flash_when_health_unchanged() {
        let mut game = make_doom_game();

        // Tick without any damage.
        game.tick(TicInput::default());

        assert_eq!(
            game.active_palette(),
            0,
            "active_palette must remain 0 when player takes no damage"
        );
    }

    // -----------------------------------------------------------------------
    // Test 22: Pain flash palette scales with damage amount
    // -----------------------------------------------------------------------

    #[test]
    fn pain_flash_palette_scales_with_damage() {
        // Small damage (7 HP): palette = max(7/8, 1) = 1
        {
            let mut game = make_doom_game();
            let handle = game.gs.player.handle;
            if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
                mo.health = 93;
            }
            game.gs.player.set_health_capped(93, 100);
            game.tick(TicInput::default());
            assert_eq!(
                game.palette_flash.active_palette(),
                1,
                "7 damage should give palette 1 (7/8=0, clamped to 1)"
            );
        }

        // Large damage (80 HP): palette = min(80/8, 8) = 8
        {
            let mut game = make_doom_game();
            let handle = game.gs.player.handle;
            if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
                mo.health = 20;
            }
            game.gs.player.set_health_capped(20, 100);
            game.tick(TicInput::default());
            assert_eq!(
                game.palette_flash.active_palette(),
                8,
                "80 damage should give palette 8 (80/8=10, clamped to 8)"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test 23: prev_health tracks across multiple ticks
    // -----------------------------------------------------------------------

    #[test]
    fn prev_health_tracks_across_ticks() {
        let mut game = make_doom_game();
        assert_eq!(game.prev_health, 100, "prev_health starts at 100");

        // First tick with no damage: prev_health should update to current.
        game.tick(TicInput::default());
        assert_eq!(game.prev_health, 100, "prev_health still 100 after no-damage tick");

        // Simulate damage between ticks.
        let handle = game.gs.player.handle;
        if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
            mo.health = 80;
        }
        game.gs.player.set_health_capped(80, 100);
        game.tick(TicInput::default());
        assert_eq!(game.prev_health, 80, "prev_health updated to 80 after damage");

        // Second damage event.
        if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
            mo.health = 50;
        }
        game.gs.player.set_health_capped(50, 100);
        game.tick(TicInput::default());
        assert_eq!(game.prev_health, 50, "prev_health updated to 50 after second damage");
    }

    // -----------------------------------------------------------------------
    // Test 24: CLI args parse --deh flag correctly
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_parse_deh_flag() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad", "doom1.wad",
            "--deh", "my_patch.deh",
        ]);
        assert!(args.is_ok(), "args with --deh must parse successfully");
        let args = args.unwrap();
        assert_eq!(args.deh.as_deref(), Some("my_patch.deh"));
    }

    // -----------------------------------------------------------------------
    // Test 25: CLI args parse --server flag with port
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_parse_server_flag() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad", "doom1.wad",
            "--server", "5029",
        ]);
        assert!(args.is_ok(), "args with --server must parse successfully");
        let args = args.unwrap();
        assert_eq!(args.server, Some(5029));
    }

    // -----------------------------------------------------------------------
    // Test 26: CLI args parse --connect flag with address
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_parse_connect_flag() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad", "doom1.wad",
            "--connect", "127.0.0.1:5029",
        ]);
        assert!(args.is_ok(), "args with --connect must parse successfully");
        let args = args.unwrap();
        assert_eq!(args.connect.as_deref(), Some("127.0.0.1:5029"));
    }

    // -----------------------------------------------------------------------
    // Test 27: DeHackEd parse error for nonexistent file
    // -----------------------------------------------------------------------

    #[test]
    fn dehacked_file_not_found() {
        let result = std::fs::read_to_string("nonexistent_patch.deh");
        assert!(
            result.is_err(),
            "reading a nonexistent .deh file must return an error"
        );
    }

    // -----------------------------------------------------------------------
    // Test 28: DeHackEd parse succeeds on valid patch text
    // -----------------------------------------------------------------------

    #[test]
    fn dehacked_parse_valid_patch() {
        use doom_game::dehacked::DehPatch;

        let patch_text = "Thing 1\nHit points = 200\n";
        let patch = DehPatch::parse(patch_text);
        assert!(patch.is_ok(), "DehPatch::parse must succeed on valid input");
        let patch = patch.unwrap();
        assert_eq!(patch.things.len(), 1);
        assert_eq!(patch.things[0].hit_points, Some(200));
    }

    // -----------------------------------------------------------------------
    // Test 29: CLI args --deh defaults to None
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_deh_defaults_to_none() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad", "doom1.wad",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(args.deh.is_none(), "--deh must default to None");
    }

    // -----------------------------------------------------------------------
    // Test 30: CLI args --server defaults to None
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_server_defaults_to_none() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad", "doom1.wad",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(args.server.is_none(), "--server must default to None");
    }

    // -----------------------------------------------------------------------
    // Test 31: CLI args --connect defaults to None
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_connect_defaults_to_none() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad", "doom1.wad",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(args.connect.is_none(), "--connect must default to None");
    }
}
