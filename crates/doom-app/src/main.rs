//! Doom engine entry point and master orchestrator.
//!
//! # The Grand Assembly
//!
//! While the `doom-*` crates are meticulously decoupled components—`doom-game`
//! simulates the world, `doom-renderer` paints the walls, `doom-audio` mixes the
//! screams, and `doom-tui` maps pixels to the terminal—they cannot play Doom
//! on their own. They need a conductor.
//!
//! `doom-app` is that conductor. It owns the main executable loop and wires
//! the isolated systems together. It parses command-line arguments, loads the
//! `WadFile`s, initializes the `AudioSystem`, sets up the `Terminal`, and
//! pumps the `DoomEventLoop`.
//!
//! # Modes of Play
//!
//! The app can boot into several different modes depending on the arguments:
//! - **Singleplayer**: The default mode. Connects local `TicInput` directly to `doom-game`.
//! - **Demo Playback**: Wraps the game in a [`demo_mode::DemoPlaybackApp`], ignoring
//!   local input and feeding pre-recorded tics from an LMP file.
//! - **Demo Recording**: Wraps the game in a [`demo_mode::DemoRecordingWrapper`], saving
//!   every local input to disk while playing.
//! - **Netplay Client**: Uses [`net_mode::NetGameApp`] to synchronize tics over UDP
//!   with a relay server before feeding them to the local simulation.
//!
//! Usage: doom-app --iwad doom1.wad [--pwad mod.wad] [--warp E1M1]

mod audio_system;
mod cheats;
mod cogmind;
mod console;
mod demo_mode;
mod net_mode;
mod savegame;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use doom_demo::{DemoPlayer, DemoRecorder, LmpHeader};
use doom_game::FaceState;
use doom_game::LockedDoorColor;
use doom_game::cheats as game_cheats;
use doom_game::dehacked::DehPatch;
use doom_game::{
    AutomapState, GamePhase, GamePhaseController, GameState, Skill, TitleScreen, init_conveyors,
    init_scrolling_walls, init_sector_lights, kind_to_doomed_type, spawn_level_things,
};
use doom_game::{MOBJINFO, STATES};
use doom_map::Level;
use doom_renderer::IDENTITY_COLORMAP;
use doom_renderer::{
    ActorRenderInfo, AnimState, BitmapFont, ColormapCache, FlatCache, Framebuffer,
    IntermissionRenderer, PLAYER_HEIGHT, PaletteFlash, PaletteLut, PatchCache, RenderOut,
    SpriteCache, SpriteClip, TextureCache, WadFont, WeaponAnimState, WeaponTransition,
    draw_automap_ex, draw_finale_wad, draw_intermission, draw_intermission_wad, draw_menu_wad,
    draw_status_bar_wad, draw_title_screen_wad, draw_weapon_animated_with_override,
    render_actors_with_masked_and_fixed_colormap_ex, render_flag_from_state,
    render_level_with_view_height_and_extra_light_and_fixed_colormap, thing_sprite_prefix,
};
use doom_tui::{DoomApp, DoomEventLoop, RendererMode, TicInput};
use doom_types::weapons::WeaponType;
use doom_types::{Bam, CompatibilityProfile, Fixed16_16};
use doom_wad::WadStack;

#[cfg(test)]
use doom_wad::WadFile;

#[cfg(test)]
use doom_renderer::SwitchList;

use audio_system::{AudioSystem, music_lump_for_map, sound_request_sfx};
use doom_audio::{GenmidiBank, MidiPlayer, MusScore, SfxEmitter, SfxPriority, compute_spatial};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

fn cli_styles() -> clap::builder::styling::Styles {
    use clap::builder::styling::{AnsiColor, Effects, Styles};
    Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default())
        .error(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .valid(AnsiColor::Green.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Yellow.on_default() | Effects::BOLD)
}

fn parse_compatibility_profile(value: &str) -> Result<CompatibilityProfile, &'static str> {
    value.parse()
}

/// Presentation backend selection.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum, Default)]
enum Present {
    /// Render into the terminal via ratatui/crossterm (default).
    #[default]
    Terminal,
    /// Render into a native desktop window (requires the `window` build feature).
    Window,
}

#[derive(Parser, Debug)]
#[command(name = "doom-app", about = "Doom engine (doom-rs)", styles = cli_styles())]
struct Args {
    /// Path to IWAD file (doom1.wad, doom2.wad, freedoom1.wad, etc.).
    #[arg(long, alias = "wad")]
    iwad: std::path::PathBuf,

    /// Optional PWAD overlay(s). Repeat to stack multiple patch WADs.
    #[arg(long)]
    pwad: Vec<std::path::PathBuf>,

    /// Map to load (e.g. E1M1, MAP01). Omit to start at the title screen.
    #[arg(long)]
    warp: Option<String>,

    /// Skill level 1-5 (1=ITYTD, 2=HNTR, 3=HMP, 4=UV, 5=NM). Defaults to 3.
    #[arg(long, default_value = "3")]
    skill: u8,

    /// Compatibility profile for vanilla strictness versus extended behavior.
    #[arg(long, default_value = "extended", value_parser = parse_compatibility_profile)]
    compat: CompatibilityProfile,

    /// Record gameplay to a .lmp demo file (e.g. --record my.lmp)
    #[arg(long)]
    record: Option<std::path::PathBuf>,

    /// Play back a .lmp demo file instead of live input (e.g. --playdemo my.lmp)
    #[arg(long)]
    playdemo: Option<std::path::PathBuf>,

    /// Play back a .lmp demo file as fast as possible to benchmark the engine (e.g. --timedemo my.lmp)
    #[arg(long)]
    timedemo: Option<std::path::PathBuf>,

    /// Path to DeHackEd (.deh) patch file to apply.
    #[arg(long)]
    deh: Option<String>,

    /// Run as a relay server on this port (e.g. --server 5029).
    /// Players connect to this address.  Mutually exclusive with --connect,
    /// --record, --playdemo, and --capture.
    #[arg(long)]
    server: Option<u16>,

    /// Connect to a relay server for netplay (e.g. --connect 127.0.0.1:5029).
    /// Mutually exclusive with --server, --record, --playdemo, and --capture.
    #[arg(long)]
    connect: Option<String>,

    /// Player slot for netplay (1-based, e.g. 1 = player 1).  Defaults to 1.
    #[arg(long, default_value = "1")]
    player: u8,

    /// Total number of players in a netplay session (2-4).  Defaults to 2.
    #[arg(long, default_value = "2")]
    num_players: u8,

    /// Headless capture: advance N game tics, render once, save a BMP, then exit.
    /// May be combined with --playdemo to capture deterministic replay frames.
    /// Example: --playdemo repro.lmp --capture screenshot.bmp
    #[arg(long)]
    capture: Option<std::path::PathBuf>,

    /// Number of game tics to advance before capturing (default: 1).
    #[arg(long, default_value = "1")]
    capture_frames: u32,

    /// Write a plain-text debug event log to this file while playing.
    /// Each line is prefixed with the game tic number.
    /// Example: --debug-log gameplay.log
    #[arg(long)]
    debug_log: Option<std::path::PathBuf>,

    /// Renderer mode: halfblocks, sixel, kitty, iterm2, ascii, braille, shading, blocks.
    /// Default: auto-detect best graphics protocol, fall back to halfblocks.
    /// Graphics protocols silently fall back to halfblocks if unsupported.
    /// Press F2 at runtime to cycle through all modes.
    #[arg(long, default_value = "auto")]
    renderer: String,

    /// Enable turn-based simulation pacing (single-player only).
    #[arg(long)]
    turn_based: bool,

    /// Presentation backend: `terminal` (default) or `window`.
    /// `window` opens a native desktop window and requires the binary to be
    /// built with `--features window`.
    #[arg(long, value_enum, default_value_t = Present::Terminal)]
    present: Present,

    /// Export the level layout and statistics to a standalone HTML report and exit.
    #[arg(long)]
    export_html: Option<std::path::PathBuf>,

    /// Export the level layout and statistics to a standalone JSON file and exit.
    #[arg(long)]
    export_json: Option<std::path::PathBuf>,

    /// Export the level layout to an SVG file and exit.
    #[arg(long)]
    export_svg: Option<std::path::PathBuf>,

    /// Export the level layout to an OBJ 3D model file and exit.
    #[arg(long)]
    export_obj: Option<std::path::PathBuf>,

    /// Export the level layout to a GeoJSON file and exit.
    #[arg(long)]
    export_geojson: Option<std::path::PathBuf>,

    /// Save telemetry data for the session as a GeoJSON file upon exit.
    #[cfg(feature = "telemetry")]
    #[arg(long)]
    telemetry_out: Option<std::path::PathBuf>,

    /// Convert a .lmp demo file to a CSV and exit.
    #[arg(long, num_args = 2, value_names = ["INPUT_LMP", "OUTPUT_CSV"])]
    export_demo_csv: Option<Vec<std::path::PathBuf>>,

    /// Export the sector topological graph to a Graphviz DOT file and exit.
    #[arg(long)]
    export_dot: Option<std::path::PathBuf>,

    /// Export map music as a 16-bit PCM WAV file and exit.
    ///
    /// Uses MUS + GENMIDI + OPL synthesis for source-faithful Doom music.
    #[arg(long)]
    export_music_wav: Option<std::path::PathBuf>,

    /// Number of full music loops to render when using --export-music-wav.
    #[arg(long, default_value = "1")]
    music_loops: u32,

    /// Export a specific sound effect lump as a 16-bit PCM WAV file and exit.
    #[arg(long, requires = "sfx_name")]
    export_sfx_wav: Option<std::path::PathBuf>,

    /// The name of the SFX lump to export (e.g. DSPISTOL) when using --export-sfx-wav.
    #[arg(long, requires = "export_sfx_wav")]
    sfx_name: Option<String>,

    /// Compute the total map statistics (kills, items, secrets, par time) and print them to the console.
    #[arg(long)]
    map_stats: bool,

    /// Find the shortest topological path between two sectors. Provide as "START,END" (e.g. "0,5").
    #[arg(long)]
    pathfind: Option<String>,

    /// Run tactical analysis on the map topology and print chokepoints and isolated areas.
    #[arg(long)]
    analyze: bool,

    /// Print the map statistics or tactical analysis as raw JSON. Only valid when combined with --map-stats, --analyze, or --pathfind.
    #[arg(long)]
    json: bool,

    /// Headless demo-verification harness: replay a demo with NO rendering/audio
    /// and write a per-tic determinism log. SOURCE is either a path to an
    /// external .lmp file OR a lump name (DEMO1/DEMO2/DEMO3) resolved from the
    /// loaded IWAD. Level, skill, and game flags are driven from the demo header.
    #[arg(long, value_name = "SOURCE")]
    verify_demo: Option<String>,

    /// Path to write the per-tic verification CSV (used with --verify-demo).
    #[arg(long)]
    verify_log: Option<std::path::PathBuf>,

    /// Number of times to replay the demo from scratch for the cross-run
    /// determinism self-check (used with --verify-demo). Defaults to 2.
    #[arg(long, default_value = "2")]
    verify_runs: u32,

    /// Path to write a per-draw RNG call-trace CSV (`leveltime,seq,retval,caller`)
    /// captured during demo replay (used with --verify-demo).
    #[arg(long)]
    verify_rng_trace: Option<std::path::PathBuf>,

    /// Path to write a per-tic FULL-ACTOR-STATE dump CSV
    /// (`tic,ord,sprite,frame,x,y,z,momx,momy,momz,angle,health,tics`) — one row
    /// per live mobj, in thinker (creation/generation) order — captured during
    /// demo replay (used with --verify-demo). Byte-diffable against the
    /// instrumented oracle's `$CHOCO_ACTORS_CSV` dump. Off (zero-cost) unless set.
    #[arg(long)]
    verify_actors: Option<std::path::PathBuf>,
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
    /// Timed HUD message queue: (pickup notifications, level names, etc.).
    /// Displayed at the top of the screen and ticks down each frame.
    hud_messages: doom_renderer::HudMessageQueue,
    console: console::Console,
    /// Path used for quick save (F5) and quick load (F9).
    save_path: std::path::PathBuf,
    /// Stateful automap with zoom/pan/follow support.
    automap: AutomapState,
    /// Whether the IDDT cheat has toggled full automap reveal.
    automap_full_reveal: bool,
    /// Optional audio subsystem.  `None` when no audio device is available.
    audio: Option<AudioSystem>,
    /// WAD music lumps keyed by their canonical `D_*` lump names.
    music_library: std::collections::HashMap<doom_wad::lump::LumpName, std::sync::Arc<[u8]>>,
    /// Flat texture cache (floor/ceiling textures loaded from the WAD).
    flat_cache: Option<FlatCache>,
    /// Wall texture cache (TEXTURE1/TEXTURE2 composed textures from the WAD).
    tex_cache: Option<TextureCache>,
    /// Sprite frame cache (loaded from S_START..S_END).
    sprite_cache: Option<SpriteCache>,
    /// Colormap cache (COLORMAP lump, 34 × 256 bytes for light-level shading).
    colormap_cache: Option<ColormapCache>,
    /// Compatibility profile selected at startup.
    compat: CompatibilityProfile,
    /// Animated texture state (flat + wall animation sequences, ticked per tic).
    anim_state: AnimState,
    /// Palette flash controller (pain/pickup/rad-suit full-screen tints).
    palette_flash: PaletteFlash,
    /// Switch texture pair lookup (SW1xxx <-> SW2xxx bidirectional).
    #[cfg(test)]
    switch_list: SwitchList,
    /// Player health from the previous tic — used to detect damage for pain flash.
    prev_health: i32,
    /// First-person view height above the floor, lowered while the player is dead.
    player_view_height: i32,
    /// In-game menu (Esc toggles it).
    menu: doom_game::menu::GameMenu,
    /// Bitmap font for menu/console text rendering.
    bitmap_font: BitmapFont,
    /// WAD patch cache for menu/HUD graphics.
    patch_cache: PatchCache,
    /// WAD-based HU font (STCFN patches). `None` until WAD is attached.
    wad_font: Option<WadFont>,
    /// WAD stack for patch lookups and stacked map loads.
    wad_stack: WadStack,
    /// Mugshot face animation FSM.
    face_state: FaceState,
    /// Title screen state. `Some` = still on title screen, `None` = in gameplay.
    title_screen: Option<TitleScreen>,
    /// Optional plain-text debug event log (opened with --debug-log).
    debug_log: Option<std::fs::File>,
    /// SFX ID for the player pain sound (DSPLPAIN), resolved at startup.
    pain_sfx_id: Option<u16>,
    /// Name → SFX ID lookup built from the WAD at startup (same ordering as
    /// `SfxCache`).  Used to play monster wake/attack/death sounds by lump name.
    sfx_lookup: std::collections::HashMap<doom_wad::lump::LumpName, u16>,
    /// Current skill used when spawning the next map.
    skill: Skill,
    /// Top-level playing/intermission/finale controller.
    phase_controller: GamePhaseController,
    /// Animated intermission tally renderer, active only during intermission.
    intermission_renderer: Option<IntermissionRenderer>,
    /// Previous tic's held attack/use mask for transition skip edge detection.
    transition_buttons_down: u8,
    /// First-person weapon bob/raise/flash controller.
    weapon_anim: WeaponAnimState,
    /// Cogmind-mode rendering state (tile grid + visibility cache).
    cogmind_state: cogmind::CogmindState,
}

const DEAD_PLAYER_VIEW_HEIGHT: i32 = 6;

#[inline]
fn next_player_view_height(current: i32, player_dead: bool) -> i32 {
    if player_dead {
        current.saturating_sub(1).max(DEAD_PLAYER_VIEW_HEIGHT)
    } else {
        PLAYER_HEIGHT
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum WeaponMotion {
    Preserve,
    Reset,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum PlayerStateCarry {
    Carry,
    Reset,
}

impl DoomGame {
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    fn new(
        gs: GameState,
        level: Level,
        audio: Option<AudioSystem>,
        music_library: std::collections::HashMap<doom_wad::lump::LumpName, std::sync::Arc<[u8]>>,
        flat_cache: Option<FlatCache>,
        tex_cache: Option<TextureCache>,
        sprite_cache: Option<SpriteCache>,
        colormap_cache: Option<ColormapCache>,
        show_title: bool,
        debug_log: Option<std::fs::File>,
        pain_sfx_id: Option<u16>,
        sfx_lookup: std::collections::HashMap<doom_wad::lump::LumpName, u16>,
    ) -> Self {
        Self::new_with_compat(
            gs,
            level,
            audio,
            music_library,
            flat_cache,
            tex_cache,
            sprite_cache,
            colormap_cache,
            show_title,
            debug_log,
            pain_sfx_id,
            sfx_lookup,
            CompatibilityProfile::Extended,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_compat(
        mut gs: GameState,
        level: Level,
        audio: Option<AudioSystem>,
        music_library: std::collections::HashMap<doom_wad::lump::LumpName, std::sync::Arc<[u8]>>,
        flat_cache: Option<FlatCache>,
        tex_cache: Option<TextureCache>,
        sprite_cache: Option<SpriteCache>,
        colormap_cache: Option<ColormapCache>,
        show_title: bool,
        debug_log: Option<std::fs::File>,
        pain_sfx_id: Option<u16>,
        sfx_lookup: std::collections::HashMap<doom_wad::lump::LumpName, u16>,
        compat: CompatibilityProfile,
    ) -> Self {
        // Initialize scrolling wall and conveyor belt specials from level linedefs.
        init_scrolling_walls(&mut gs, &level);
        init_conveyors(&mut gs, &level);

        // Initialize dynamic sector lighting (blinking, strobe, fireflicker).
        init_sector_lights(&mut gs, &level);
        // Freeze the setup/gameplay actor-generation boundary (vanilla thinker
        // order for the sector-light pass; see `GameState::tick_world`).
        gs.freeze_thinker_setup_boundary();

        // Capture initial player health for pain-flash delta detection.
        let initial_health = gs.player.health();

        let mut menu = doom_game::menu::GameMenu::new(doom_game::menu::GameVersion::Doom1); // false = Doom 1 mode
        let title_screen = if show_title {
            menu.open();
            Some(TitleScreen::new())
        } else {
            None
        };
        let start_map = Self::map_id_from_level_name(gs.level_name.as_str());
        let phase_controller = if show_title {
            GamePhaseController::new_at_title()
        } else {
            GamePhaseController::new(start_map)
        };

        let mut game = Self {
            gs,
            level,
            cheat_detector: cheats::CheatDetector::new(),
            cheat_buffer: game_cheats::CheatBuffer::new(),
            hud_messages: doom_renderer::HudMessageQueue::new(4),
            console: console::Console::new(),
            save_path: std::path::PathBuf::from("doom_save.bin"),
            automap: AutomapState::new(),
            automap_full_reveal: false,
            audio,
            music_library,
            flat_cache,
            tex_cache,
            sprite_cache,
            colormap_cache,
            compat,
            anim_state: AnimState::new(),
            palette_flash: PaletteFlash::new(),
            #[cfg(test)]
            switch_list: SwitchList::new(),
            prev_health: initial_health,
            player_view_height: PLAYER_HEIGHT,
            menu,
            bitmap_font: BitmapFont::new(),
            patch_cache: PatchCache::new(),
            wad_font: None,
            wad_stack: WadStack::new(),
            face_state: FaceState::new(),
            title_screen,
            debug_log,
            pain_sfx_id,
            sfx_lookup,
            skill: Skill::Medium,
            phase_controller,
            intermission_renderer: None,
            transition_buttons_down: 0,
            weapon_anim: WeaponAnimState::new(),
            cogmind_state: cogmind::CogmindState::new(),
        };

        game.reset_weapon_anim();

        if game.title_screen.is_none() {
            game.start_level_music();
        }

        game
    }

    fn current_fixed_colormap(&self) -> Option<&[u8; 256]> {
        let is_invulnerable =
            self.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] > 0;
        let cache = self.colormap_cache.as_ref()?;
        is_invulnerable.then_some(cache.invulnerability_row(self.compat))
    }

    fn start_music_lump(&self, lump_name: &str) -> bool {
        let Some(audio) = &self.audio else {
            return false;
        };
        let Some(music) = self
            .music_library
            .get(&doom_wad::lump::LumpName::from_str(lump_name))
        else {
            return false;
        };
        audio.start_music(std::sync::Arc::clone(music));
        true
    }

    fn start_level_music(&self) {
        let Some(music_lump) = music_lump_for_map(self.gs.level_name.as_str()) else {
            return;
        };
        let _ = self.start_music_lump(&music_lump);
    }

    fn start_intermission_music(&self, next_map: doom_game::MapId) {
        let primary = if next_map.is_doom2() {
            "D_DM2INT"
        } else {
            "D_INTER"
        };
        let fallback = if next_map.is_doom2() {
            Some("D_INTER")
        } else {
            None
        };

        if self.start_music_lump(primary) {
            return;
        }

        if let Some(fallback) = fallback {
            let _ = self.start_music_lump(fallback);
        }
    }

    fn map_id_from_level_name(level_name: &str) -> doom_game::MapId {
        doom_game::MapId::from_name(level_name).unwrap_or(doom_game::MapId::new(1, 1))
    }

    fn attach_wad_for_transitions(&mut self, skill: Skill, wad_stack: WadStack) {
        self.skill = skill;
        self.wad_stack = wad_stack;
        // Preload menu and status bar patches now that we have a WAD stack.
        self.patch_cache.preload_menu_patches(&self.wad_stack);
        self.patch_cache.preload_statusbar_patches(&self.wad_stack);
        self.wad_font = Some(WadFont::load(&mut self.patch_cache, &self.wad_stack));
    }

    fn enter_title_screen(&mut self) {
        self.title_screen = Some(TitleScreen::new());
        self.menu = doom_game::menu::GameMenu::new(doom_game::menu::GameVersion::Doom1);
        self.menu.open();
        self.intermission_renderer = None;
    }

    fn handle_menu_result(&mut self, result: doom_game::menu::MenuResult) {
        match result {
            doom_game::menu::MenuResult::StartGame { episode: _, skill } => {
                // Map skill index to Skill enum (0=Baby..4=Nightmare).
                let sk = Skill::from_num(skill).unwrap_or(Skill::Medium);
                // Re-spawn the level with the chosen skill.
                self.gs = GameState::new(&self.gs.level_name.clone());
                spawn_level_things(
                    &mut self.gs,
                    &self.level,
                    sk,
                    doom_game::GameMode::SinglePlayer,
                );
                init_scrolling_walls(&mut self.gs, &self.level);
                init_conveyors(&mut self.gs, &self.level);
                init_sector_lights(&mut self.gs, &self.level);
                self.gs.freeze_thinker_setup_boundary();
                self.player_view_height = PLAYER_HEIGHT;
                self.prev_health = self.gs.player.health();
                self.skill = sk;
                self.phase_controller
                    .start_new_game(Self::map_id_from_level_name(self.gs.level_name.as_str()));
                self.intermission_renderer = None;
                self.menu.close();
                self.title_screen = None;
                self.start_level_music();
            }
            doom_game::menu::MenuResult::Quit => {
                // Can't stop the event loop from here; just close the menu.
                self.menu.close();
                self.title_screen = None;
            }
            doom_game::menu::MenuResult::LoadGame(slot) => {
                let path = format!("doom_save_{slot}.bin");
                match savegame::load_game(std::path::Path::new(&path), self.compat) {
                    Ok((_header, payload)) => {
                        if let Err(e) = savegame::apply_save(&mut self.gs, &payload) {
                            self.console.print(format!("Load failed: {e}"));
                            self.hud_messages.push(format!("Load failed: {e}"), 105);
                        } else {
                            self.player_view_height = if self.gs.player.is_dead() {
                                DEAD_PLAYER_VIEW_HEIGHT
                            } else {
                                PLAYER_HEIGHT
                            };
                            self.console.print("Game loaded.".to_string());
                            self.hud_messages.push("Game loaded.".to_string(), 105);
                            self.start_level_music();
                            self.menu.close();
                        }
                    }
                    Err(e) => {
                        self.console.print(format!("Load failed: {e}"));
                        self.hud_messages.push(format!("Load failed: {e}"), 105);
                    }
                }
            }
            doom_game::menu::MenuResult::SaveGame(slot) => {
                let path = format!("doom_save_{slot}.bin");
                if let Err(e) =
                    savegame::save_game(std::path::Path::new(&path), &self.gs, slot, self.compat)
                {
                    self.console.print(format!("Save failed: {e}"));
                    self.hud_messages.push(format!("Save failed: {e}"), 105);
                } else {
                    self.console.print(format!("Saved to slot {slot}."));
                    self.hud_messages
                        .push(format!("Saved to slot {slot}."), 105);
                    self.menu.close();
                }
            }
            _ => {}
        }
    }

    fn update_intermission_renderer(&mut self) {
        match self.phase_controller.phase() {
            GamePhase::Intermission { stats, next_map } => {
                if self.intermission_renderer.is_none() {
                    self.intermission_renderer = Some(IntermissionRenderer::new(
                        stats,
                        self.gs.level_name.as_str(),
                    ));
                    self.start_intermission_music(*next_map);
                }
            }
            _ => {
                self.intermission_renderer = None;
            }
        }
    }

    fn load_map_after_intermission(
        &mut self,
        map_id: doom_game::MapId,
        carry_player_state: PlayerStateCarry,
    ) {
        if !self.wad_stack.has_iwad() {
            self.console
                .print("Cannot load next level: no WAD attached for transitions.".to_string());
            self.phase_controller.clear_load_request();
            return;
        }

        let map_name = map_id.map_name();
        let level = match Level::from_wad_stack(&self.wad_stack, &map_name) {
            Ok(level) => level,
            Err(e) => {
                self.console
                    .print(format!("Failed to load next map {map_name}: {e}"));
                self.phase_controller.clear_load_request();
                return;
            }
        };

        let carried_player =
            (carry_player_state == PlayerStateCarry::Carry).then(|| self.gs.player.clone());
        let mut gs = GameState::new(&map_name);
        let player_handle = spawn_level_things(
            &mut gs,
            &level,
            self.skill,
            doom_game::GameMode::SinglePlayer,
        );

        if let (Some(mut player), Some(handle)) = (carried_player, player_handle) {
            player.handle = handle;
            player.pending_weapon = None;
            player.attack_down = false;
            player.attack_cooldown = 0;
            player.refire = 0;
            player.use_down = false;
            player.bonus_count = 0;
            player.damage_count = 0;
            player.kill_count = 0;
            player.item_count = 0;
            player.secret_count = 0;
            doom_game::weapons::setup_psprites(&mut player);
            gs.player = player;
            gs.sync_player_mobj_health();
        }

        init_scrolling_walls(&mut gs, &level);
        init_conveyors(&mut gs, &level);
        init_sector_lights(&mut gs, &level);
        gs.freeze_thinker_setup_boundary();

        self.gs = gs;
        self.level = level;
        self.player_view_height = PLAYER_HEIGHT;
        self.prev_health = self.gs.player.health();
        self.phase_controller.clear_load_request();
        self.intermission_renderer = None;
        self.reset_weapon_anim();
        self.start_level_music();
    }

    fn reset_weapon_anim(&mut self) {
        self.weapon_anim = WeaponAnimState::new();
        self.ensure_player_psprites_initialized();
        self.sync_weapon_anim_from_player_psprites(WeaponMotion::Reset);
    }

    fn player_weapon_anim_speed(&self) -> i32 {
        let Some(mo) = self.gs.mobjslab.get(self.gs.player.handle) else {
            return 0;
        };
        let momx = i64::from(mo.momx.0);
        let momy = i64::from(mo.momy.0);
        ((momx * momx + momy * momy) as f64).sqrt() as i32
    }

    fn ensure_player_psprites_initialized(&mut self) {
        use doom_game::player::psprite_slots;

        let weapon = self.gs.player.psprites[psprite_slots::WEAPON].state;
        let flash = self.gs.player.psprites[psprite_slots::FLASH].state;
        if weapon == doom_game::StateNum::NULL && flash == doom_game::StateNum::NULL {
            doom_game::weapons::setup_psprites(&mut self.gs.player);
        }
    }

    fn sync_weapon_anim_from_player_psprites(&mut self, motion: WeaponMotion) {
        use doom_game::player::psprite_slots;

        let weapon_psprite = self.gs.player.psprites[psprite_slots::WEAPON];
        let flash_psprite = self.gs.player.psprites[psprite_slots::FLASH];
        let previous_offset =
            (motion == WeaponMotion::Preserve).then_some(self.weapon_anim.raise_offset);

        self.weapon_anim.current.sx = weapon_psprite.sx;
        self.weapon_anim.current.sprite_name =
            psprite_patch_name(weapon_psprite.state).unwrap_or(*b"PISGA0\0\0");
        self.weapon_anim.current.flash_active = flash_psprite.state != doom_game::StateNum::NULL;
        self.weapon_anim.current.flash_sprite =
            psprite_patch_name(flash_psprite.state).unwrap_or([0; 8]);
        self.weapon_anim.current.flash_tics = flash_psprite.tics.max(0) as u32;
        self.weapon_anim.current.full_bright = self.weapon_anim.current.flash_active
            || psprite_state_is_fullbright(weapon_psprite.state);
        // Gameplay psprite `sy` rests at `WEAPON_TOP` (vanilla `WEAPONTOP`, now
        // stored in 16.16 fixed point), but the renderer's `raise_offset`
        // baseline is 0 = fully raised, in integer pixels.  Subtract the
        // gameplay resting offset and shift down from fixed point to pixels so a
        // rested weapon draws at the same on-screen position as before
        // (raise_offset 0), preserving the existing view calibration.
        let raise_offset = (weapon_psprite.sy - doom_game::weapons::WEAPON_TOP) >> 16;
        self.weapon_anim.raise_offset = raise_offset;

        let transition = psprite_transition(self.gs.player.weapon, weapon_psprite.state);

        if transition != WeaponTransition::None {
            self.weapon_anim.current.transition = transition;
        } else if let Some(previous_offset) = previous_offset {
            if raise_offset < previous_offset {
                self.weapon_anim.current.transition = WeaponTransition::Raising;
            } else if raise_offset > previous_offset {
                self.weapon_anim.current.transition = WeaponTransition::Lowering;
            } else {
                self.weapon_anim.current.transition = WeaponTransition::None;
            }
        } else {
            self.weapon_anim.current.transition = WeaponTransition::None;
        }
    }

    fn tick_weapon_anim(&mut self) {
        self.ensure_player_psprites_initialized();
        if self.gs.player.is_dead() {
            self.weapon_anim.bob.reset();
            self.sync_weapon_anim_from_player_psprites(WeaponMotion::Reset);
            return;
        }

        self.weapon_anim.bob.tick(self.player_weapon_anim_speed());
        self.sync_weapon_anim_from_player_psprites(WeaponMotion::Preserve);
    }

    fn transition_input_pressed(&mut self, input: &TicInput) -> bool {
        let button_mask = input.buttons & (doom_types::bt::BT_ATTACK | doom_types::bt::BT_USE);
        let button_pressed = button_mask != 0 && self.transition_buttons_down == 0;
        self.transition_buttons_down = button_mask;
        input.menu_select || input.escape_pressed || button_pressed
    }
}

impl DoomGame {
    fn handle_sound_events(&mut self, events: impl IntoIterator<Item = doom_game::SoundRequest>) {
        use doom_game::SoundRequest;

        let Some(ref audio) = self.audio else {
            for ev in events {
                if let SoundRequest::PlayerUseLockedDoor(color) = ev {
                    self.hud_messages
                        .push(locked_door_message(color).to_string(), 105);
                }
            }
            return;
        };

        let (pl_x, pl_y, pl_angle) = self
            .gs
            .mobjslab
            .get(self.gs.player.handle)
            .map(|mo| (mo.x, mo.y, mo.angle))
            .unwrap_or_default();
        let player_origin = Some(self.gs.player.handle);

        for ev in events {
            if let SoundRequest::PlayerUseLockedDoor(color) = ev {
                self.hud_messages
                    .push(locked_door_message(color).to_string(), 105);
            }
            let Some((lump, priority)) = sound_request_sfx(ev) else {
                continue;
            };

            let emitter = ev.emitter(pl_x, pl_y);
            let origin = ev.origin_handle(player_origin);

            if lump.is_empty() {
                continue;
            }

            if let Some(&id) = self
                .sfx_lookup
                .get(&doom_wad::lump::LumpName::from_str(lump))
            {
                if let Some((emitter_x, emitter_y)) = emitter {
                    let spatial = compute_spatial(
                        &SfxEmitter {
                            x: emitter_x,
                            y: emitter_y,
                        },
                        pl_x,
                        pl_y,
                        pl_angle,
                    );

                    if spatial.volume < 0.01 {
                        continue;
                    }

                    audio.play_sfx(id, priority, spatial.volume, spatial.pan, origin);
                } else {
                    audio.play_sfx(id, priority, 1.0, 0.0, origin);
                }
            }
        }
    }

    /// Write a plain-text event line to the debug log (if active).
    ///
    /// Format: `tic=<N> <msg>\n`  — no ANSI codes, no box drawing.
    fn dlog(&mut self, msg: &str) {
        if let Some(ref mut f) = self.debug_log {
            use std::io::Write;
            let _ = writeln!(f, "tic={} {}", self.gs.tic_num, msg);
        }
    }

    /// Snapshot player state to the log (position, health, armor, ammo, weapon).
    fn dlog_player_snapshot(&mut self) {
        if self.debug_log.is_none() {
            return;
        }
        let (px, py, pa) = self
            .gs
            .mobjslab
            .get(self.gs.player.handle)
            .map(|mo| (mo.x.to_int(), mo.y.to_int(), mo.angle.0))
            .unwrap_or((0, 0, 0));
        let hp = self.gs.player.health();
        let arm = self.gs.player.armor();
        let kills = self.gs.player.kill_count;
        let weapon = self.gs.player.weapon;
        use doom_types::weapons::{AmmoType, WEAPON_AMMO};
        let cur_ammo_type = WEAPON_AMMO[weapon as usize];
        let cur_ammo = if cur_ammo_type == AmmoType::None {
            u32::MAX
        } else {
            self.gs.player.ammo(cur_ammo_type as usize)
        };
        let ammo_str = if cur_ammo == u32::MAX {
            "inf".to_owned()
        } else {
            cur_ammo.to_string()
        };
        let msg = format!(
            "player pos=({},{}) angle={:#010x} health={} armor={} kills={} weapon={:?} ammo={}",
            px, py, pa, hp, arm, kills, weapon, ammo_str
        );
        self.dlog(&msg);
    }

    /// Log the state of all live enemies.
    ///
    /// Include enough AI state to distinguish "never woke up" from
    /// "woke up but got stuck on movement/pathing".
    fn dlog_live_enemies(&mut self) {
        if self.debug_log.is_none() {
            return;
        }

        // Iterate by index to avoid allocating a vector while still dropping
        // the immutable borrow of `self.gs` before calling `self.dlog`.
        let slot_count = self.gs.mobjslab.slot_count();
        for i in 0..slot_count {
            let msg = {
                let Some(h) = self.gs.mobjslab.handle_at(i) else {
                    continue;
                };
                let Some(mo) = self.gs.mobjslab.get(h) else {
                    continue;
                };
                if mo.flags & doom_game::mobj::flags::MF_COUNTKILL == 0 {
                    continue;
                }
                let ex = mo.x.to_int();
                let ey = mo.y.to_int();
                let state_idx = mo.state.0;
                let flags = mo.flags;
                let is_dead = mo.health <= 0;
                let target = mo.target;
                format!(
                    "enemy idx={} gen={} {:?} pos=({},{}) health={} state={} tics={} dead={} flags={:#010x} target=({}, {}) threshold={} reaction={} movecount={} subsector={}",
                    h.index,
                    h.generation,
                    mo.kind,
                    ex,
                    ey,
                    mo.health,
                    state_idx,
                    mo.tics,
                    is_dead,
                    flags,
                    target.index,
                    target.generation,
                    mo.threshold,
                    mo.reactiontime,
                    mo.movecount,
                    mo.subsector,
                )
            };
            self.dlog(&msg);
        }
    }

    /// Log death events — enemies that fired A_Scream this tic (SCREAMED flag set).
    /// Clears the flag after logging so each death is logged exactly once.
    fn dlog_death_events(&mut self) {
        if self.debug_log.is_none() {
            return;
        }

        // Iterate by index to avoid allocating a vector while still dropping
        // the borrow of `self.gs` before calling `self.dlog`.
        let slot_count = self.gs.mobjslab.slot_count();
        for i in 0..slot_count {
            let msg = {
                let Some(h) = self.gs.mobjslab.handle_at(i) else {
                    continue;
                };
                let Some(mo) = self.gs.mobjslab.get_mut(h) else {
                    continue;
                };
                if mo.flags & doom_game::mobj::flags::MF_SCREAMED != 0 {
                    // Clear the flag so we only log once.
                    mo.flags &= !doom_game::mobj::flags::MF_SCREAMED;
                    let kind = mo.kind;
                    let x = mo.x.to_int();
                    let y = mo.y.to_int();
                    let state_idx = mo.state.0;
                    Some(format!(
                        "enemy_died {:?} pos=({},{}) death_state={}",
                        kind, x, y, state_idx
                    ))
                } else {
                    None
                }
            };
            if let Some(m) = msg {
                self.dlog(&m);
            }
        }
    }
}

impl DoomApp for DoomGame {
    fn tick(&mut self, input: TicInput) {
        let transition_pressed = self.transition_input_pressed(&input);

        // --- Title screen mode ---
        // While the title screen is showing, route input to the menu and skip
        // all game simulation.  StartGame dismisses the title screen.
        if let Some(ref mut ts) = self.title_screen {
            ts.tick();
            self.menu.tick();

            // Edge-triggered navigation: Up/Down arrows, Escape = back.
            if input.menu_up {
                self.menu.move_up();
            } else if input.menu_down {
                self.menu.move_down();
            }
            if input.escape_pressed {
                self.menu.back();
            }

            // Enter = select; Backspace = back.
            if input.menu_select {
                if let Some(result) = self.menu.select() {
                    self.handle_menu_result(result);
                }
            } else if let Some('\x08') = input.console_char {
                // Backspace = back in menu (alternative to Escape).
                self.menu.back();
            }
            return;
        }

        match self.phase_controller.phase() {
            GamePhase::Intermission { .. } => {
                if let Some(renderer) = self.intermission_renderer.as_mut() {
                    if transition_pressed {
                        if renderer.is_done() {
                            self.phase_controller.request_skip();
                        } else {
                            renderer.skip();
                        }
                    } else {
                        renderer.tick();
                    }
                }

                self.phase_controller.tick(&mut self.gs);
                if let Some(map_id) = self.phase_controller.should_load_map() {
                    self.load_map_after_intermission(map_id, PlayerStateCarry::Carry);
                }
                self.update_intermission_renderer();
                return;
            }
            GamePhase::Finale { .. } => {
                if transition_pressed {
                    self.phase_controller.request_skip();
                }
                self.phase_controller.tick(&mut self.gs);
                if matches!(self.phase_controller.phase(), GamePhase::TitleScreen) {
                    self.enter_title_screen();
                }
                return;
            }
            GamePhase::TitleScreen | GamePhase::Playing => {}
        }

        // Tick menu skull animation each tic regardless of menu state.
        self.menu.tick();

        // Handle console / cheat input before forwarding movement to the
        // game simulation.
        // Escape: toggle the in-game menu (when console is not open).
        if input.escape_pressed && !self.console.visible {
            if self.menu.is_active() {
                self.menu.close();
            } else {
                self.menu.open();
            }
        }

        // Also handle in-game menu navigation via edge-triggered keys.
        if self.menu.is_active() {
            if input.menu_up {
                self.menu.move_up();
            } else if input.menu_down {
                self.menu.move_down();
            }
            if input.menu_select {
                if let Some(result) = self.menu.select() {
                    self.handle_menu_result(result);
                }
            }
        }

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
                            format!("(ERR) UNKNOWN COMMAND {}", line)
                        } else {
                            format!("(OK) {}", msg)
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
                        self.hud_messages.push(msg.to_string(), 105); // 3 sec @ 35 tics/sec
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
                self.hud_messages.push(msg, 105); // 3 seconds at 35 tics/sec
                self.cheat_buffer.clear();
            }
        }

        // Tick down cheat message timer.
        self.hud_messages.tick();

        // Quick save (F5).
        if input.f5_save {
            if let Err(e) = savegame::save_game(&self.save_path, &self.gs, 0, self.compat) {
                self.console.print(format!("Save failed: {e}"));
                self.hud_messages.push(format!("Save failed: {e}"), 105);
            } else {
                self.console.print("Game saved.".to_string());
                self.hud_messages.push("Game saved.".to_string(), 105);
            }
        }

        // Quick load (F9).
        if input.f9_load {
            match savegame::load_game(&self.save_path, self.compat) {
                Ok((_header, payload)) => {
                    if let Err(e) = savegame::apply_save(&mut self.gs, &payload) {
                        self.console.print(format!("Load failed: {e}"));
                        self.hud_messages.push(format!("Load failed: {e}"), 105);
                    } else {
                        self.reset_weapon_anim();
                        self.console.print("Game loaded.".to_string());
                        self.hud_messages.push("Game loaded.".to_string(), 105);
                        self.start_level_music();
                    }
                }
                Err(e) => {
                    self.console.print(format!("Load failed: {e}"));
                    self.hud_messages.push(format!("Load failed: {e}"), 105);
                }
            }
        }

        let cmd = crate::net_mode::ticinput_to_ticcmd(input);

        // Pause the game simulation while the menu is open during gameplay.
        // Title screen and intermission handle their own timing; only Playing
        // needs the pause.
        let paused =
            self.menu.is_active() && matches!(self.phase_controller.phase(), GamePhase::Playing);

        // Snapshot kill/item counts before the tick to detect changes.
        let pre_kills = self.gs.player.kill_count;
        let pre_items = self.gs.player.item_count;

        if !paused {
            self.gs.tick(cmd, Some(&mut self.level));
            self.player_view_height =
                next_player_view_height(self.player_view_height, self.gs.player.is_dead());
            self.tick_weapon_anim();
            self.phase_controller.tick(&mut self.gs);
        }
        self.update_intermission_renderer();
        if let Some(map_id) = self.phase_controller.should_load_map() {
            self.load_map_after_intermission(map_id, PlayerStateCarry::Carry);
        }

        // Drain the game's sound event queue.  Each event maps to a DS* lump
        // name and a priority.  The SfxMixer's 8-channel priority system handles
        // contention — weapon-priority sounds always win; monster sounds compete
        // with each other, matching Doom's original S_StartSound behaviour.
        {
            let events = std::mem::take(&mut self.gs.sound.sound_queue);

            #[cfg(feature = "sound_ripples")]
            if self.title_screen.is_none() {
                self.cogmind_state
                    .effects
                    .spawn_sound_ripples(&events, &self.gs);
            }
            self.handle_sound_events(events);
        }

        // Log kill and item events.
        if self.debug_log.is_some() {
            if self.gs.player.kill_count > pre_kills {
                let msg = format!("kill count={}", self.gs.player.kill_count);
                self.dlog(&msg);
            }
            if self.gs.player.item_count > pre_items {
                let msg = format!("pickup items={}", self.gs.player.item_count);
                self.dlog(&msg);
            }
        }

        // Advance animated texture state (flat and wall animations).
        self.anim_state.tick();

        // Advance palette flash timer (pain/pickup/rad-suit tints).
        self.palette_flash.tick();

        // Detect player damage and trigger a pain flash + hurt sound.
        {
            let cur_health = self.gs.player.health();
            if cur_health < self.prev_health {
                let damage = self.prev_health - cur_health;
                // Pain palette indices 1-8 (increasing red tint).
                // Simple formula: one palette step per 8 HP lost, clamped.
                let palette = ((damage / 8) as usize).clamp(1, 8);
                self.palette_flash.trigger(palette, 12);
                // Signal face FSM about damage.
                self.face_state.on_damage(damage, Bam::ZERO);
                // Play DSPLPAIN on any damage taken.
                if let (Some(audio), Some(sfx_id)) = (&self.audio, self.pain_sfx_id) {
                    audio.play_sfx(
                        sfx_id,
                        SfxPriority::High,
                        1.0,
                        0.0,
                        Some(self.gs.player.handle),
                    );
                }
                if self.debug_log.is_some() {
                    let msg = format!("damage -{} health={}", damage, cur_health);
                    self.dlog(&msg);
                }
            }
            // Tick face FSM every tic.
            {
                let is_firing = self.gs.player.attack_down;
                let is_invulnerable =
                    self.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] > 0;
                self.face_state
                    .tick(cur_health, is_firing, is_invulnerable, None);
            }
            if self.debug_log.is_some() {
                // Log player snapshot every 35 tics (once per second of gametime).
                if self.gs.tic_num.is_multiple_of(35) {
                    self.dlog_player_snapshot();
                    self.dlog_live_enemies();
                }
                // Log any deaths that fired A_Scream this tic.
                self.dlog_death_events();
            }
            self.prev_health = cur_health;
        }

        // Update automap center to follow the player position.
        if self.automap.active {
            if let Some(mo) = self.gs.mobjslab.get(self.gs.player.handle) {
                self.automap.update_center(mo.x.to_int(), mo.y.to_int());
            }
        }
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        // Title screen mode: draw the title/credits screen + menu overlay.
        if let Some(ref ts) = self.title_screen {
            draw_title_screen_wad(
                fb,
                ts,
                &mut self.patch_cache,
                &self.wad_stack,
                &self.bitmap_font,
            );
            draw_menu_wad(
                fb,
                &self.menu,
                &mut self.patch_cache,
                &self.wad_stack,
                &self.bitmap_font,
            );
            return;
        }

        match self.phase_controller.phase() {
            GamePhase::Intermission { .. } => {
                if let Some(renderer) = self.intermission_renderer.as_ref() {
                    if self.wad_stack.lump_data("WIOSTK").is_some() {
                        draw_intermission_wad(fb, &mut self.patch_cache, &self.wad_stack, renderer);
                    } else {
                        fb.clear(0);
                        draw_intermission(fb, renderer);
                    }
                } else {
                    fb.clear(0);
                }
                return;
            }
            GamePhase::Finale { text_index, tic } => {
                let map = self.phase_controller.current_map();
                let episode = if map.is_doom2() { 0 } else { map.episode };
                if self.wad_stack.lump_data("PFUB1").is_some()
                    || self.wad_stack.lump_data("INTERPIC").is_some()
                {
                    draw_finale_wad(
                        fb,
                        &mut self.patch_cache,
                        &self.wad_stack,
                        &self.bitmap_font,
                        episode,
                        *text_index,
                        *tic,
                    );
                } else {
                    fb.clear(0);
                    draw_mini_string(fb, 96, "THE END", 176);
                }
                return;
            }
            GamePhase::TitleScreen => {
                fb.clear(0);
                return;
            }
            GamePhase::Playing => {}
        }

        let handle = self.gs.player.handle;
        let (px, py, angle) = if let Some(mo) = self.gs.mobjslab.get(handle) {
            (mo.x.to_int(), mo.y.to_int(), mo.angle)
        } else {
            (0, 0, Bam::ZERO)
        };

        let palette = PaletteLut::grayscale();
        let colormap_cache = self.colormap_cache.as_ref();
        let fixed_colormap = self.current_fixed_colormap();

        if self.automap.active {
            // Draw the overhead automap using the stateful AutomapState
            // (supports zoom/pan/follow). automap_full_reveal (toggled by
            // IDDT) is wired for future use; the current automap
            // implementation already shows all linedefs.
            draw_automap_ex(fb, &self.level, &self.automap, px, py, angle);

            // Draw status bar over the bottom of the automap.
            {
                let data = doom_renderer::StatusBarData::from_player(&self.gs.player);
                draw_status_bar_wad(
                    fb,
                    &mut self.patch_cache,
                    &self.wad_stack,
                    &data,
                    &self.face_state,
                );
            }
        } else {
            // Draw the first-person 3D view.
            // We pass a grayscale palette; render_level currently ignores it
            // (wall colors are derived from light levels only).
            let render_out = render_level_with_view_height_and_extra_light_and_fixed_colormap(
                &self.level,
                px,
                py,
                angle,
                self.player_view_height,
                fb,
                &palette,
                self.flat_cache.as_ref(),
                self.tex_cache.as_ref(),
                colormap_cache,
                None,
                false,
                fixed_colormap,
                self.gs.player.extra_light,
            );

            let RenderOut {
                z_buf,
                clip_top,
                clip_bot,
                clip_top_depth,
                clip_bot_depth,
                clip_top_history,
                clip_bot_history,
                masked_columns,
            } = render_out;

            // Project live mobj positions as state-driven billboard sprites.
            // Uses ActorRenderInfo so animations play correctly.
            if let Some(ref cache) = self.sprite_cache {
                let player_handle = self.gs.player.handle;
                let actors_iter = self
                    .gs
                    .mobjslab
                    .iter_handles()
                    .filter(|&h| h != player_handle)
                    .filter_map(|h| self.gs.mobjslab.get(h))
                    .filter_map(|mo| {
                        let state_entry = doom_game::STATES.get(mo.state.0 as usize);
                        let (sprite, frame) = state_entry
                            .map(|s| (s.sprite, s.frame))
                            .unwrap_or((doom_game::states::sprite_names::SPR_NONE, 0));
                        // For items whose spawn_state is S_NULL (sprite = SPR_NONE),
                        // fall back to the DoomEd-type-derived prefix so they still render.
                        let fallback_prefix = if sprite == doom_game::states::sprite_names::SPR_NONE
                        {
                            kind_to_doomed_type(mo.kind).and_then(thing_sprite_prefix)
                        } else {
                            None
                        };
                        // Skip completely if no sprite and no fallback.
                        if sprite == doom_game::states::sprite_names::SPR_NONE
                            && fallback_prefix.is_none()
                        {
                            return None;
                        }
                        Some(ActorRenderInfo {
                            x: mo.x.0,
                            y: mo.y.0,
                            z: mo.z.0,
                            angle: mo.angle.0,
                            sprite,
                            frame,
                            height: mo.height.0,
                            render_flag: render_flag_from_state(frame, mo.flags),
                            fallback_prefix,
                        })
                    });
                render_actors_with_masked_and_fixed_colormap_ex(
                    actors_iter,
                    &self.level,
                    Fixed16_16::from_int(px),
                    Fixed16_16::from_int(py),
                    angle,
                    fb,
                    cache,
                    Some(&z_buf),
                    colormap_cache,
                    Some(SpriteClip {
                        top: &clip_top,
                        bottom: &clip_bot,
                        top_depth: &clip_top_depth,
                        bottom_depth: &clip_bot_depth,
                        top_history: Some(&clip_top_history),
                        bottom_history: Some(&clip_bot_history),
                    }),
                    Some(&masked_columns),
                    fixed_colormap,
                );
            }

            // Draw weapon sprite overlay using the player's current weapon.
            if let Some(ref cache) = self.sprite_cache
                && !self.gs.player.is_dead()
            {
                draw_weapon_animated_with_override(
                    fb,
                    &self.weapon_anim,
                    cache,
                    &IDENTITY_COLORMAP,
                    fixed_colormap,
                );
            }

            // Draw HUD status bar over the bottom 32 rows.
            {
                let data = doom_renderer::StatusBarData::from_player(&self.gs.player);
                draw_status_bar_wad(
                    fb,
                    &mut self.patch_cache,
                    &self.wad_stack,
                    &data,
                    &self.face_state,
                );
            }
        }

        // Draw cheat message overlay at the top of the screen (if active).
        if !self.hud_messages.is_empty() {
            draw_cheat_message_overlay(fb, &self.hud_messages);
        }

        // Draw menu overlay on top of the game view (no-op when menu is not active).
        draw_menu_wad(
            fb,
            &self.menu,
            &mut self.patch_cache,
            &self.wad_stack,
            &self.bitmap_font,
        );

        // Draw console overlay on top of everything (highest priority).
        if self.console.visible {
            draw_console_overlay(fb, &self.console);
        }
    }

    fn active_palette(&self) -> usize {
        self.palette_flash.active_palette()
    }

    fn render_cogmind(&mut self, term_w: u16, term_h: u16) -> Option<doom_tui::CogmindFrame> {
        // Not available on the title screen.
        if self.title_screen.is_some() {
            return None;
        }

        // Ensure grid is built for the current level.
        self.cogmind_state.ensure_grid(&self.level);

        // Find the player's sector via BSP lookup.
        let player_mobj = self.gs.mobjslab.get(self.gs.player.handle)?;
        let px = player_mobj.x.to_int();
        let py = player_mobj.y.to_int();

        if let Some(sector_idx) = self.level.sector_index_at(px, py) {
            self.cogmind_state
                .update_visibility(sector_idx, &self.level);
        }

        Some(
            self.cogmind_state
                .render_frame(&self.gs, &self.level, term_w, term_h),
        )
    }

    fn cogmind_hud(&self) -> Option<doom_tui::CogmindHud> {
        if self.title_screen.is_some() {
            return None;
        }
        use doom_game::player::{
            KEY_BLUE_CARD, KEY_BLUE_SKULL, KEY_RED_CARD, KEY_RED_SKULL, KEY_YELLOW_CARD,
            KEY_YELLOW_SKULL,
        };
        use doom_types::weapons::{AmmoType, WEAPON_AMMO};

        let p = &self.gs.player;
        let ammo_type = WEAPON_AMMO[p.weapon as usize];
        let (ammo, max_ammo) = if ammo_type != AmmoType::None {
            (
                Some(p.ammo(ammo_type as usize)),
                Some(p.max_ammo[ammo_type as usize]),
            )
        } else {
            (None, None)
        };

        let weapon_name = match p.weapon {
            doom_types::weapons::WeaponType::Fist => "FIST",
            doom_types::weapons::WeaponType::Pistol => "PIST",
            doom_types::weapons::WeaponType::Shotgun => "SG",
            doom_types::weapons::WeaponType::Chaingun => "CG",
            doom_types::weapons::WeaponType::RocketLauncher => "RL",
            doom_types::weapons::WeaponType::PlasmaRifle => "PLAS",
            doom_types::weapons::WeaponType::Bfg => "BFG",
            doom_types::weapons::WeaponType::Chainsaw => "SAW",
            doom_types::weapons::WeaponType::SuperShotgun => "SSG",
        };

        Some(doom_tui::CogmindHud {
            health: p.health(),
            max_health: 100,
            armor: p.armor(),
            ammo,
            max_ammo,
            weapon_name,
            keys: [
                p.keys & KEY_BLUE_CARD != 0,
                p.keys & KEY_YELLOW_CARD != 0,
                p.keys & KEY_RED_CARD != 0,
                p.keys & KEY_BLUE_SKULL != 0,
                p.keys & KEY_YELLOW_SKULL != 0,
                p.keys & KEY_RED_SKULL != 0,
            ],
            kill_count: p.kill_count,
            total_monsters: self.gs.stats.total_kills,
            level_name: self.gs.level_name.clone(),
            #[cfg(feature = "style_meter")]
            style_rank: Some(self.gs.style.rank()),
        })
    }
}

// ---------------------------------------------------------------------------
// Console overlay
// ---------------------------------------------------------------------------

/// Draw the console overlay onto the top ~80 rows of the framebuffer.
///
/// The console is drawn on top of everything else (highest priority).
/// Layout:
///   - Rows 0..80: darkened background panel
///   - Row 2:      "--- CONSOLE ---" header (yellow)
///   - Rows 10+:   recent messages (newest first, white)
///   - Row 70:     "> input_" prompt line (green)
fn draw_console_overlay(fb: &mut Framebuffer, console: &console::Console) {
    const FB_W: usize = 320;
    const PANEL_H: usize = 80; // console panel height in pixels
    const CHAR_H: usize = 7; // 6px glyph + 1px gap
    const COLOR_HEADER: u8 = 231; // yellow — "--- CONSOLE ---"
    const COLOR_MSG: u8 = 200; // light — message lines
    const COLOR_PROMPT: u8 = 112; // green-ish — "> input_"
    const COLOR_BG: u8 = 4; // dark blue-gray panel

    // Darken the top PANEL_H rows to form the console background.
    for y in 0..PANEL_H {
        let row_start = y * FB_W;
        let row_end = row_start + FB_W;
        if row_end <= fb.data.len() {
            for px in &mut fb.data[row_start..row_end] {
                *px = px.wrapping_shr(1).saturating_add(COLOR_BG / 4);
            }
        }
    }

    // Draw "--- CONSOLE ---" header at the top.
    draw_mini_string(fb, 2, "--- CONSOLE ---", COLOR_HEADER);

    // Draw recent messages (up to 8), newest first.
    // Iterating directly avoids a `.collect::<Vec<_>>()` allocation per frame.
    for (i, msg) in console
        .messages
        .iter()
        .rev()
        .take(8)
        .map(|s| s.as_str())
        .enumerate()
    {
        let y = 10 + i * CHAR_H;
        if y + CHAR_H > PANEL_H {
            break;
        }
        draw_mini_string(fb, y, msg, COLOR_MSG);
    }

    // Draw "> input_" prompt at the bottom of the panel.
    let prompt_y = PANEL_H.saturating_sub(CHAR_H + 2);
    // ⚡ Bolt Optimization:
    // Avoids an unnecessary `format!` string allocation per frame by
    // chaining iterators and drawing the characters directly.
    draw_mini_string_chained(
        fb,
        prompt_y,
        "> ".chars()
            .chain(console.input.chars())
            .chain(std::iter::once('_')),
        COLOR_PROMPT,
    );
}

fn draw_mini_string_chained(
    fb: &mut Framebuffer,
    y: usize,
    chars: impl Iterator<Item = char>,
    color: u8,
) {
    const FB_W: usize = 320;
    const CHAR_W: usize = 5;
    const GLYPH_ROWS: usize = 6;

    for (ci, ch) in chars.enumerate() {
        let glyph = mini_glyph(ch);
        let cx = 2 + ci * CHAR_W;
        for (row, &bits) in glyph.iter().enumerate().take(GLYPH_ROWS) {
            let sy = y + row;
            if sy >= 200 {
                break;
            }
            for col in 0..4 {
                if bits & (1 << (3 - col)) != 0 {
                    let sx = cx + col;
                    if sx < FB_W {
                        fb.set_pixel(sx, sy, color);
                    }
                }
            }
        }
    }
}

/// Draw a string using the mini 4x6 glyph font at `(2, y)`.
///
/// Characters that overflow the 320-pixel width are clipped.
fn draw_mini_string(fb: &mut Framebuffer, y: usize, text: &str, color: u8) {
    draw_mini_string_chained(fb, y, text.chars(), color);
}

// ---------------------------------------------------------------------------
// Cheat message overlay
// ---------------------------------------------------------------------------

/// Draw the HUD messages overlay at the top of the framebuffer.
///
/// Uses a minimal 4x6 bitmap font to render ASCII text. Each character cell
/// is 5 pixels wide (4px glyph + 1px spacing). The messages are rendered in
/// yellow (palette index 231) on a black (palette index 0) background bar.
fn draw_cheat_message_overlay(fb: &mut Framebuffer, msgs: &doom_renderer::HudMessageQueue) {
    const FB_W: usize = 320;
    const CHAR_W: usize = 5; // 4px glyph + 1px gap
    const CHAR_H: usize = 7; // 6px glyph + 1px gap
    const TOP_Y: usize = 2;
    const COLOR_MSG: u8 = 231; // yellow
    const COLOR_MSG_BG: u8 = 0; // black

    let active_msgs = msgs.active_messages();
    if active_msgs.is_empty() {
        return;
    }

    // Draw a background bar across the top of the screen that covers all active messages.
    let bar_h = (active_msgs.len() * CHAR_H) + 2; // 1px padding top+bottom
    for y in 0..bar_h {
        let row_start = y * FB_W;
        let row_end = row_start + FB_W;
        if row_end <= fb.data.len() {
            fb.data[row_start..row_end].fill(COLOR_MSG_BG);
        }
    }

    // Render each message vertically.
    for (i, msg) in active_msgs.iter().enumerate() {
        let text = msg.text();
        // Center the message horizontally.
        let msg_width_px = text.len() * CHAR_W;
        let start_x = if msg_width_px < FB_W {
            (FB_W - msg_width_px) / 2
        } else {
            0
        };

        for (ci, ch) in text.chars().enumerate() {
            let glyph = mini_glyph(ch);
            let cx = start_x + ci * CHAR_W;
            for (row, &bits) in glyph.iter().enumerate() {
                let sy = TOP_Y + (i * CHAR_H) + row;
                if sy >= 200 {
                    break;
                }
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
}

fn locked_door_message(color: LockedDoorColor) -> &'static str {
    match color {
        LockedDoorColor::Blue => "You need a blue key to open this door",
        LockedDoorColor::Red => "You need a red key to open this door",
        LockedDoorColor::Yellow => "You need a yellow key to open this door",
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
        _ => [0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b0000],
    }
}

// ---------------------------------------------------------------------------
// Weapon sprite mapping
// ---------------------------------------------------------------------------

fn psprite_state_is_fullbright(state: doom_game::StateNum) -> bool {
    if state == doom_game::StateNum::NULL {
        return false;
    }

    STATES
        .get(state.0 as usize)
        .map(|entry| entry.frame & 0x80 != 0)
        .unwrap_or(false)
}

fn psprite_patch_name(state: doom_game::StateNum) -> Option<[u8; 8]> {
    if state == doom_game::StateNum::NULL {
        return None;
    }

    let entry = STATES.get(state.0 as usize)?;
    if entry.sprite == doom_game::sprite_names::SPR_NONE {
        return None;
    }

    let sprite_index = entry.sprite as usize;
    let sprite_name = *doom_game::sprite_names::SPRITE_NAMES.get(sprite_index)?;
    let frame_index = entry.frame & 0x7f;
    if frame_index >= 26 {
        return None;
    }

    let mut lump = [0u8; 8];
    lump[..4].copy_from_slice(sprite_name.as_bytes());
    lump[4] = b'A' + frame_index;
    lump[5] = b'0';
    Some(lump)
}

fn psprite_transition(weapon: WeaponType, state: doom_game::StateNum) -> WeaponTransition {
    use doom_game::states::ids;

    let (up, down) = match weapon {
        WeaponType::Fist => (ids::S_PUNCH_UP, ids::S_PUNCH_DOWN),
        WeaponType::Pistol => (ids::S_PISTOL_UP, ids::S_PISTOL_DOWN),
        WeaponType::Shotgun => (ids::S_SGUN_UP, ids::S_SGUN_DOWN),
        WeaponType::Chaingun => (ids::S_CHAIN_UP, ids::S_CHAIN_DOWN),
        WeaponType::RocketLauncher => (ids::S_MISSILE_UP, ids::S_MISSILE_DOWN),
        WeaponType::PlasmaRifle => (ids::S_PLASMA_UP, ids::S_PLASMA_DOWN),
        WeaponType::Bfg => (ids::S_BFG_UP, ids::S_BFG_DOWN),
        WeaponType::Chainsaw => (ids::S_SAW_UP, ids::S_SAW_DOWN),
        WeaponType::SuperShotgun => (ids::S_DSGUN_UP, ids::S_DSGUN_DOWN),
    };

    if state == doom_game::StateNum(up) {
        WeaponTransition::Raising
    } else if state == doom_game::StateNum(down) {
        WeaponTransition::Lowering
    } else {
        WeaponTransition::None
    }
}

// ---------------------------------------------------------------------------
// Input conversion
fn load_music_library(
    wad: &WadStack,
) -> std::collections::HashMap<doom_wad::lump::LumpName, std::sync::Arc<[u8]>> {
    let mut music_library = std::collections::HashMap::new();

    for episode in 1..=4 {
        for map in 1..=9 {
            let map_name = format!("E{episode}M{map}");
            let Some(music_lump) = music_lump_for_map(&map_name) else {
                continue;
            };
            if let Some(mus_data) = wad.lump_data(&music_lump) {
                music_library.insert(
                    doom_wad::lump::LumpName::from_str(&music_lump),
                    std::sync::Arc::<[u8]>::from(mus_data),
                );
            }
        }
    }

    for map in 1..=32 {
        let map_name = format!("MAP{map:02}");
        let Some(music_lump) = music_lump_for_map(&map_name) else {
            continue;
        };
        if let Some(mus_data) = wad.lump_data(&music_lump) {
            music_library.insert(
                doom_wad::lump::LumpName::from_str(&music_lump),
                std::sync::Arc::<[u8]>::from(mus_data),
            );
        }
    }

    for lump in ["D_INTER", "D_DM2INT"] {
        if let Some(mus_data) = wad.lump_data(lump) {
            music_library.insert(
                doom_wad::lump::LumpName::from_str(lump),
                std::sync::Arc::<[u8]>::from(mus_data),
            );
        }
    }

    music_library
}

fn default_warp_map(wad_stack: &WadStack) -> String {
    for map in 1..=32 {
        let name = format!("MAP{map:02}");
        if wad_stack.find_map_lump_group(&name).is_some() {
            return name;
        }
    }

    for episode in 1..=4 {
        for map in 1..=9 {
            let name = format!("E{episode}M{map}");
            if wad_stack.find_map_lump_group(&name).is_some() {
                return name;
            }
        }
    }

    "E1M1".to_string()
}

fn score_total_ticks(score: &MusScore) -> u64 {
    score
        .events
        .iter()
        .map(|(delta, _)| u64::from(*delta))
        .sum()
}

fn wav_from_f32_mono(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let channels: u16 = 1;
    let block_align: u16 = channels * (bits_per_sample / 8);
    let byte_rate: u32 = sample_rate * u32::from(block_align);
    let data_bytes_len = (samples.len() * usize::from(block_align)) as u32;

    let mut out = Vec::with_capacity(44 + data_bytes_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32 + data_bytes_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes_len.to_le_bytes());

    for s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let pcm = (clamped * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&pcm.to_le_bytes());
    }

    out
}

fn export_sfx_wav_for_name(
    wad_stack: &WadStack,
    sfx_name: &str,
    out_path: &std::path::Path,
) -> Result<()> {
    let sfx_lump = wad_stack
        .lump_data(sfx_name)
        .ok_or_else(|| anyhow::anyhow!("SFX lump {} not found in WAD stack", sfx_name))?;

    let sfx_sample = doom_audio::mixer::PcmSample::parse_sfx_lump(sfx_lump)
        .map_err(|e| anyhow::anyhow!("Failed to parse SFX lump {}: {}", sfx_name, e))?;

    let mut samples_i16 = Vec::with_capacity(sfx_sample.data.len());
    for &byte in sfx_sample.data.iter() {
        // Convert 8-bit unsigned DOOM PCM (128 = silence) to normalized 32-bit float, then 16-bit signed
        let s = (byte as i32 - 128) as f32 / 127.0;
        let s_clamped = s.clamp(-1.0, 1.0);
        let pcm = (s_clamped * i16::MAX as f32).round() as i16;
        samples_i16.push(pcm);
    }

    let wav_bytes = doom_audio::wav::encode_pcm16_wav_mono(sfx_sample.sample_rate, &samples_i16);
    std::fs::write(out_path, wav_bytes).with_context(|| {
        format!(
            "Could not save the WAV file to '{}'. Please check your permissions.",
            out_path.display()
        )
    })?;
    Ok(())
}

fn export_music_wav_for_map(
    wad_stack: &WadStack,
    map_name: &str,
    loops: u32,
    out_path: &std::path::Path,
) -> Result<()> {
    let loops = loops.max(1);
    let sample_rate: u32 = 44_100;

    let music_lump = music_lump_for_map(map_name)
        .ok_or_else(|| anyhow::anyhow!("No music lump mapping for map {map_name}"))?;
    let mus_data = wad_stack
        .lump_data(&music_lump)
        .ok_or_else(|| anyhow::anyhow!("Music lump {music_lump} not found in WAD stack"))?;
    let score = MusScore::parse(mus_data)
        .map_err(|e| anyhow::anyhow!("Failed to parse MUS lump {music_lump}: {e}"))?;

    let mut player = MidiPlayer::new();
    if let Some(genmidi_data) = wad_stack.lump_data("GENMIDI") {
        if let Ok(bank) = GenmidiBank::parse(genmidi_data) {
            player.load_genmidi(bank);
        }
    }

    let total_ticks = score_total_ticks(&score).max(1);
    let total_samples = ((total_ticks * u64::from(loops)) * u64::from(sample_rate)
        / u64::from(player.ticks_per_sec)) as usize;

    let mut rendered = vec![0.0f32; total_samples];
    player.load_score(score);
    player.advance_samples(total_samples, sample_rate, &mut rendered);

    let wav_bytes = wav_from_f32_mono(&rendered, sample_rate);
    std::fs::write(out_path, wav_bytes).with_context(|| {
        format!(
            "Could not save the WAV file to '{}'. Please check your permissions.",
            out_path.display()
        )
    })?;
    Ok(())
}

fn validate_mode_args(args: &Args) -> std::result::Result<(), &'static str> {
    if args.server.is_some() && args.connect.is_some() {
        return Err("--server and --connect are mutually exclusive");
    }
    if args.record.is_some() && args.playdemo.is_some() {
        return Err("--record and --playdemo are mutually exclusive");
    }
    if args.capture.is_some() && args.record.is_some() {
        return Err("--capture cannot be combined with --record");
    }
    if args.capture.is_some() && args.server.is_some() {
        return Err("--capture cannot be combined with --server");
    }
    if args.capture.is_some() && args.connect.is_some() {
        return Err("--capture cannot be combined with --connect");
    }
    if args.server.is_some() && args.record.is_some() {
        return Err("--server and --record are mutually exclusive");
    }
    if args.server.is_some() && args.playdemo.is_some() {
        return Err("--server and --playdemo are mutually exclusive");
    }
    if args.connect.is_some() && args.record.is_some() {
        return Err("--connect and --record are mutually exclusive");
    }
    if args.connect.is_some() && args.playdemo.is_some() {
        return Err("--connect and --playdemo are mutually exclusive");
    }
    if args.turn_based && (args.server.is_some() || args.connect.is_some()) {
        return Err("--turn-based is only supported in single-player mode");
    }

    Ok(())
}

struct HeadlessCapture {
    framebuffer: Framebuffer,
    active_palette: usize,
}

fn capture_headless_frame(app: &mut impl DoomApp, capture_frames: u32) -> HeadlessCapture {
    let mut framebuffer = Framebuffer::new();
    for _ in 0..capture_frames {
        app.tick(TicInput::default());
    }
    app.render(&mut framebuffer);

    HeadlessCapture {
        framebuffer,
        active_palette: app.active_palette(),
    }
}

fn load_demo_player(path: &std::path::Path) -> Result<DemoPlayer> {
    let demo_bytes =
        std::fs::read(path).with_context(|| format!("Failed to read demo: {}", path.display()))?;
    DemoPlayer::parse(&demo_bytes).with_context(|| "Failed to parse demo")
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn handle_export(
    export_path: Option<&std::path::Path>,
    generate_data: impl FnOnce() -> String,
    success_icon: &str,
    success_verb: &str,
    success_noun: &str,
    error_noun: &str,
    is_json: bool,
) -> Result<bool> {
    let Some(path) = export_path else {
        return Ok(false);
    };

    let data = generate_data();
    std::fs::write(path, data).with_context(|| {
        format!(
            "Could not save the {} to '{}'. Please check your permissions.",
            error_noun,
            path.display()
        )
    })?;

    if is_json {
        let json_data = format!(
            r#"{{"status":"success","action":"export","type":"{}","file":{:?}}}"#,
            success_noun,
            path.display().to_string()
        );
        println!("{json_data}");
    } else {
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} {} to {}",
                success_icon.green(),
                success_verb.green().bold(),
                success_noun,
                path.display().to_string().cyan()
            );
        } else {
            println!("{} {} to {}", success_verb, success_noun, path.display());
        }
    }

    Ok(true)
}

// ---------------------------------------------------------------------------
// Headless demo-verification harness (--verify-demo)
// ---------------------------------------------------------------------------

/// Why a single verification replay stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyEndReason {
    /// The demo tic stream was fully consumed.
    DemoConsumed,
    /// The level signalled an exit (`gs.exit_request` became `Some`).
    LevelExit,
    /// The replay stopped early (e.g. the player mobj disappeared).
    EarlyStop,
}

impl VerifyEndReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::DemoConsumed => "demo-stream-fully-consumed",
            Self::LevelExit => "level-exit",
            Self::EarlyStop => "early-stop",
        }
    }
}

/// Result of a single verification replay run.
struct VerifyRun {
    /// Per-tic CSV (including the header line), terminated by a trailing newline.
    csv: String,
    /// Number of demo ticcmds actually applied.
    total_tics: usize,
    /// Why the replay stopped.
    reason: VerifyEndReason,
    // Final player-0 state (captured after the last applied tic).
    final_px_raw: i32,
    final_py_raw: i32,
    final_pz_raw: i32,
    final_px_int: i32,
    final_py_int: i32,
    final_pz_int: i32,
    final_angle: u32,
    final_health: i32,
    final_kills: u32,
    final_items: u32,
    final_secrets: u32,
    final_rndindex: u32,
    /// Optional RNG call-trace CSV (`leveltime,seq,retval,caller`), populated
    /// only when `--verify-rng-trace` requested it for this run.
    rng_trace_csv: Option<String>,
    /// Optional full-actor-state dump CSV, populated only when `--verify-actors`
    /// requested it for this run.
    actors_csv: Option<String>,
}

/// Header line for the full-actor-state dump, shared byte-for-byte with the
/// instrumented oracle's `$CHOCO_ACTORS_CSV` output.
const VERIFY_ACTORS_HEADER: &str =
    "tic,ord,sprite,frame,x,y,z,momx,momy,momz,angle,health,tics";

/// Walk every live actor in vanilla thinker (creation) order and append one CSV
/// row per mobj to `out`. Ordering mirrors `tic::actors_by_generation`: the
/// slab's monotonic `generation` counter is each actor's global creation index,
/// so sorting live handles by ascending generation reproduces vanilla's
/// `P_AddThinker` tail-append thinker-list order. Fields are raw fixed-point /
/// raw ints (no truncation); `sprite` is the 4-char `sprnames[]` name and
/// `frame` strips the fullbright bit — both cross-comparable with the oracle.
fn dump_actors_for_tic(out: &mut String, tic: usize, gs: &doom_game::GameState) {
    use std::fmt::Write as _;
    let slab = &gs.mobjslab;
    let mut handles: Vec<doom_game::MobjHandle> = (0..slab.slot_count())
        .filter_map(|i| slab.handle_at(i))
        .collect();
    handles.sort_unstable_by_key(|h| h.generation);
    for (ord, h) in handles.iter().enumerate() {
        let Some(mo) = slab.get(*h) else { continue };
        // Derive sprite/frame from the actor's current state (vanilla sets
        // mobj->sprite/frame from the state on every P_SetMobjState).
        let (sprite, frame) = match doom_game::states::STATES.get(mo.state.0 as usize) {
            Some(st) => {
                let name = doom_game::states::sprite_names::SPRITE_NAMES
                    .get(st.sprite as usize)
                    .copied()
                    .unwrap_or("NONE");
                (name, (st.frame & 0x7f) as i32)
            }
            None => ("NONE", 0),
        };
        let _ = writeln!(
            out,
            "{tic},{ord},{sprite},{frame},{x},{y},{z},{momx},{momy},{momz},{angle},{health},{tics}",
            x = mo.x.raw(),
            y = mo.y.raw(),
            z = mo.z.raw(),
            momx = mo.momx.raw(),
            momy = mo.momy.raw(),
            momz = mo.momz.raw(),
            angle = mo.angle.raw(),
            health = mo.health,
            tics = mo.tics,
        );
    }
}

/// Header line shared byte-for-byte with the reference oracle.
const VERIFY_CSV_HEADER: &str =
    "i,rndindex,px,py,pz,angle,health,kills,items,secrets,leveltime";

/// Build a fresh level for `warp_str`, spawn things from the demo header, and
/// replay the demo, emitting one CSV row per applied ticcmd.
fn verify_replay_once(
    wad_stack: &WadStack,
    warp_str: &str,
    header: &doom_demo::LmpHeader,
    demo_bytes: &[u8],
    rng_trace: bool,
    actor_dump: bool,
) -> Result<VerifyRun> {
    if rng_trace {
        doom_game::rng_trace_enable();
    }
    // A fresh, mutable level per run: gs.tick mutates sector heights, etc.
    let mut level = Level::from_wad_stack(wad_stack, warp_str).with_context(|| {
        format!("Could not load map '{warp_str}' for demo verification.")
    })?;

    // Fresh game state: RNG index starts at 0. No title/menu code runs, so the
    // only RNG advancement before the first tic comes from monster-spawn tic
    // randomization inside `spawn_level_things`, exactly as in vanilla.
    let mut gs = GameState::new(warp_str);

    // Skill is driven by the demo header (0-based: 0=ITYTD .. 4=Nightmare).
    let skill = Skill::from_num(header.skill).unwrap_or(Skill::Medium);
    spawn_level_things(&mut gs, &level, skill, doom_game::GameMode::SinglePlayer);

    // Mirror the level-setup specials that the real playback pipeline
    // (`DoomGame::new_with_compat`) initializes, so the simulation matches a
    // normal playthrough as closely as possible.
    init_scrolling_walls(&mut gs, &level);
    init_conveyors(&mut gs, &level);
    init_sector_lights(&mut gs, &level);

    // Freeze the setup/gameplay actor-generation boundary now that all map
    // things and sector specials exist. `tick_world` uses it to run the
    // sector-light thinker pass at vanilla's creation-order position (after
    // setup monsters, before gameplay-spawned actors).
    gs.freeze_thinker_setup_boundary();

    let mut player = DemoPlayer::from_lmp(demo_bytes)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse demo LMP data"))?;

    let mut csv = String::with_capacity(64 * player.total_tics().max(1) + 64);
    csv.push_str(VERIFY_CSV_HEADER);
    csv.push('\n');

    let mut actors_csv: Option<String> = if actor_dump {
        let mut s = String::with_capacity(1 << 20);
        s.push_str(VERIFY_ACTORS_HEADER);
        s.push('\n');
        Some(s)
    } else {
        None
    };

    let mut i: usize = 0;
    let reason = loop {
        let Some(cmd) = player.next_tic() else {
            break VerifyEndReason::DemoConsumed;
        };
        if rng_trace {
            // Stamp draws with the pre-increment leveltime, matching vanilla
            // P_Ticker order (thinkers run, then leveltime++).
            doom_game::rng_trace_set_leveltime(gs.stats.level_time);
        }
        gs.tick(cmd, Some(&mut level));

        // Read player-0 mobj state AFTER simulating this tic.
        let Some(mo) = gs.mobjslab.get(gs.player.handle) else {
            break VerifyEndReason::EarlyStop;
        };
        let px = mo.x.raw();
        let py = mo.y.raw();
        let pz = mo.z.raw();
        let angle = mo.angle.raw();

        let rndindex = gs.rng.index();
        let health = gs.player.health();
        let kills = gs.player.kill_count;
        let items = gs.player.item_count;
        let secrets = gs.player.secret_count;
        let leveltime = gs.stats.level_time;

        use std::fmt::Write as _;
        let _ = writeln!(
            csv,
            "{i},{rndindex},{px},{py},{pz},{angle},{health},{kills},{items},{secrets},{leveltime}"
        );

        // Full-actor-state dump for this tic (same sampling point as the player
        // CSV row above), if requested. `i` is the 0-based tic index == the
        // oracle's `leveltime - 1`.
        if let Some(ref mut acsv) = actors_csv {
            dump_actors_for_tic(acsv, i, &gs);
        }

        i += 1;

        // Stop once the level signals an exit (the row for the exiting tic has
        // already been emitted above).
        if gs.exit_request.is_some() {
            break VerifyEndReason::LevelExit;
        }
    };

    // Capture final player-0 state for the console summary.
    let (final_px_raw, final_py_raw, final_pz_raw, final_px_int, final_py_int, final_pz_int, final_angle) =
        match gs.mobjslab.get(gs.player.handle) {
            Some(mo) => (
                mo.x.raw(),
                mo.y.raw(),
                mo.z.raw(),
                mo.x.to_int(),
                mo.y.to_int(),
                mo.z.to_int(),
                mo.angle.raw(),
            ),
            None => (0, 0, 0, 0, 0, 0, 0),
        };

    Ok(VerifyRun {
        csv,
        total_tics: i,
        reason,
        final_px_raw,
        final_py_raw,
        final_pz_raw,
        final_px_int,
        final_py_int,
        final_pz_int,
        final_angle,
        final_health: gs.player.health(),
        final_kills: gs.player.kill_count,
        final_items: gs.player.item_count,
        final_secrets: gs.player.secret_count,
        final_rndindex: gs.rng.index(),
        rng_trace_csv: if rng_trace {
            let entries = doom_game::rng_trace_take();
            let mut s = String::with_capacity(48 * entries.len() + 32);
            s.push_str("leveltime,seq,retval,caller\n");
            use std::fmt::Write as _;
            for e in &entries {
                let _ = writeln!(s, "{},{},{},{}", e.leveltime, e.seq, e.retval, e.caller);
            }
            Some(s)
        } else {
            None
        },
        actors_csv,
    })
}

/// Derive the warp string (e.g. `E1M5` or `MAP03`) from the demo header,
/// preferring whichever candidate actually loads from the WAD stack.
fn verify_warp_from_header(wad_stack: &WadStack, header: &doom_demo::LmpHeader) -> String {
    let episode = header.episode.max(1);
    let map = header.map.max(1);
    let candidates = [format!("E{episode}M{map}"), format!("MAP{map:02}")];
    for cand in &candidates {
        if Level::from_wad_stack(wad_stack, cand).is_ok() {
            return cand.clone();
        }
    }
    // Fall back to the Doom 1 form even if it did not load, so the caller
    // surfaces a clear load error.
    candidates.into_iter().next().unwrap_or_else(|| "E1M1".to_owned())
}

/// Resolve demo bytes from `source`: a filesystem path if it exists, otherwise
/// a WAD lump name (DEMO1/DEMO2/DEMO3) pulled from the loaded IWAD.
fn verify_resolve_demo_bytes(wad_stack: &WadStack, source: &str) -> Result<(Vec<u8>, String)> {
    let path = std::path::Path::new(source);
    if path.is_file() {
        let bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read demo file '{source}'"))?;
        return Ok((bytes, format!("file:{source}")));
    }
    match wad_stack.lump_data(source) {
        Some(data) => Ok((data.to_vec(), format!("lump:{source}"))),
        None => Err(anyhow::anyhow!(
            "Demo source '{source}' is neither an existing file nor a lump present in the loaded WAD(s)"
        )),
    }
}

fn run_verify_demo(args: &Args, wad_stack: &WadStack, source: &str) -> Result<()> {
    let (demo_bytes, source_label) = verify_resolve_demo_bytes(wad_stack, source)?;

    let header = doom_demo::LmpHeader::from_bytes(&demo_bytes).ok_or_else(|| {
        anyhow::anyhow!("Demo source '{source}' is too short to contain a 13-byte v1.9 header")
    })?;

    let warp_str = verify_warp_from_header(wad_stack, &header);

    let runs = args.verify_runs.max(1);
    let mut results: Vec<VerifyRun> = Vec::with_capacity(runs as usize);
    for run_idx in 0..runs {
        // Only trace/dump the first run to avoid overhead on the rest.
        let trace = args.verify_rng_trace.is_some() && run_idx == 0;
        let actor_dump = args.verify_actors.is_some() && run_idx == 0;
        results.push(verify_replay_once(
            wad_stack, &warp_str, &header, &demo_bytes, trace, actor_dump,
        )?);
    }

    if let Some(ref trace_path) = args.verify_rng_trace {
        if let Some(csv) = results[0].rng_trace_csv.as_ref() {
            std::fs::write(trace_path, csv.as_bytes()).with_context(|| {
                format!("Could not write RNG trace to '{}'", trace_path.display())
            })?;
            println!("rng trace         : {}", trace_path.display());
        }
    }

    if let Some(ref actors_path) = args.verify_actors {
        if let Some(csv) = results[0].actors_csv.as_ref() {
            std::fs::write(actors_path, csv.as_bytes()).with_context(|| {
                format!("Could not write actor dump to '{}'", actors_path.display())
            })?;
            println!("actor dump        : {}", actors_path.display());
        }
    }

    // Cross-run determinism check: every CSV must be byte-identical.
    let first_csv = &results[0].csv;
    let mut determinism_pass = true;
    let mut first_diff_row: Option<usize> = None;
    let mut diff_run: Option<usize> = None;
    for (run_idx, r) in results.iter().enumerate().skip(1) {
        if r.csv.as_bytes() != first_csv.as_bytes() {
            determinism_pass = false;
            diff_run = Some(run_idx);
            // Locate the first differing row (0-based data-row index, i.e. the
            // header line is row index -1 and is skipped from the count).
            let base_lines: Vec<&str> = first_csv.lines().collect();
            let this_lines: Vec<&str> = r.csv.lines().collect();
            let max = base_lines.len().max(this_lines.len());
            for li in 0..max {
                let a = base_lines.get(li).copied().unwrap_or("");
                let b = this_lines.get(li).copied().unwrap_or("");
                if a != b {
                    // li == 0 is the header line; data rows start at li == 1.
                    first_diff_row = Some(li.saturating_sub(1));
                    break;
                }
            }
            break;
        }
    }

    // Write the per-tic CSV (all runs are identical when determinism passes;
    // we always write run 0's log).
    if let Some(ref log_path) = args.verify_log {
        std::fs::write(log_path, first_csv.as_bytes()).with_context(|| {
            format!(
                "Could not write verification log to '{}'",
                log_path.display()
            )
        })?;
    }

    // --- Console summary ---
    let r0 = &results[0];
    let flags_set = header.deathmatch != 0
        || header.respawn
        || header.fast
        || header.nomonsters;

    println!("=== doom-rs demo verification ===");
    println!("demo source       : {source_label}");
    println!(
        "header            : version={} skill={} episode={} map={} -> warp={warp_str}",
        header.version, header.skill, header.episode, header.map
    );
    println!(
        "header flags       : deathmatch={} respawn={} fast={} nomonsters={}",
        header.deathmatch,
        u8::from(header.respawn),
        u8::from(header.fast),
        u8::from(header.nomonsters),
    );
    if header.nomonsters || header.fast || header.respawn {
        println!(
            "WARNING           : nomonsters/fast/respawn are NOT wired into spawn_level_things; \
             this replay ignores them and may desync from vanilla."
        );
    }
    let _ = flags_set;
    println!("demo tics in file : {}", {
        // Reconstruct the parsed tic count for reporting.
        DemoPlayer::from_lmp(&demo_bytes).map_or(0, |p| p.total_tics())
    });
    println!("total tics played : {}", r0.total_tics);
    println!("stop reason       : {}", r0.reason.as_str());
    println!(
        "final player pos  : raw=({},{},{})  int=({},{},{}) map units",
        r0.final_px_raw,
        r0.final_py_raw,
        r0.final_pz_raw,
        r0.final_px_int,
        r0.final_py_int,
        r0.final_pz_int
    );
    println!("final angle (BAM) : {}", r0.final_angle);
    println!("final health      : {}", r0.final_health);
    println!(
        "final k/i/s       : kills={} items={} secrets={}",
        r0.final_kills, r0.final_items, r0.final_secrets
    );
    println!("final rndindex    : {}", r0.final_rndindex);
    if determinism_pass {
        println!("determinism ({runs}x): PASS");
    } else {
        println!(
            "determinism ({runs}x): FAIL (run {} differs; first differing row index {})",
            diff_run.unwrap_or(0),
            first_diff_row
                .map(|r| r.to_string())
                .unwrap_or_else(|| "unknown".to_owned()),
        );
    }
    if let Some(ref log_path) = args.verify_log {
        println!("per-tic log       : {}", log_path.display());
    }

    Ok(())
}

fn run_doom(args: Args) -> Result<()> {
    // Initialize trig tables (required for sin/cos in the game simulation).
    // SAFETY: called exactly once at startup, single-threaded, before any
    // Bam::sin() or Bam::cos() calls.
    doom_types::Bam::init_trig_tables();

    let compat = args.compat;

    let iwad_bytes = std::fs::read(&args.iwad).with_context(|| {
        format!(
            "Could not locate the IWAD file '{}'. Please check the path and try again.",
            args.iwad.display()
        )
    })?;

    // Build a WadStack for patch/lump lookups (menu graphics, HUD sprites).
    let mut wad_stack = WadStack::new();
    wad_stack.push_iwad(iwad_bytes).with_context(|| {
        format!(
            "The IWAD file '{}' could not be parsed. It may be corrupted.",
            args.iwad.display()
        )
    })?;

    for pwad_path in &args.pwad {
        let pwad_bytes = std::fs::read(pwad_path).with_context(|| {
            format!(
                "Could not locate the PWAD file '{}'. Please check the path and try again.",
                pwad_path.display()
            )
        })?;
        wad_stack.push_pwad(pwad_bytes).with_context(|| {
            format!(
                "The PWAD file '{}' could not be parsed. It may be corrupted.",
                pwad_path.display()
            )
        })?;
    }

    // Headless demo-verification harness. Runs a fully self-contained replay
    // (no rendering, no audio, no title/menu) driven entirely by the demo
    // header, and writes a per-tic determinism log. Trig tables were already
    // initialized at the top of `run_doom`.
    if let Some(ref source) = args.verify_demo {
        return run_verify_demo(&args, &wad_stack, source);
    }

    // Build the PLAYPAL blit palette (for terminal RGB conversion).
    let blit_palette = match wad_stack.lump_data("PLAYPAL") {
        Some(data) => PaletteLut::from_playpal(data).unwrap_or_else(|_| PaletteLut::grayscale()),
        None => PaletteLut::grayscale(),
    };

    // Determine which map to load — default to the first canonical map present.
    let default_warp = default_warp_map(&wad_stack);
    let warp_str = args.warp.as_deref().unwrap_or(&default_warp);
    let show_title = args.warp.is_none();

    // Parse the requested level.
    let level = Level::from_wad_stack(&wad_stack, warp_str).with_context(|| {
        format!(
            "Could not load the map '{warp_str}'. Please ensure it exists in the provided WADs."
        )
    })?;

    if handle_export(
        args.export_html.as_deref(),
        || doom_map::export_map_to_html(&level),
        "🌟",
        "Exported",
        "HTML report",
        "HTML report",
        args.json,
    )? {
        return Ok(());
    }

    if handle_export(
        args.export_json.as_deref(),
        || doom_map::export_map_to_json(&level),
        "🌟",
        "Exported",
        "JSON report",
        "JSON report",
        args.json,
    )? {
        return Ok(());
    }

    if handle_export(
        args.export_svg.as_deref(),
        || doom_map::export_map_to_svg(&level),
        "🌟",
        "Exported",
        "layout",
        "SVG layout",
        args.json,
    )? {
        return Ok(());
    }

    if handle_export(
        args.export_geojson.as_deref(),
        || doom_map::export_map_to_geojson(&level),
        "🌟",
        "Exported",
        "GeoJSON",
        "GeoJSON file",
        args.json,
    )? {
        return Ok(());
    }

    if handle_export(
        args.export_dot.as_deref(),
        || doom_map::SectorGraph::build(&level).to_dot(),
        "🌟",
        "Exported",
        "Graphviz DOT",
        "Graphviz DOT file",
        args.json,
    )? {
        return Ok(());
    }

    if handle_export(
        args.export_obj.as_deref(),
        || doom_map::obj::export_map_to_obj(&level),
        "🌟",
        "Exported",
        "3D model",
        "3D model",
        args.json,
    )? {
        return Ok(());
    }

    if let Some(ref paths) = args.export_demo_csv {
        let input_path = &paths[0];
        let output_path = &paths[1];
        let mut player = load_demo_player(input_path)?;
        let csv_data = doom_demo::export_demo_to_csv(&mut player);
        std::fs::write(output_path, csv_data).with_context(|| {
            format!(
                "Could not save demo CSV to '{}'. Please check your permissions.",
                output_path.display()
            )
        })?;
        if args.json {
            let json_data = format!(
                r#"{{"status":"success","action":"export","type":"demo CSV","file":{:?}}}"#,
                output_path.display().to_string()
            );
            println!("{json_data}");
        } else {
            use crossterm::style::Stylize;
            if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
                println!(
                    "{} {} demo CSV to {}",
                    "🌟".green(),
                    "Exported".green().bold(),
                    output_path.display().to_string().cyan()
                );
            } else {
                println!("Exported demo CSV to {}", output_path.display());
            }
        }
        return Ok(());
    }

    if let Some(ref wav_path) = args.export_music_wav {
        export_music_wav_for_map(&wad_stack, warp_str, args.music_loops, wav_path)?;
        if args.json {
            let json_data = format!(
                r#"{{"status":"success","action":"export","type":"music WAV","file":{:?}}}"#,
                wav_path.display().to_string()
            );
            println!("{json_data}");
        } else {
            use crossterm::style::Stylize;
            if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
                println!(
                    "{} {} music WAV to {}",
                    "🎵".green(),
                    "Exported".green().bold(),
                    wav_path.display().to_string().cyan()
                );
            } else {
                println!("Exported music WAV to {}", wav_path.display());
            }
        }
        return Ok(());
    }

    if let Some(ref sfx_wav_path) = args.export_sfx_wav {
        let sfx_name = args
            .sfx_name
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("--sfx-name is required when using --export-sfx-wav"))?;
        export_sfx_wav_for_name(&wad_stack, sfx_name, sfx_wav_path)?;
        if args.json {
            let json_data = format!(
                r#"{{"status":"success","action":"export","type":"SFX WAV","file":{:?}}}"#,
                sfx_wav_path.display().to_string()
            );
            println!("{json_data}");
        } else {
            use crossterm::style::Stylize;
            if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
                println!(
                    "{} {} SFX WAV to {}",
                    "🔊".green(),
                    "Exported".green().bold(),
                    sfx_wav_path.display().to_string().cyan()
                );
            } else {
                println!("Exported SFX WAV to {}", sfx_wav_path.display());
            }
        }
        return Ok(());
    }

    if args.analyze {
        let graph = doom_map::SectorGraph::build(&level);
        let analyzer = doom_map::MapAnalyzer::new(&graph);
        let chokepoints = analyzer.chokepoints();
        let areas = analyzer.isolated_areas();

        if args.json {
            // ⚡ Bolt Optimization:
            // Formats the JSON array inline directly into a single `String` buffer.
            // This completely eliminates intermediate `.collect::<Vec<_>>()` chains
            // and intermediate inner string allocations that previously happened per-area,
            // saving ~3 heap allocations per JSON generation loop.
            let mut chokepoints_json = String::new();
            chokepoints_json.push('[');
            for (i, s) in chokepoints.iter().enumerate() {
                if i > 0 {
                    chokepoints_json.push_str(", ");
                }
                chokepoints_json.push_str(&s.to_string());
            }
            chokepoints_json.push(']');
            let mut areas_json = String::new();
            areas_json.push('[');
            for (i, a) in areas.iter().enumerate() {
                if i > 0 {
                    areas_json.push_str(", ");
                }
                areas_json.push('[');
                for (j, s) in a.iter().enumerate() {
                    if j > 0 {
                        areas_json.push_str(", ");
                    }
                    areas_json.push_str(&s.to_string());
                }
                areas_json.push(']');
            }
            areas_json.push(']');

            let json_data = format!(
                r#"{{
  "map": "{}",
  "chokepoints": {},
  "isolated_areas": {}
}}"#,
                warp_str, chokepoints_json, areas_json
            );
            println!("{json_data}");
        } else {
            use crossterm::style::Stylize;
            let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
            if is_tty {
                println!(
                    "{} {} tactical analysis for {}",
                    "🌟".green(),
                    "Completed".green().bold(),
                    warp_str.cyan()
                );
            } else {
                println!("Completed tactical analysis for {}", warp_str);
            }

            let mut chokepoints_str = String::new();
            if chokepoints.is_empty() {
                chokepoints_str.push_str("None");
            } else {
                for (i, s) in chokepoints.iter().enumerate() {
                    if i > 0 {
                        chokepoints_str.push_str(", ");
                    }
                    chokepoints_str.push_str(&s.to_string());
                }
            }

            let mut table = comfy_table::Table::new();
            table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
            table
                .load_preset(comfy_table::presets::UTF8_FULL)
                .apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS)
                .set_content_arrangement(comfy_table::ContentArrangement::Dynamic);

            if is_tty {
                table.set_header(vec![
                    comfy_table::Cell::new("Feature")
                        .fg(comfy_table::Color::Cyan)
                        .add_attribute(comfy_table::Attribute::Bold),
                    comfy_table::Cell::new("Data")
                        .fg(comfy_table::Color::Cyan)
                        .add_attribute(comfy_table::Attribute::Bold),
                ]);
                table.add_row(vec![
                    comfy_table::Cell::new("🗺️  Chokepoints"),
                    comfy_table::Cell::new(&chokepoints_str).fg(comfy_table::Color::Yellow),
                ]);
                for (i, area) in areas.iter().enumerate() {
                    let mut area_str = String::new();
                    for (j, s) in area.iter().enumerate() {
                        if j > 0 {
                            area_str.push_str(", ");
                        }
                        area_str.push_str(&s.to_string());
                    }
                    table.add_row(vec![
                        comfy_table::Cell::new(format!("🏝️  Isolated Area {}", i + 1)),
                        comfy_table::Cell::new(area_str).fg(comfy_table::Color::Magenta),
                    ]);
                }
            } else {
                table.set_header(vec![
                    comfy_table::Cell::new("Feature"),
                    comfy_table::Cell::new("Data"),
                ]);
                table.add_row(vec![
                    comfy_table::Cell::new("🗺️  Chokepoints"),
                    comfy_table::Cell::new(&chokepoints_str),
                ]);
                for (i, area) in areas.iter().enumerate() {
                    let mut area_str = String::new();
                    for (j, s) in area.iter().enumerate() {
                        if j > 0 {
                            area_str.push_str(", ");
                        }
                        area_str.push_str(&s.to_string());
                    }
                    table.add_row(vec![
                        comfy_table::Cell::new(format!("Isolated Area {}", i + 1)),
                        comfy_table::Cell::new(area_str),
                    ]);
                }
            }
            println!("{table}");
        }
        return Ok(());
    }

    if let Some(path_str) = &args.pathfind {
        use crossterm::style::Stylize;
        let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
        // Avoids an unnecessary heap allocation from .collect::<Vec<_>>()
        if let Some((start_str, end_str)) = path_str.split_once(',') {
            if let (Ok(start), Ok(end)) = (start_str.parse::<usize>(), end_str.parse::<usize>()) {
                let graph = doom_map::SectorGraph::build(&level);
                if let Some(path) = graph.shortest_path(start, end) {
                    if args.json {
                        let mut path_inner = String::new();
                        for (j, s) in path.iter().enumerate() {
                            if j > 0 {
                                path_inner.push_str(", ");
                            }
                            path_inner.push_str(&s.to_string());
                        }
                        let path_json = format!("[{}]", path_inner);
                        let json_data = format!(r#"{{ "path": {} }}"#, path_json);
                        println!("{json_data}");
                    } else {
                        let mut path_str = String::new();
                        for (j, s) in path.iter().enumerate() {
                            if j > 0 {
                                path_str.push_str(" ➔ ");
                            }
                            path_str.push_str(&s.to_string());
                        }
                        if is_tty {
                            println!(
                                "{} {} {}",
                                "🗺️ ".green(),
                                "Path found:".green().bold(),
                                path_str.cyan()
                            );
                        } else {
                            println!("Path found: {}", path_str);
                        }
                    }
                } else {
                    if args.json {
                        let msg =
                            format!("No path found between sector {} and sector {}", start, end);
                        let json_data = format!(r#"{{ "error": "{}" }}"#, msg);
                        println!("{json_data}");
                    } else {
                        if is_tty {
                            println!(
                                "{} {}",
                                "❌".yellow(),
                                format!(
                                    "No path found between sector {} and sector {}",
                                    start, end
                                )
                                .yellow()
                                .bold()
                            );
                        } else {
                            println!("No path found between sector {} and sector {}", start, end);
                        }
                    }
                }
            } else {
                let msg =
                    "Invalid sector indices. Please provide two integers separated by a comma.";
                if args.json {
                    let json_data = format!(r#"{{ "error": "{}" }}"#, msg);
                    println!("{json_data}");
                } else {
                    if is_tty {
                        println!("{} {}", "❌".yellow(), msg.yellow().bold());
                    } else {
                        println!("{}", msg);
                    }
                }
            }
        } else {
            let msg = "Invalid format. Please use START,END (e.g. 0,5).";
            if args.json {
                let json_data = format!(r#"{{ "error": "{}" }}"#, msg);
                println!("{json_data}");
            } else {
                if is_tty {
                    println!("{} {}", "❌".yellow(), msg.yellow().bold());
                } else {
                    println!("{}", msg);
                }
            }
        }
        return Ok(());
    }

    if args.map_stats {
        let mut gs = GameState::new(warp_str);
        doom_game::spawn_level_things(
            &mut gs,
            &level,
            Skill::Medium,
            doom_game::GameMode::SinglePlayer,
        );
        let stats = gs.compute_intermission_stats();

        if args.json {
            let json_data = format!(
                r#"{{
  "map": "{}",
  "total_kills": {},
  "total_items": {},
  "total_secrets": {},
  "par_time_tics": {}
}}"#,
                warp_str,
                stats.total_kills,
                stats.total_items,
                stats.total_secrets,
                stats.par_time_tics
            );
            println!("{json_data}");
        } else {
            let par_time_mins = stats.par_time_tics / 35 / 60;
            let par_time_secs = (stats.par_time_tics / 35) % 60;
            let par_time_formatted = format!(
                "{:02}:{:02} ({} tics)",
                par_time_mins, par_time_secs, stats.par_time_tics
            );

            let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
            let mut table = comfy_table::Table::new();
            table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
            table
                .load_preset(comfy_table::presets::UTF8_FULL)
                .apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS)
                .set_content_arrangement(comfy_table::ContentArrangement::Dynamic);

            if is_tty {
                table
                    .set_header(vec![
                        comfy_table::Cell::new("Statistic")
                            .fg(comfy_table::Color::Cyan)
                            .add_attribute(comfy_table::Attribute::Bold),
                        comfy_table::Cell::new("Value")
                            .fg(comfy_table::Color::Cyan)
                            .add_attribute(comfy_table::Attribute::Bold),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("🗺️  Map"),
                        comfy_table::Cell::new(warp_str.to_string()).fg(comfy_table::Color::Yellow),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("💀 Total Kills"),
                        comfy_table::Cell::new(stats.total_kills.to_string())
                            .fg(comfy_table::Color::Yellow),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("📦 Total Items"),
                        comfy_table::Cell::new(stats.total_items.to_string())
                            .fg(comfy_table::Color::Green),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("🕵️  Total Secrets"),
                        comfy_table::Cell::new(stats.total_secrets.to_string())
                            .fg(comfy_table::Color::Magenta),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("⏱️  Par Time"),
                        comfy_table::Cell::new(par_time_formatted).fg(comfy_table::Color::Cyan),
                    ]);
            } else {
                table
                    .set_header(vec![
                        comfy_table::Cell::new("Statistic"),
                        comfy_table::Cell::new("Value"),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("Map"),
                        comfy_table::Cell::new(warp_str.to_string()),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("Total Kills"),
                        comfy_table::Cell::new(stats.total_kills.to_string()),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("Total Items"),
                        comfy_table::Cell::new(stats.total_items.to_string()),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("Total Secrets"),
                        comfy_table::Cell::new(stats.total_secrets.to_string()),
                    ])
                    .add_row(vec![
                        comfy_table::Cell::new("Par Time"),
                        comfy_table::Cell::new(par_time_formatted),
                    ]);
            }
            println!("{table}");
        }
        return Ok(());
    }

    // Create game state and spawn ALL level things (player, monsters, items, keys).
    let mut gs = GameState::new(warp_str);
    let skill = args
        .skill
        .checked_sub(1)
        .and_then(Skill::from_num)
        .unwrap_or(Skill::Medium);
    spawn_level_things(&mut gs, &level, skill, doom_game::GameMode::SinglePlayer);

    // Apply DeHackEd patch if one was specified.
    if let Some(ref deh_path) = args.deh {
        let contents = std::fs::read_to_string(deh_path)
            .with_context(|| format!("Failed to read DeHackEd file: {deh_path}"))?;
        let patch =
            DehPatch::parse(&contents).map_err(|e| anyhow::anyhow!("DeHackEd parse error: {e}"))?;
        let mut mobjinfo_vec: Vec<_> = MOBJINFO.to_vec();
        let mut states_vec: Vec<_> = STATES.to_vec();
        let count = patch
            .apply(&mut mobjinfo_vec, &mut states_vec)
            .map_err(|e| anyhow::anyhow!("DeHackEd apply error: {e}"))?;
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} applied {} modification(s) from {}",
                "⚙️".green(),
                "DeHackEd:".green().bold(),
                count.to_string().cyan(),
                deh_path.as_str().yellow()
            );
        } else {
            println!(
                "DeHackEd: applied {} modification(s) from {}",
                count,
                deh_path.as_str()
            );
        }
    }

    // Load flat texture cache (floor/ceiling textures between F_START and F_END).
    let flat_cache = FlatCache::load_from_stack_with_profile(&wad_stack, compat);
    let flat_cache = if flat_cache.is_empty() {
        None
    } else {
        Some(flat_cache)
    };

    // Load wall texture cache (TEXTURE1/TEXTURE2 composed textures).
    let tex_cache = {
        let cache = TextureCache::load_from_stack(&wad_stack);
        if cache.is_empty() { None } else { Some(cache) }
    };

    // Load sprite cache (sprite frames between S_START and S_END).
    let sprite_cache = {
        let cache = SpriteCache::load_from_stack_with_profile(&wad_stack, compat);
        if cache.is_empty() { None } else { Some(cache) }
    };

    // Load colormap cache (COLORMAP lump: 34 × 256 bytes, light-level shading).
    let colormap_cache = Some(ColormapCache::load_with_profile(&wad_stack, compat));

    // Try to open the audio subsystem.  Returns None in headless/CI environments.
    let audio = AudioSystem::try_open(&wad_stack);

    let music_library = load_music_library(&wad_stack);

    if let Err(message) = validate_mode_args(&args) {
        return Err(anyhow::anyhow!("Invalid arguments: {message}"));
    }

    // Server mode: spin up a relay server that forwards tic packets between
    // connected clients. No WAD-based game loop is required on the server.
    if let Some(port) = args.server {
        return net_mode::run_server(port);
    }

    // Build the app.
    let debug_log = args
        .debug_log
        .as_ref()
        .and_then(|p| match std::fs::File::create(p) {
            Ok(f) => Some(f),
            Err(e) => {
                use crossterm::style::Stylize;
                if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
                    eprintln!(
                        "⚠️ {}: could not open debug log '{}': {}",
                        "Warning".yellow().bold(),
                        p.display(),
                        e
                    );
                } else {
                    eprintln!("Warning: could not open debug log '{}': {}", p.display(), e);
                }
                None
            }
        });

    // Build a name→ID map for all DS* lumps so monster sounds can be resolved
    // by lump name at play time without additional WAD scans.
    let sfx_lookup = audio_system::build_sfx_lookup(&wad_stack);

    // Resolve player pain SFX (DSPLPAIN) once at startup so we can fire it cheaply.
    let pain_sfx_id = sfx_lookup
        .get(&doom_wad::lump::LumpName::from_str("DSPLPAIN"))
        .copied();

    let mut app = DoomGame::new_with_compat(
        gs,
        level,
        audio,
        music_library,
        flat_cache,
        tex_cache,
        sprite_cache,
        colormap_cache,
        show_title,
        debug_log,
        pain_sfx_id,
        sfx_lookup,
        compat,
    );
    app.attach_wad_for_transitions(skill, wad_stack);

    // Timedemo mode: play back a demo as fast as possible, then print FPS and exit.
    if let Some(ref timedemo_path) = args.timedemo {
        use crossterm::style::Stylize;
        use std::time::Instant;

        let player = load_demo_player(timedemo_path)?;
        let mut playback_app = demo_mode::DemoPlaybackApp::new_with_compat(app, player, compat);
        let mut framebuffer = Framebuffer::new();

        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} timedemo from {}",
                "🚀".cyan(),
                "Starting".cyan().bold(),
                timedemo_path.display().to_string().cyan()
            );
        } else {
            println!("Starting timedemo from {}", timedemo_path.display());
        }

        let start = Instant::now();
        let mut actual_tics = 0;

        while !playback_app.is_finished() {
            playback_app.tick(TicInput::default());
            playback_app.render(&mut framebuffer);
            actual_tics += 1;
        }

        let duration = start.elapsed();
        let fps = (actual_tics as f64) / duration.as_secs_f64();

        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} timedemo: {} tics in {:.2} seconds ({:.2} fps)",
                "✅".green(),
                "Finished".green().bold(),
                actual_tics.to_string().yellow(),
                duration.as_secs_f64(),
                fps.to_string().green().bold()
            );
        } else {
            println!(
                "Finished timedemo: {} tics in {:.2} seconds ({:.2} fps)",
                actual_tics,
                duration.as_secs_f64(),
                fps
            );
        }

        return Ok(());
    }

    // Headless capture mode: tick N frames, render, save BMP, exit.
    if let Some(ref capture_path) = args.capture {
        let capture = if let Some(ref demo_path) = args.playdemo {
            let player = load_demo_player(demo_path)?;
            let mut playback_app = demo_mode::DemoPlaybackApp::new_with_compat(app, player, compat);
            capture_headless_frame(&mut playback_app, args.capture_frames)
        } else {
            let mut app = app;
            capture_headless_frame(&mut app, args.capture_frames)
        };

        // Write the framebuffer as a 24-bit BMP file.
        write_bmp(
            capture_path,
            &capture.framebuffer,
            &blit_palette,
            capture.active_palette,
        )
        .with_context(|| format!("Failed to write capture: {}", capture_path.display()))?;
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} frame {} to {}",
                "✅".green(),
                "Captured".green().bold(),
                args.capture_frames.to_string().cyan(),
                capture_path.display().to_string().yellow()
            );
        } else {
            println!(
                "Captured frame {} to {}",
                args.capture_frames,
                capture_path.display()
            );
        }
        return Ok(());
    }

    // Client (netplay) mode: wrap DoomGame in a NetGameApp for network-aware input.
    if let Some(ref addr_str) = args.connect {
        let client = doom_net::NetClient::connect(addr_str, 0).map_err(|e| match e.kind() {
            std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::ConnectionReset => {
                anyhow::anyhow!(
                    "Connection Failed: The relay server at {} is not responding.",
                    addr_str
                )
            }
            _ => anyhow::anyhow!("Connection Failed: {}", e),
        })?;
        let mut net_app = net_mode::NetGameApp::new(app, client);

        let mut event_loop = DoomEventLoop::new()
            .map_err(|e| anyhow::anyhow!("Terminal Initialization Failed: {}", e))?;
        event_loop.set_turn_based_mode(args.turn_based);
        event_loop
            .run(&mut net_app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Terminal Display Failed: {}", e))?;
        return Ok(());
    }

    // Windowed presentation backend: hand the same DoomGame + palette to the
    // native windowed host instead of the terminal event loop. The default
    // terminal path below is left entirely unchanged.
    if args.present == Present::Window {
        #[cfg(feature = "window")]
        {
            return doom_present::run_windowed(app, blit_palette)
                .map_err(|e| anyhow::anyhow!("Windowed presenter failed: {}", e));
        }
        #[cfg(not(feature = "window"))]
        {
            anyhow::bail!("windowed presenter not compiled in; rebuild with `--features window`");
        }
    }

    // Start the terminal event loop and run until the user quits (Q or Esc).
    let mut event_loop = DoomEventLoop::new()
        .map_err(|e| anyhow::anyhow!("Terminal Initialization Failed: {}", e))?;
    event_loop.set_turn_based_mode(args.turn_based);

    // Set renderer mode from --renderer flag.
    // "auto" = detect best graphics protocol; named modes set explicitly (with silent fallback).
    if args.renderer == "auto" {
        if args.turn_based {
            event_loop.set_renderer_mode(RendererMode::Cogmind);
        } else {
            event_loop.set_graphics_protocol(true);
        }
    } else if let Some(mode) = RendererMode::from_str_loose(&args.renderer) {
        event_loop.set_renderer_mode(mode);
    } else {
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            eprintln!(
                "⚠️ {}: unknown --renderer {:?}, using auto-detect",
                "Warning".yellow().bold(),
                args.renderer
            );
        } else {
            eprintln!(
                "Warning: unknown --renderer {:?}, using auto-detect",
                args.renderer
            );
        }
        event_loop.set_graphics_protocol(true);
    }

    if let Some(demo_path) = args.playdemo {
        // Load and parse the demo file.
        let player = load_demo_player(&demo_path)?;
        let mut playback_app = demo_mode::DemoPlaybackApp::new_with_compat(app, player, compat);
        event_loop
            .run(&mut playback_app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Terminal Display Failed: {}", e))?;

        #[cfg(feature = "telemetry")]
        if let Some(path) = args.telemetry_out.as_deref() {
            if let Err(e) =
                std::fs::write(path, playback_app.inner().gs.telemetry.export_to_geojson())
            {
                log::error!("Failed to write telemetry: {}", e);
            } else {
                println!("Telemetry written to {}", path.display());
            }
        }
    } else if let Some(record_path) = args.record {
        // Parse episode/map from the --warp argument.
        let (episode, map) = parse_warp_episode_map(warp_str);
        // Clamp skill to 0-4 (LMP uses 0-based skill internally).
        let skill = args.skill.saturating_sub(1).min(4);
        let header = LmpHeader::new_singleplayer(skill, episode, map);
        let recorder = DemoRecorder::new(header);
        let mut recording_app =
            demo_mode::DemoRecordingWrapper::new_with_compat(app, recorder, record_path, compat);
        event_loop
            .run(&mut recording_app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Terminal Display Failed: {}", e))?;

        #[cfg(feature = "telemetry")]
        if let Some(path) = args.telemetry_out.as_deref() {
            if let Err(e) =
                std::fs::write(path, recording_app.inner().gs.telemetry.export_to_geojson())
            {
                log::error!("Failed to write telemetry: {}", e);
            } else {
                println!("Telemetry written to {}", path.display());
            }
        }
    } else {
        let mut app = app;
        event_loop
            .run(&mut app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Terminal Display Failed: {}", e))?;

        #[cfg(feature = "telemetry")]
        if let Some(path) = args.telemetry_out.as_deref() {
            if let Err(e) = std::fs::write(path, app.gs.telemetry.export_to_geojson()) {
                log::error!("Failed to write telemetry: {}", e);
            } else {
                println!("Telemetry written to {}", path.display());
            }
        }
    }

    Ok(())
}

fn main() {
    let args = match Args::try_parse() {
        Ok(a) => a,
        Err(e) => {
            e.exit();
        }
    };
    let is_json = args.json;

    if let Err(err) = run_doom(args) {
        if is_json {
            // Memory: manually serialize without serde using format!("{:?}", ...) for escaping
            let mut error_msg = format!("{}", err);
            let mut causes = err.chain().skip(1).peekable();
            if causes.peek().is_some() {
                error_msg.push_str(" \nReason:\n");
                for cause in causes {
                    error_msg.push_str(&format!("    {}\n", cause));
                }
            }
            let json_data = format!(r#"{{"error": {:?}}}"#, error_msg.trim_end());
            println!("{json_data}");
        } else {
            use crossterm::style::Stylize;
            if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
                eprintln!("\n❌ {}: {}", "Engine Failure".red().bold(), err);

                let mut causes = err.chain().skip(1).peekable();
                if causes.peek().is_some() {
                    eprintln!("\n↳ {}:", "Reason".red().bold());
                    for cause in causes {
                        eprintln!("    {}", cause);
                    }
                }
                eprintln!();
            } else {
                eprintln!("❌ Engine Failure: {}", err);

                let mut causes = err.chain().skip(1).peekable();
                if causes.peek().is_some() {
                    eprintln!("↳ Reason:");
                    for cause in causes {
                        eprintln!("    {}", cause);
                    }
                }
                eprintln!();
            }
        }
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// BMP frame capture (zero extra dependencies)
// ---------------------------------------------------------------------------

/// Write a 320x200 palette-indexed framebuffer as a 24-bit BMP file.
fn write_bmp(
    path: &std::path::Path,
    fb: &Framebuffer,
    palette: &PaletteLut,
    active_palette: usize,
) -> std::io::Result<()> {
    use std::io::Write;

    const W: usize = 320;
    const H: usize = 200;
    // Each row: W * 3 bytes BGR. Row stride must be 4-byte aligned.
    let row_bytes = W * 3;
    let row_stride = (row_bytes + 3) & !3;
    let pixel_data_size = row_stride * H;
    let file_size = 14 + 40 + pixel_data_size;

    let mut out = std::fs::File::create(path)?;

    // -- BMP file header (14 bytes) --
    out.write_all(b"BM")?;
    out.write_all(&(file_size as u32).to_le_bytes())?;
    out.write_all(&0u32.to_le_bytes())?; // reserved
    out.write_all(&54u32.to_le_bytes())?; // pixel data offset

    // -- DIB header (BITMAPINFOHEADER, 40 bytes) --
    out.write_all(&40u32.to_le_bytes())?;
    out.write_all(&(W as i32).to_le_bytes())?;
    out.write_all(&(H as i32).to_le_bytes())?; // positive = bottom-up
    out.write_all(&1u16.to_le_bytes())?; // planes
    out.write_all(&24u16.to_le_bytes())?; // bits per pixel
    out.write_all(&0u32.to_le_bytes())?; // compression (none)
    out.write_all(&(pixel_data_size as u32).to_le_bytes())?;
    out.write_all(&0u32.to_le_bytes())?; // x ppm
    out.write_all(&0u32.to_le_bytes())?; // y ppm
    out.write_all(&0u32.to_le_bytes())?; // colors used
    out.write_all(&0u32.to_le_bytes())?; // important colors

    // -- Pixel data (bottom-up rows, BGR) --
    let pad = [0u8; 3];
    let pad_len = row_stride - row_bytes;
    for y in (0..H).rev() {
        for x in 0..W {
            let idx = fb.data[y * W + x];
            let rgb = palette.get(active_palette, idx);
            out.write_all(&[rgb.b, rgb.g, rgb.r])?;
        }
        if pad_len > 0 {
            out.write_all(&pad[..pad_len])?;
        }
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
    use doom_game::cheats as game_cheats;
    use doom_game::{GameState, Mobj, PlayerState, flags};
    use doom_map::{Blockmap, Level, Reject, Sector};
    use doom_types::mobj_kind::MobjKind;

    #[derive(Default)]
    struct FakeCaptureApp {
        ticks: u32,
        renders: u32,
        active_palette: usize,
        fill_color: u8,
    }

    impl DoomApp for FakeCaptureApp {
        fn tick(&mut self, _input: TicInput) {
            self.ticks += 1;
            self.fill_color = self.fill_color.wrapping_add(1);
        }

        fn render(&mut self, fb: &mut Framebuffer) {
            self.renders += 1;
            fb.clear(self.fill_color);
        }

        fn active_palette(&self) -> usize {
            self.active_palette
        }
    }
    use doom_renderer::{Framebuffer, StatusBarData};
    use doom_types::{Bam, CompatibilityProfile, Fixed16_16};
    use std::path::PathBuf;

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

    pub(crate) fn make_game_state_with_level(level_name: &str) -> GameState {
        let mut gs = GameState::new(level_name);
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

    pub(crate) fn make_game_state() -> GameState {
        make_game_state_with_level("E1M1")
    }

    #[test]
    fn wav_from_f32_mono_writes_valid_header_and_payload_len() {
        let samples = [0.0_f32, 0.5_f32, -0.5_f32, 1.0_f32];
        let wav = wav_from_f32_mono(&samples, 44_100);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        let data_len = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]);
        assert_eq!(data_len, 8, "4 mono i16 samples should be 8 bytes");
        assert_eq!(wav.len(), 44 + data_len as usize);
    }

    #[test]
    fn score_total_ticks_sums_all_event_deltas() {
        let score = MusScore {
            header: doom_audio::mus::MusHeader {
                score_length: 0,
                score_start: 0,
                primary_channels: 0,
                secondary_channels: 0,
                instrument_count: 0,
            },
            instruments: Vec::new(),
            events: vec![
                (0, doom_audio::MusEvent::MeasureEnd),
                (7, doom_audio::MusEvent::MeasureEnd),
                (9, doom_audio::MusEvent::ScoreEnd),
            ],
        };
        assert_eq!(score_total_ticks(&score), 16);
    }

    pub(crate) fn make_doom_game() -> DoomGame {
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

    fn make_test_colormap_with_special_row_32(value: u8) -> ColormapCache {
        let mut data = vec![
            0u8;
            doom_renderer::colormap::COLORMAP_ROWS
                * doom_renderer::colormap::COLORMAP_SIZE
        ];
        for row in 0..doom_renderer::colormap::COLORMAP_ROWS {
            let start = row * doom_renderer::colormap::COLORMAP_SIZE;
            data[start..start + doom_renderer::colormap::COLORMAP_SIZE].fill(row as u8);
        }
        let row_32 = 32 * doom_renderer::colormap::COLORMAP_SIZE;
        data[row_32..row_32 + doom_renderer::colormap::COLORMAP_SIZE].fill(value);
        ColormapCache::from_test_data(data)
    }

    #[test]
    fn current_fixed_colormap_returns_none_without_invulnerability() {
        let game = DoomGame::new_with_compat(
            make_game_state(),
            make_test_level(),
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
            Some(make_test_colormap_with_special_row_32(0xA5)),
            false,
            None,
            None,
            std::collections::HashMap::new(),
            CompatibilityProfile::VanillaStrict,
        );

        assert!(
            game.current_fixed_colormap().is_none(),
            "no fixed colormap should be selected when invulnerability is inactive"
        );
    }

    #[test]
    fn current_fixed_colormap_uses_row_32_in_vanilla_strict() {
        let mut game = DoomGame::new_with_compat(
            make_game_state(),
            make_test_level(),
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
            Some(make_test_colormap_with_special_row_32(0xA5)),
            false,
            None,
            None,
            std::collections::HashMap::new(),
            CompatibilityProfile::VanillaStrict,
        );
        game.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] = 1;

        let row = game
            .current_fixed_colormap()
            .expect("invulnerability should select a fixed colormap");
        assert_eq!(row[0], 0xA5, "strict mode must use COLORMAP row 32");
    }

    #[test]
    fn current_fixed_colormap_keeps_extended_fallback() {
        let mut game = DoomGame::new_with_compat(
            make_game_state(),
            make_test_level(),
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
            Some(make_test_colormap_with_special_row_32(0xA5)),
            false,
            None,
            None,
            std::collections::HashMap::new(),
            CompatibilityProfile::Extended,
        );
        game.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] = 1;

        let row = game
            .current_fixed_colormap()
            .expect("invulnerability should select a fixed colormap");
        assert_eq!(
            row[0],
            doom_renderer::INVULN_COLORMAP[0],
            "extended mode should keep the synthetic fallback"
        );
        assert_ne!(
            row[0], 0xA5,
            "extended mode must not use WAD row 32 directly"
        );
    }

    fn make_minimal_blockmap() -> Blockmap {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        Blockmap::parse_lump(&bm_data).expect("blockmap parse")
    }

    fn make_walk_exit_level_named(level_name: &str) -> Level {
        let reject = Reject::parse_lump(&[0u8], 2).expect("reject parse");
        Level {
            name: level_name.to_string(),
            things: vec![],
            linedefs: vec![doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004,
                special: 52, // W1 exit
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            }],
            sidedefs: vec![
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"\0\0\0\0\0\0\0\0",
                    lower_texture: *b"\0\0\0\0\0\0\0\0",
                    middle_texture: *b"\0\0\0\0\0\0\0\0",
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"\0\0\0\0\0\0\0\0",
                    lower_texture: *b"\0\0\0\0\0\0\0\0",
                    middle_texture: *b"\0\0\0\0\0\0\0\0",
                    sector: 1,
                },
            ],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: -10 },
                doom_map::Vertex { x: 0, y: 10 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    fn make_walk_exit_level() -> Level {
        make_walk_exit_level_named("E1M1")
    }

    fn build_test_wad_bytes_from_lumps(
        kind: &[u8; 4],
        lump_payloads: Vec<([u8; 8], Vec<u8>)>,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(kind);
        data.extend_from_slice(&(lump_payloads.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());

        let mut offsets = Vec::new();
        for (_, payload) in &lump_payloads {
            let pos = data.len();
            data.extend_from_slice(payload);
            offsets.push((pos, payload.len()));
        }

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());
        for (i, (name, _)) in lump_payloads.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());
            data.extend_from_slice(name);
        }

        data
    }

    fn build_test_wad_from_lumps(lump_payloads: Vec<([u8; 8], Vec<u8>)>) -> WadFile {
        let data = build_test_wad_bytes_from_lumps(b"IWAD", lump_payloads);
        WadFile::parse(data).expect("test WAD parse")
    }

    fn build_test_wad_stack_from_lumps(
        iwad_lumps: Vec<([u8; 8], Vec<u8>)>,
        pwad_lumps: Vec<([u8; 8], Vec<u8>)>,
    ) -> WadStack {
        let mut stack = WadStack::new();
        stack
            .push_iwad(build_test_wad_bytes_from_lumps(b"IWAD", iwad_lumps))
            .expect("IWAD test stack push");
        if !pwad_lumps.is_empty() {
            stack
                .push_pwad(build_test_wad_bytes_from_lumps(b"PWAD", pwad_lumps))
                .expect("PWAD test stack push");
        }
        stack
    }

    fn build_minimal_wad_for_maps(map_names: &[&str]) -> WadFile {
        let mut lump_payloads: Vec<([u8; 8], Vec<u8>)> = Vec::new();

        for &map_name in map_names {
            let mut marker = [0u8; 8];
            for (i, &b) in map_name.as_bytes().iter().take(8).enumerate() {
                marker[i] = b.to_ascii_uppercase();
            }

            let mut sector_data = vec![0u8; 26];
            sector_data[0..2].copy_from_slice(&0i16.to_le_bytes());
            sector_data[2..4].copy_from_slice(&128i16.to_le_bytes());
            sector_data[4..12].copy_from_slice(b"FLAT1\0\0\0");
            sector_data[12..20].copy_from_slice(b"FLAT2\0\0\0");
            sector_data[20..22].copy_from_slice(&192i16.to_le_bytes());

            let mut vert_data = vec![0u8; 16];
            let verts = [(0i16, 0i16), (64, 0), (64, 64), (0, 64)];
            for (i, (x, y)) in verts.iter().enumerate() {
                vert_data[i * 4..i * 4 + 2].copy_from_slice(&x.to_le_bytes());
                vert_data[i * 4 + 2..i * 4 + 4].copy_from_slice(&y.to_le_bytes());
            }

            let mut sd_data = vec![0u8; 120];
            for i in 0..4 {
                sd_data[i * 30 + 20..i * 30 + 28].copy_from_slice(b"WALL1\0\0\0");
                sd_data[i * 30 + 28..i * 30 + 30].copy_from_slice(&0u16.to_le_bytes());
            }

            let mut ld_data = vec![0u8; 56];
            let edges = [(0u16, 1u16), (1, 2), (2, 3), (3, 0)];
            for (i, (from, to)) in edges.iter().enumerate() {
                let b = &mut ld_data[i * 14..i * 14 + 14];
                b[0..2].copy_from_slice(&from.to_le_bytes());
                b[2..4].copy_from_slice(&to.to_le_bytes());
                b[10..12].copy_from_slice(&(i as u16).to_le_bytes());
                b[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
            }

            let mut seg_data = vec![0u8; 12];
            seg_data[2..4].copy_from_slice(&1u16.to_le_bytes());

            let mut ss_data = vec![0u8; 4];
            ss_data[0..2].copy_from_slice(&1u16.to_le_bytes());

            let node_data = vec![];

            let mut thing_data = vec![0u8; 10];
            thing_data[6..8].copy_from_slice(&1u16.to_le_bytes());
            thing_data[8..10].copy_from_slice(&7u16.to_le_bytes());

            let reject_data = vec![0u8; 1];

            let mut bm_data = vec![0u8; 14];
            bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
            bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
            bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
            bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
            bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());

            lump_payloads.push((marker, Vec::new()));
            lump_payloads.push((*b"THINGS\0\0", thing_data));
            lump_payloads.push((*b"LINEDEFS", ld_data));
            lump_payloads.push((*b"SIDEDEFS", sd_data));
            lump_payloads.push((*b"VERTEXES", vert_data));
            lump_payloads.push((*b"SEGS\0\0\0\0", seg_data));
            lump_payloads.push((*b"SSECTORS", ss_data));
            lump_payloads.push((*b"NODES\0\0\0", node_data));
            lump_payloads.push((*b"SECTORS\0", sector_data));
            lump_payloads.push((*b"REJECT\0\0", reject_data));
            lump_payloads.push((*b"BLOCKMAP", bm_data));
        }

        build_test_wad_from_lumps(lump_payloads)
    }

    fn build_minimal_wad_stack_for_maps(map_names: &[&str]) -> WadStack {
        let wad = build_minimal_wad_for_maps(map_names);
        let mut stack = WadStack::new();
        // Rebuild from bytes so the stack owns the same map data in IWAD position.
        let mut lump_payloads: Vec<([u8; 8], Vec<u8>)> = Vec::new();
        for lump in wad.lumps() {
            let mut name = [0u8; 8];
            let raw = lump.name.as_str().as_bytes();
            for (idx, &byte) in raw.iter().take(8).enumerate() {
                name[idx] = byte;
            }
            lump_payloads.push((name, wad.lump_data(lump).to_vec()));
        }
        stack
            .push_iwad(build_test_wad_bytes_from_lumps(b"IWAD", lump_payloads))
            .expect("IWAD test stack push");
        stack
    }

    fn unique_temp_log_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        path.push(format!("doom-rs-{name}-{nanos}.log"));
        path
    }

    #[allow(dead_code)]
    fn music_library_for(
        level_name: &str,
        data: Vec<u8>,
    ) -> std::collections::HashMap<doom_wad::lump::LumpName, std::sync::Arc<[u8]>> {
        let mut library = std::collections::HashMap::new();
        if let Some(lump) = music_lump_for_map(level_name) {
            library.insert(
                doom_wad::lump::LumpName::from_str(&lump),
                std::sync::Arc::<[u8]>::from(data),
            );
        }
        library
    }

    #[allow(dead_code)]
    fn wait_for_music_requests(audio: &AudioSystem, expected: usize) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
        while std::time::Instant::now() < deadline {
            if audio.debug_music_start_count() >= expected {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: AutomapState toggle works through DoomGame
    // -----------------------------------------------------------------------

    #[test]
    fn automap_toggle_via_tab_pressed() {
        let mut game = make_doom_game();
        assert!(!game.automap.active, "automap must start inactive");

        // Simulate Tab press.
        let input = TicInput {
            tab_pressed: true,
            ..TicInput::default()
        };
        game.tick(input);
        assert!(
            game.automap.active,
            "automap must be active after first Tab"
        );

        // Tab again.
        let input2 = TicInput {
            tab_pressed: true,
            ..TicInput::default()
        };
        game.tick(input2);
        assert!(
            !game.automap.active,
            "automap must be inactive after second Tab"
        );
    }

    // -----------------------------------------------------------------------
    // Test 2: AutomapState follows player position
    // -----------------------------------------------------------------------

    #[test]
    fn automap_follows_player_position() {
        let mut game = make_doom_game();

        // Open the automap.
        let input = TicInput {
            tab_pressed: true,
            ..TicInput::default()
        };
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
        assert_eq!(
            data.ready_weapon, 1,
            "pistol start weapon must be 1 (pistol)"
        );
    }

    // -----------------------------------------------------------------------
    // Test 6: Cheat message appears and expires
    // -----------------------------------------------------------------------

    #[test]
    fn cheat_message_expires_after_ticking() {
        let mut game = make_doom_game();
        // Manually set a cheat message.
        game.hud_messages.push("Test message".to_string(), 3);

        // Tick 3 times to expire the message.
        for _ in 0..3 {
            game.tick(TicInput::default());
        }
        // After 3 ticks the tics should have reached 0 and on the next tick
        // it should be cleared.
        game.tick(TicInput::default());
        assert!(
            game.hud_messages.is_empty(),
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
        assert!(
            !game.automap_full_reveal,
            "automap_full_reveal must start false"
        );
        assert!(
            game.hud_messages.is_empty(),
            "cheat_message must start None"
        );
        assert_eq!(
            game.player_view_height, PLAYER_HEIGHT,
            "player view height must start at the normal standing height"
        );
    }

    #[test]
    fn doom_game_new_syncs_weapon_anim_to_player_weapon() {
        let mut gs = make_game_state();
        gs.player.weapons[WeaponType::Shotgun as usize] = true;
        gs.player.weapon = WeaponType::Shotgun;
        let game = DoomGame::new(
            gs,
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
        );

        assert_eq!(
            game.weapon_anim.current.sprite_name, *b"SHTGA0\0\0",
            "weapon overlay must initialize from the player's psprite state, not always pistol"
        );
    }

    #[test]
    fn player_attack_tick_triggers_weapon_flash_from_psprites() {
        let mut game = make_doom_game();
        for _ in 0..24 {
            game.tick(TicInput::default());
        }

        // Vanilla A_FirePistol runs on PISTOL2 (4 tics into the fire animation)
        // and is what starts the muzzle-flash overlay.
        for _ in 0..5 {
            game.tick(TicInput {
                buttons: doom_types::bt::BT_ATTACK,
                ..TicInput::default()
            });
        }

        assert!(
            game.weapon_anim.current.flash_active,
            "player attack should drive the overlay flash from the game psprite state"
        );
        assert_eq!(
            game.weapon_anim.current.flash_sprite, *b"PISGD0\0\0",
            "pistol flash should come from the current psprite frame, not a hard-coded patch name"
        );
    }

    #[test]
    fn weapon_anim_bobs_when_player_has_momentum() {
        let mut game = make_doom_game();
        game.gs
            .mobjslab
            .get_mut(game.gs.player.handle)
            .expect("player mobj must exist")
            .momx = Fixed16_16::from_int(4);

        game.tick(TicInput::default());

        assert!(
            game.weapon_anim.bob.offset_x != 0 || game.weapon_anim.bob.offset_y != 0,
            "moving player momentum must drive first-person weapon sway"
        );
    }

    #[test]
    fn weapon_switch_lowers_then_raises_new_weapon() {
        let mut game = make_doom_game();
        game.gs.player.weapons[WeaponType::Shotgun as usize] = true;
        for _ in 0..24 {
            game.tick(TicInput::default());
        }

        game.tick(TicInput {
            buttons: doom_types::bt::BT_CHANGE | (2u8 << 3),
            ..TicInput::default()
        });

        assert_eq!(
            game.gs.player.pending_weapon,
            Some(WeaponType::Shotgun),
            "weapon switch should stage the new weapon in the gameplay psprite state"
        );
        assert!(
            game.weapon_anim.current.transition == doom_renderer::WeaponTransition::Lowering,
            "weapon switch should start by lowering the current weapon psprite"
        );

        for _ in 0..80 {
            game.tick(TicInput::default());
        }

        assert_eq!(
            game.weapon_anim.current.sprite_name, *b"SHTGA0\0\0",
            "after the transition finishes the overlay must show the new weapon"
        );
        assert!(
            game.gs.player.pending_weapon.is_none(),
            "pending weapon must clear once the new weapon is raised"
        );
    }

    #[test]
    fn dead_player_view_height_lowers_toward_floor() {
        let mut game = make_doom_game();
        game.gs.player.apply_damage(200);
        assert!(
            game.gs.player.is_dead(),
            "player must be dead for death view test"
        );

        game.tick(TicInput::default());
        assert_eq!(
            game.player_view_height,
            PLAYER_HEIGHT - 1,
            "dead player view height must start lowering one unit per tic"
        );

        for _ in 0..128 {
            game.tick(TicInput::default());
        }

        assert_eq!(
            game.player_view_height, DEAD_PLAYER_VIEW_HEIGHT,
            "dead player view height must clamp to the low vanilla death view"
        );
    }

    #[test]
    fn alive_player_view_height_resets_after_revival() {
        let mut game = make_doom_game();
        game.player_view_height = DEAD_PLAYER_VIEW_HEIGHT;

        game.tick(TicInput::default());
        assert_eq!(
            game.player_view_height, PLAYER_HEIGHT,
            "alive player must render from the normal standing height"
        );
    }

    #[test]
    fn debug_log_includes_far_enemy_ai_state() {
        let log_path = unique_temp_log_path("far-enemy");
        let log_file = std::fs::File::create(&log_path).expect("temp debug log must open");
        let mut game = DoomGame::new(
            make_game_state(),
            make_test_level(),
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
            None,
            false,
            Some(log_file),
            None,
            std::collections::HashMap::new(),
        );

        let mut imp = Mobj::new(
            MobjKind::Imp,
            Fixed16_16::from_int(5000),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        imp.health = 60;
        imp.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;
        imp.state = doom_game::MOBJINFO[MobjKind::Imp as usize].spawn_state;
        imp.tics = 10;
        imp.threshold = 60;
        imp.reactiontime = 18;
        imp.movecount = 7;
        imp.subsector = 3;
        let imp_handle = game.gs.mobjslab.alloc(imp);

        game.dlog_live_enemies();
        game.debug_log
            .as_ref()
            .expect("debug log should still be present")
            .sync_all()
            .expect("debug log should flush");
        drop(game);

        let log_text = std::fs::read_to_string(&log_path).expect("debug log should be readable");
        let _ = std::fs::remove_file(&log_path);

        assert!(
            log_text.contains("enemy idx="),
            "debug log should include enemy handle identity, got: {log_text}"
        );
        assert!(
            log_text.contains("Imp"),
            "debug log should include the far enemy kind, got: {log_text}"
        );
        assert!(
            log_text.contains(&format!("idx={}", imp_handle.index)),
            "debug log should include the far enemy handle index, got: {log_text}"
        );
        assert!(
            log_text.contains("threshold=60")
                && log_text.contains("reaction=18")
                && log_text.contains("movecount=7")
                && log_text.contains("subsector=3"),
            "debug log should include AI state fields for diagnosis, got: {log_text}"
        );
    }

    #[test]
    fn debug_log_ignores_frame_timing_diagnostics() {
        let log_path = unique_temp_log_path("frame-timing-ignored");
        let log_file = std::fs::File::create(&log_path).expect("temp debug log must open");
        let mut game = DoomGame::new(
            make_game_state(),
            make_test_level(),
            None,
            std::collections::HashMap::new(),
            None,
            None,
            None,
            None,
            false,
            Some(log_file),
            None,
            std::collections::HashMap::new(),
        );

        <DoomGame as DoomApp>::on_frame_timings(&mut game, 5_000, 18_000, 2_000);
        game.debug_log
            .as_ref()
            .expect("debug log should still be present")
            .sync_all()
            .expect("debug log should flush");
        drop(game);

        let log_text = std::fs::read_to_string(&log_path).expect("debug log should be readable");
        let _ = std::fs::remove_file(&log_path);

        assert!(
            !log_text.contains("slow-frame"),
            "frame timing diagnostics should not be written to the debug log, got: {log_text}"
        );
    }

    // -----------------------------------------------------------------------
    // Test 8: draw_cheat_message_overlay does not panic
    // -----------------------------------------------------------------------

    #[test]
    fn draw_cheat_message_overlay_does_not_panic() {
        let mut fb = Framebuffer::new();
        let mut queue = doom_renderer::HudMessageQueue::new(4);
        queue.push("Degreelessness Mode On".to_string(), 100);
        draw_cheat_message_overlay(&mut fb, &queue);
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
        let mut queue = doom_renderer::HudMessageQueue::new(4);
        queue.push("".to_string(), 100);
        draw_cheat_message_overlay(&mut fb, &queue);
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
            let input = TicInput {
                chatchar: ch,
                ..TicInput::default()
            };
            game.tick(input);
        }

        assert!(
            game.gs.player.god_mode,
            "god_mode must be true after typing iddqd via chatchar"
        );
        assert!(
            !game.hud_messages.is_empty(),
            "cheat_message must be set after cheat activation"
        );
    }

    #[test]
    fn locked_door_feedback_sets_overlay_message() {
        let mut game = make_doom_game();

        game.handle_sound_events([doom_game::SoundRequest::PlayerUseLockedDoor(
            doom_game::LockedDoorColor::Blue,
        )]);

        assert_eq!(
            game.hud_messages
                .active_messages()
                .first()
                .map(|m| m.text()),
            Some("You need a blue key to open this door"),
            "locked door feedback should surface the classic Doom HUD message"
        );
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn game_without_title_starts_level_music_immediately() {
        let audio = AudioSystem::try_open_null().expect("null audio must succeed");
        let game = DoomGame::new(
            make_game_state(),
            make_test_level(),
            Some(audio),
            music_library_for("E1M1", vec![1, 2, 3, 4]),
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            std::collections::HashMap::new(),
        );

        let audio = game.audio.as_ref().expect("audio must be present");
        wait_for_music_requests(audio, 1);
        assert_eq!(audio.debug_music_start_count(), 1);
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn starting_game_from_title_starts_level_music() {
        let audio = AudioSystem::try_open_null().expect("null audio must succeed");
        let mut game = DoomGame::new(
            make_game_state(),
            make_test_level(),
            Some(audio),
            music_library_for("E1M1", vec![1, 2, 3, 4]),
            None,
            None,
            None,
            None,
            true,
            None,
            None,
            std::collections::HashMap::new(),
        );

        for _ in 0..3 {
            let input = TicInput {
                menu_select: true,
                ..TicInput::default()
            };
            game.tick(input);
        }

        assert!(
            game.title_screen.is_none(),
            "title screen should be dismissed"
        );
        let audio = game.audio.as_ref().expect("audio must be present");
        wait_for_music_requests(audio, 1);
        assert_eq!(audio.debug_music_start_count(), 1);
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn start_level_music_resolves_from_current_level_name() {
        let audio = AudioSystem::try_open_null().expect("null audio must succeed");
        let mut music_library = music_library_for("E1M1", vec![1, 2, 3, 4]);
        music_library.extend(music_library_for("MAP01", vec![5, 6, 7, 8]));
        let mut game = DoomGame::new(
            make_game_state(),
            make_test_level(),
            Some(audio),
            music_library,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            std::collections::HashMap::new(),
        );

        let audio = game.audio.as_ref().expect("audio must be present");
        wait_for_music_requests(audio, 1);
        assert_eq!(audio.debug_music_start_count(), 1);

        game.gs.level_name = "MAP01".to_string();
        game.start_level_music();

        wait_for_music_requests(audio, 2);
        assert_eq!(audio.debug_music_start_count(), 2);
    }

    #[test]
    fn load_music_library_includes_intermission_lumps() {
        let wad = build_test_wad_stack_from_lumps(
            vec![
                (*b"D_INTER\0", vec![1, 2, 3, 4]),
                (*b"D_DM2INT", vec![5, 6, 7, 8]),
            ],
            Vec::new(),
        );

        let library = load_music_library(&wad);

        assert_eq!(
            library.get(&doom_wad::lump::LumpName::from_str("D_INTER")),
            Some(&std::sync::Arc::<[u8]>::from(vec![1, 2, 3, 4]))
        );
        assert_eq!(
            library.get(&doom_wad::lump::LumpName::from_str("D_DM2INT")),
            Some(&std::sync::Arc::<[u8]>::from(vec![5, 6, 7, 8]))
        );
    }

    #[test]
    fn held_attack_on_exit_does_not_skip_intermission_summary() {
        let mut game = DoomGame::new(
            make_game_state(),
            make_walk_exit_level(),
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
        );
        game.gs
            .mobjslab
            .get_mut(game.gs.player.handle)
            .expect("player mobj should exist")
            .momx = Fixed16_16::from_int(40);

        game.tick(TicInput {
            buttons: doom_types::bt::BT_ATTACK,
            ..TicInput::default()
        });
        game.tick(TicInput {
            buttons: doom_types::bt::BT_ATTACK,
            ..TicInput::default()
        });

        let renderer = game
            .intermission_renderer
            .as_ref()
            .expect("intermission renderer should exist after level exit");
        assert!(
            !renderer.is_done(),
            "held attack from gameplay must not auto-skip the intermission summary on entry"
        );
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn level_exit_starts_intermission_music_then_next_level_music() {
        let audio = AudioSystem::try_open_null().expect("null audio must succeed");
        let mut music_library = music_library_for("E1M1", vec![1, 2, 3, 4]);
        music_library.extend(music_library_for("E1M2", vec![5, 6, 7, 8]));
        music_library.insert(
            doom_wad::lump::LumpName::from_str("D_INTER"),
            std::sync::Arc::<[u8]>::from(vec![9, 10, 11, 12]),
        );
        let mut game = DoomGame::new(
            make_game_state(),
            make_walk_exit_level(),
            Some(audio),
            music_library,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            std::collections::HashMap::new(),
        );
        game.attach_wad_for_transitions(
            Skill::Medium,
            build_minimal_wad_stack_for_maps(&["E1M1", "E1M2"]),
        );
        game.gs
            .mobjslab
            .get_mut(game.gs.player.handle)
            .expect("player mobj should exist")
            .momx = Fixed16_16::from_int(40);

        wait_for_music_requests(game.audio.as_ref().expect("audio must be present"), 1);

        game.tick(TicInput::default());
        wait_for_music_requests(game.audio.as_ref().expect("audio must be present"), 2);
        assert_eq!(
            game.audio
                .as_ref()
                .expect("audio must be present")
                .debug_music_start_count(),
            2,
            "entering intermission should start the intermission track"
        );

        for _ in 0..350 {
            game.tick(TicInput::default());
        }

        wait_for_music_requests(game.audio.as_ref().expect("audio must be present"), 3);
        assert_eq!(
            game.audio
                .as_ref()
                .expect("audio must be present")
                .debug_music_start_count(),
            3,
            "loading the next level should start its map music after intermission"
        );
        assert_eq!(game.gs.level_name, "E1M2");
    }

    #[test]
    fn level_exit_renders_intermission_summary() {
        let mut game = DoomGame::new(
            make_game_state(),
            make_walk_exit_level(),
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
        );
        game.gs
            .mobjslab
            .get_mut(game.gs.player.handle)
            .expect("player mobj should exist")
            .momx = Fixed16_16::from_int(40);

        game.tick(TicInput::default());

        let mut fb = Framebuffer::new();
        game.render(&mut fb);
        assert!(
            fb.get_pixel(160, 199) == Some(0),
            "level exit should replace the gameplay HUD with the intermission screen"
        );
    }

    #[test]
    fn level_exit_advances_to_next_level_after_intermission() {
        let mut game = DoomGame::new(
            make_game_state(),
            make_walk_exit_level(),
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
        );
        game.attach_wad_for_transitions(
            Skill::Medium,
            build_minimal_wad_stack_for_maps(&["E1M1", "E1M2"]),
        );
        game.gs
            .mobjslab
            .get_mut(game.gs.player.handle)
            .expect("player mobj should exist")
            .momx = Fixed16_16::from_int(40);

        game.tick(TicInput::default());
        for _ in 0..350 {
            game.tick(TicInput::default());
        }

        assert_eq!(
            game.gs.level_name, "E1M2",
            "after finishing E1M1 and waiting through intermission, the next level should load"
        );
    }

    #[test]
    fn doom2_level_exit_advances_to_map02_after_intermission() {
        let mut game = DoomGame::new(
            make_game_state_with_level("MAP01"),
            make_walk_exit_level_named("MAP01"),
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
        );
        game.attach_wad_for_transitions(
            Skill::Medium,
            build_minimal_wad_stack_for_maps(&["MAP01", "MAP02"]),
        );
        game.gs
            .mobjslab
            .get_mut(game.gs.player.handle)
            .expect("player mobj should exist")
            .momx = Fixed16_16::from_int(40);

        game.tick(TicInput::default());
        for _ in 0..350 {
            game.tick(TicInput::default());
        }

        assert_eq!(
            game.gs.level_name, "MAP02",
            "after finishing MAP01 and waiting through intermission, the next level should load"
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
        assert!(!game.switch_list.is_empty(), "SwitchList must not be empty");
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
        assert_eq!(
            game.prev_health, 100,
            "prev_health still 100 after no-damage tick"
        );

        // Simulate damage between ticks.
        let handle = game.gs.player.handle;
        if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
            mo.health = 80;
        }
        game.gs.player.set_health_capped(80, 100);
        game.tick(TicInput::default());
        assert_eq!(
            game.prev_health, 80,
            "prev_health updated to 80 after damage"
        );

        // Second damage event.
        if let Some(mo) = game.gs.mobjslab.get_mut(handle) {
            mo.health = 50;
        }
        game.gs.player.set_health_capped(50, 100);
        game.tick(TicInput::default());
        assert_eq!(
            game.prev_health, 50,
            "prev_health updated to 50 after second damage"
        );
    }

    // -----------------------------------------------------------------------
    // Test 24: CLI args parse --deh flag correctly
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_parse_deh_flag() {
        let args =
            Args::try_parse_from(["doom-app", "--wad", "doom1.wad", "--deh", "my_patch.deh"]);
        assert!(args.is_ok(), "args with --deh must parse successfully");
        let args = args.expect("args parse must succeed");
        assert_eq!(args.deh.as_deref(), Some("my_patch.deh"));
    }

    #[test]
    fn cli_args_parse_iwad_and_pwad_flags() {
        let args = Args::try_parse_from([
            "doom-app",
            "--iwad",
            "doom2.wad",
            "--pwad",
            "winterbase.wad",
            "--pwad",
            "fixes.wad",
        ])
        .expect("args with explicit IWAD/PWAD flags should parse");

        assert_eq!(args.iwad, std::path::PathBuf::from("doom2.wad"));
        assert_eq!(
            args.pwad,
            vec![
                std::path::PathBuf::from("winterbase.wad"),
                std::path::PathBuf::from("fixes.wad")
            ]
        );
    }

    #[test]
    fn default_warp_map_prefers_map_markers_present_in_stack() {
        let stack = build_test_wad_stack_from_lumps(
            vec![(*b"E1M1\0\0\0\0", vec![]), (*b"THINGS\0\0", vec![])],
            vec![
                (*b"MAP01\0\0\0", vec![]),
                (*b"THINGS\0\0", vec![]),
                (*b"LINEDEFS", vec![]),
                (*b"SIDEDEFS", vec![]),
                (*b"VERTEXES", vec![]),
                (*b"SEGS\0\0\0\0", vec![]),
                (*b"SSECTORS", vec![]),
                (*b"NODES\0\0\0", vec![]),
                (*b"SECTORS\0", vec![]),
                (*b"REJECT\0\0", vec![]),
                (*b"BLOCKMAP", vec![]),
            ],
        );

        assert_eq!(default_warp_map(&stack), "MAP01");
    }

    // -----------------------------------------------------------------------
    // Test 25: CLI args parse --server flag with port
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_parse_server_flag() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad", "--server", "5029"]);
        assert!(args.is_ok(), "args with --server must parse successfully");
        let args = args.expect("args parse must succeed");
        assert_eq!(args.server, Some(5029));
    }

    // -----------------------------------------------------------------------
    // Test 26: CLI args parse --connect flag with address
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_parse_connect_flag() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad",
            "doom1.wad",
            "--connect",
            "127.0.0.1:5029",
        ]);
        assert!(args.is_ok(), "args with --connect must parse successfully");
        let args = args.expect("args parse must succeed");
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
        let patch = patch.expect("patch must parse");
        assert_eq!(patch.things.len(), 1);
        assert_eq!(patch.things[0].hit_points, Some(200));
    }

    // -----------------------------------------------------------------------
    // Test 29: CLI args --deh defaults to None
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_deh_defaults_to_none() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad"]);
        assert!(args.is_ok());
        let args = args.expect("args parse must succeed");
        assert!(args.deh.is_none(), "--deh must default to None");
    }

    // -----------------------------------------------------------------------
    // Test 30: CLI args --server defaults to None
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_server_defaults_to_none() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad"]);
        assert!(args.is_ok());
        let args = args.expect("args parse must succeed");
        assert!(args.server.is_none(), "--server must default to None");
    }

    // -----------------------------------------------------------------------
    // Test 31: CLI args --connect defaults to None
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_connect_defaults_to_none() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad"]);
        assert!(args.is_ok());
        let args = args.expect("args parse must succeed");
        assert!(args.connect.is_none(), "--connect must default to None");
    }

    #[test]
    fn cli_args_compat_defaults_to_extended() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad"]);
        assert!(args.is_ok());
        let args = args.expect("args parse must succeed");
        assert_eq!(
            args.compat,
            CompatibilityProfile::Extended,
            "--compat must default to extended"
        );
    }

    #[test]
    fn cli_args_compat_parses_vanilla_strict() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad",
            "doom1.wad",
            "--compat",
            "vanilla-strict",
        ]);
        assert!(args.is_ok());
        let args = args.expect("args parse must succeed");
        assert_eq!(
            args.compat,
            CompatibilityProfile::VanillaStrict,
            "--compat vanilla-strict must parse"
        );
    }

    #[test]
    fn cli_args_compat_rejects_invalid_value() {
        let err = Args::try_parse_from(["doom-app", "--wad", "doom1.wad", "--compat", "banana"])
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid value"), "{msg}");
        assert!(msg.contains("banana"), "{msg}");
    }

    #[test]
    fn mode_validation_allows_playdemo_capture() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad",
            "doom1.wad",
            "--playdemo",
            "repro.lmp",
            "--capture",
            "frame.bmp",
        ])
        .expect("args with playdemo capture should parse");

        assert!(
            validate_mode_args(&args).is_ok(),
            "--capture should be allowed with --playdemo"
        );
    }

    #[test]
    fn mode_validation_rejects_record_capture() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad",
            "doom1.wad",
            "--record",
            "repro.lmp",
            "--capture",
            "frame.bmp",
        ])
        .expect("args with record capture should parse");

        assert_eq!(
            validate_mode_args(&args).unwrap_err(),
            "--capture cannot be combined with --record"
        );
    }

    #[test]
    fn mode_validation_rejects_server_capture() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad",
            "doom1.wad",
            "--server",
            "5029",
            "--capture",
            "frame.bmp",
        ])
        .expect("args with server capture should parse");

        assert_eq!(
            validate_mode_args(&args).unwrap_err(),
            "--capture cannot be combined with --server"
        );
    }

    #[test]
    fn mode_validation_rejects_connect_capture() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad",
            "doom1.wad",
            "--connect",
            "127.0.0.1:5029",
            "--capture",
            "frame.bmp",
        ])
        .expect("args with connect capture should parse");

        assert_eq!(
            validate_mode_args(&args).unwrap_err(),
            "--capture cannot be combined with --connect"
        );
    }

    #[test]
    fn cli_args_parse_export_sfx_wav() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad",
            "doom1.wad",
            "--export-sfx-wav",
            "pistol.wav",
            "--sfx-name",
            "DSPISTOL",
        ]);
        assert!(
            args.is_ok(),
            "args with --export-sfx-wav must parse successfully"
        );
        let args = args.expect("args parse must succeed");
        assert_eq!(
            args.export_sfx_wav,
            Some(std::path::PathBuf::from("pistol.wav"))
        );
        assert_eq!(args.sfx_name, Some("DSPISTOL".to_string()));
    }

    #[test]
    fn export_sfx_wav_for_name_generates_valid_wav() {
        let mut sfx_data = Vec::new();
        sfx_data.extend_from_slice(&3u16.to_le_bytes()); // format
        sfx_data.extend_from_slice(&11_025u16.to_le_bytes()); // sample_rate
        sfx_data.extend_from_slice(&100u32.to_le_bytes()); // sample_count
        sfx_data.extend(vec![128u8; 100]); // 100 samples of silence

        let stack = build_test_wad_stack_from_lumps(vec![(*b"DSPISTOL", sfx_data.clone())], vec![]);

        let temp_dir = tempfile::tempdir().expect("tempdir must succeed");
        let out_path = temp_dir.path().join("pistol.wav");

        let result = export_sfx_wav_for_name(&stack, "DSPISTOL", &out_path);
        assert!(result.is_ok(), "SFX export should succeed");

        let wav_data = std::fs::read(&out_path).expect("read must succeed");
        // RIFF + 36 byte format/data headers + 100*2 bytes of 16-bit PCM = 244 bytes
        assert_eq!(wav_data[0..4], *b"RIFF");
        assert_eq!(wav_data[8..12], *b"WAVE");
        assert_eq!(wav_data.len(), 44 + 200);
    }

    #[test]
    fn mode_validation_rejects_turn_based_netplay() {
        let args = Args::try_parse_from([
            "doom-app",
            "--wad",
            "doom1.wad",
            "--turn-based",
            "--connect",
            "127.0.0.1:5029",
        ])
        .expect("args with turn-based connect should parse");

        assert_eq!(
            validate_mode_args(&args).unwrap_err(),
            "--turn-based is only supported in single-player mode"
        );
    }

    #[test]
    fn headless_capture_ticks_then_renders_once() {
        let mut app = FakeCaptureApp {
            active_palette: 3,
            ..Default::default()
        };

        let capture = capture_headless_frame(&mut app, 4);

        assert_eq!(app.ticks, 4, "capture must tick the app requested times");
        assert_eq!(app.renders, 1, "capture must render exactly one frame");
        assert_eq!(
            capture.active_palette, 3,
            "capture must preserve the app's active palette"
        );
        assert_eq!(
            capture.framebuffer.get_pixel(0, 0),
            Some(4),
            "rendered framebuffer should contain the app's last rendered content"
        );
    }

    // -----------------------------------------------------------------------
    // Test 32: Sound Ripples Feature Integration
    // -----------------------------------------------------------------------

    #[test]
    #[cfg(feature = "sound_ripples")]
    fn sound_ripples_spawned_on_sound_events_in_tick() {
        let mut game = make_doom_game();
        game.title_screen = None; // Disable title screen so gameplay is active
        game.phase_controller = doom_game::GamePhaseController::new(doom_game::MapId::new(1, 1)); // Ensure phase is Playing

        // We just use the player's handle instead since it already exists.

        // Enqueue a weapon fire sound.
        game.gs
            .sound
            .sound_queue
            .push(doom_game::SoundRequest::PlayerWeaponFire(
                doom_types::weapons::WeaponType::Pistol,
            ));

        // Open menu to pause the game, so `gs.tick()` doesn't clear the sound queue we just pushed!
        game.menu.open();

        // Let the game tick process the sound queue.

        game.tick(TicInput::default());

        // Check if effects were spawned.

        assert_eq!(
            game.cogmind_state.effects.effects.len(),
            9,
            "sound ripples must spawn 9 particles"
        );
    }

    #[test]
    fn cli_args_parse_pathfind() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad", "--pathfind", "0,5"]);
        assert!(args.is_ok(), "args with --pathfind must parse successfully");
        let args = args.expect("args parse must succeed");
        assert_eq!(args.pathfind.expect("pathfind must exist"), "0,5");
    }

    #[test]
    fn cli_args_parse_analyze() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad", "--analyze"]);
        assert!(args.is_ok(), "args with --analyze must parse successfully");
        let args = args.expect("args parse must succeed");
        assert!(args.analyze);
    }
}
