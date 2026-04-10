#![allow(unused_imports)]
//! Doom engine entry point.
//!
//! Usage: doom-app --iwad doom1.wad [--pwad mod.wad] [--warp E1M1]

mod app;
mod audio_system;
mod cheats;
mod cogmind;
mod console;
mod demo_mode;
mod net_mode;
mod savegame;
pub(crate) use app::DoomGame;

use anyhow::{Context, Result};
use clap::Parser;
use doom_demo::{DemoPlayer, DemoRecorder, LmpHeader};
use doom_game::FaceState;
use doom_game::LockedDoorColor;
use doom_game::cheats as game_cheats;
use doom_game::dehacked::DehPatch;
use doom_game::player::WeaponType;
use doom_game::{
    GamePhase, GamePhaseController, GameState, Skill, TitleScreen, init_conveyors,
    init_scrolling_walls, init_sector_lights, kind_to_doomed_type, spawn_level_things,
};
use doom_game::{MOBJINFO, STATES};
use doom_map::Level;
use doom_renderer::IDENTITY_COLORMAP;
use doom_renderer::{
    ActorRenderInfo, AnimState, AutomapState, BitmapFont, ColormapCache, FlatCache, Framebuffer,
    IntermissionRenderer, PLAYER_HEIGHT, PaletteFlash, PaletteLut, PatchCache, RenderOut,
    SpriteCache, SpriteClip, TextureCache, WadFont, WeaponAnimState, draw_automap_ex,
    draw_finale_wad, draw_intermission, draw_intermission_wad, draw_menu_wad, draw_status_bar_wad,
    draw_title_screen_wad, draw_weapon_animated_with_override,
    render_actors_with_masked_and_fixed_colormap_ex, render_flag_from_state,
    render_level_with_view_height_and_extra_light_and_fixed_colormap, thing_sprite_prefix,
};
use doom_tui::{DoomApp, DoomEventLoop, RendererMode, TicInput};
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
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Green.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Yellow.on_default() | Effects::BOLD)
}

fn parse_compatibility_profile(value: &str) -> Result<CompatibilityProfile, &'static str> {
    value.parse()
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

    /// Print the map statistics as raw JSON. Only valid when combined with --map-stats.
    #[arg(long)]
    json: bool,
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
    let prompt = format!("> {}_", console.input);
    let prompt_y = PANEL_H.saturating_sub(CHAR_H + 2);
    draw_mini_string(fb, prompt_y, &prompt, COLOR_PROMPT);
}

/// Draw a string using the mini 4x6 glyph font at `(2, y)`.
///
/// Characters that overflow the 320-pixel width are clipped.
fn draw_mini_string(fb: &mut Framebuffer, y: usize, text: &str, color: u8) {
    const FB_W: usize = 320;
    const CHAR_W: usize = 5;
    const GLYPH_ROWS: usize = 6;

    for (ci, ch) in text.chars().enumerate() {
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

fn psprite_transition_flags(weapon: WeaponType, state: doom_game::StateNum) -> (bool, bool) {
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

    (
        state == doom_game::StateNum(up),
        state == doom_game::StateNum(down),
    )
}

// ---------------------------------------------------------------------------
// Input conversion
fn load_music_library(wad: &WadStack) -> std::collections::HashMap<String, std::sync::Arc<[u8]>> {
    let mut music_library = std::collections::HashMap::new();

    for episode in 1..=4 {
        for map in 1..=9 {
            let map_name = format!("E{episode}M{map}");
            let Some(music_lump) = music_lump_for_map(&map_name) else {
                continue;
            };
            if let Some(mus_data) = wad.lump_data(&music_lump) {
                music_library.insert(music_lump, std::sync::Arc::<[u8]>::from(mus_data));
            }
        }
    }

    for map in 1..=32 {
        let map_name = format!("MAP{map:02}");
        let Some(music_lump) = music_lump_for_map(&map_name) else {
            continue;
        };
        if let Some(mus_data) = wad.lump_data(&music_lump) {
            music_library.insert(music_lump, std::sync::Arc::<[u8]>::from(mus_data));
        }
    }

    for lump in ["D_INTER", "D_DM2INT"] {
        if let Some(mus_data) = wad.lump_data(lump) {
            music_library.insert(lump.to_string(), std::sync::Arc::<[u8]>::from(mus_data));
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
    std::fs::write(out_path, wav_bytes)
        .with_context(|| format!("Failed to write WAV to {}", out_path.display()))?;
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
    std::fs::write(out_path, wav_bytes)
        .with_context(|| format!("Failed to write WAV to {}", out_path.display()))?;
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

fn run_doom() -> Result<()> {
    // Initialize trig tables (required for sin/cos in the game simulation).
    // SAFETY: called exactly once at startup, single-threaded, before any
    // Bam::sin() or Bam::cos() calls.
    unsafe {
        doom_types::Bam::init_trig_tables();
    }

    let args = Args::parse();
    let compat = args.compat;

    let iwad_bytes = std::fs::read(&args.iwad)
        .with_context(|| format!("Failed to read IWAD file: {}", args.iwad.display()))?;

    // Build a WadStack for patch/lump lookups (menu graphics, HUD sprites).
    let mut wad_stack = WadStack::new();
    wad_stack
        .push_iwad(iwad_bytes)
        .with_context(|| format!("Failed to load IWAD: {}", args.iwad.display()))?;

    for pwad_path in &args.pwad {
        let pwad_bytes = std::fs::read(pwad_path)
            .with_context(|| format!("Failed to read PWAD file: {}", pwad_path.display()))?;
        wad_stack
            .push_pwad(pwad_bytes)
            .with_context(|| format!("Failed to load PWAD: {}", pwad_path.display()))?;
    }

    // Build the PLAYPAL blit palette (for terminal RGB conversion).
    let blit_palette = match wad_stack.lump_data("PLAYPAL") {
        Some(data) => PaletteLut::from_playpal(data).unwrap_or_else(|_| PaletteLut::grayscale()),
        None => PaletteLut::grayscale(),
    };

    // Determine which map to load — default to the first canonical map present.
    let warp_name = args
        .warp
        .clone()
        .unwrap_or_else(|| default_warp_map(&wad_stack));
    let warp_str = warp_name.as_str();
    let show_title = args.warp.is_none();

    // Parse the requested level.
    let level = Level::from_wad_stack(&wad_stack, warp_str)
        .with_context(|| format!("Failed to load map {warp_str}"))?;

    if let Some(ref html_path) = args.export_html {
        let html_data = doom_map::export_map_to_html(&level);
        std::fs::write(html_path, html_data)
            .with_context(|| format!("Failed to write HTML to {}", html_path.display()))?;
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} HTML report to {}",
                "🌟".green(),
                "Exported".green().bold(),
                html_path.display().to_string().cyan()
            );
        } else {
            println!("Exported HTML report to {}", html_path.display());
        }
        return Ok(());
    }

    if let Some(ref json_path) = args.export_json {
        let json_data = doom_map::export_map_to_json(&level);
        std::fs::write(json_path, json_data)
            .with_context(|| format!("Failed to write JSON to {}", json_path.display()))?;
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} JSON report to {}",
                "🌟".green(),
                "Exported".green().bold(),
                json_path.display().to_string().cyan()
            );
        } else {
            println!("Exported JSON report to {}", json_path.display());
        }
        return Ok(());
    }

    if let Some(ref svg_path) = args.export_svg {
        let svg_data = doom_map::export_map_to_svg(&level);
        std::fs::write(svg_path, svg_data)
            .with_context(|| format!("Failed to write SVG to {}", svg_path.display()))?;
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} layout to {}",
                "🌟".green(),
                "Exported".green().bold(),
                svg_path.display().to_string().cyan()
            );
        } else {
            println!("Exported layout to {}", svg_path.display());
        }
        return Ok(());
    }

    if let Some(ref geojson_path) = args.export_geojson {
        let geojson_data = doom_map::export_map_to_geojson(&level);
        std::fs::write(geojson_path, geojson_data)
            .with_context(|| format!("Failed to write GeoJSON to {}", geojson_path.display()))?;
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} GeoJSON to {}",
                "🌟".green(),
                "Exported".green().bold(),
                geojson_path.display().to_string().cyan()
            );
        } else {
            println!("Exported GeoJSON to {}", geojson_path.display());
        }
        return Ok(());
    }

    if let Some(ref dot_path) = args.export_dot {
        let graph = doom_map::SectorGraph::build(&level);
        std::fs::write(dot_path, graph.to_dot())
            .with_context(|| format!("Failed to write DOT to {}", dot_path.display()))?;
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} Graphviz DOT to {}",
                "🌟".green(),
                "Exported".green().bold(),
                dot_path.display().to_string().cyan()
            );
        } else {
            println!("Exported Graphviz DOT to {}", dot_path.display());
        }
        return Ok(());
    }

    if let Some(ref obj_path) = args.export_obj {
        let obj_data = doom_map::obj::export_map_to_obj(&level);
        std::fs::write(obj_path, obj_data)
            .with_context(|| format!("Failed to write OBJ to {}", obj_path.display()))?;
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            println!(
                "{} {} 3D model to {}",
                "🌟".green(),
                "Exported".green().bold(),
                obj_path.display().to_string().cyan()
            );
        } else {
            println!("Exported 3D model to {}", obj_path.display());
        }
        return Ok(());
    }

    if let Some(ref wav_path) = args.export_music_wav {
        export_music_wav_for_map(&wad_stack, warp_str, args.music_loops, wav_path)?;
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
        return Ok(());
    }

    if let Some(ref sfx_wav_path) = args.export_sfx_wav {
        let sfx_name = args
            .sfx_name
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("--sfx-name is required when using --export-sfx-wav"))?;
        export_sfx_wav_for_name(&wad_stack, sfx_name, sfx_wav_path)?;
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
            let chokepoints_json = format!(
                "[{}]",
                chokepoints
                    .iter()
                    .enumerate()
                    .fold(String::new(), |mut acc, (i, s)| {
                        if i > 0 {
                            acc.push_str(", ");
                        }
                        acc.push_str(&s.to_string());
                        acc
                    })
            );
            let areas_json = format!(
                "[{}]",
                areas
                    .iter()
                    .enumerate()
                    .fold(String::new(), |mut acc_outer, (i, a)| {
                        if i > 0 {
                            acc_outer.push_str(", ");
                        }
                        acc_outer.push('[');
                        a.iter().enumerate().fold(&mut acc_outer, |acc, (j, s)| {
                            if j > 0 {
                                acc.push_str(", ");
                            }
                            acc.push_str(&s.to_string());
                            acc
                        });
                        acc_outer.push(']');
                        acc_outer
                    })
            );

            let json_data = format!(
                r#"{{
  "map": "{}",
  "chokepoints": {},
  "isolated_areas": {}
}}"#,
                warp_str, chokepoints_json, areas_json
            );
            println!("{}", json_data);
        } else {
            use crossterm::style::Stylize;
            if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
                println!(
                    "{} {} tactical analysis for {}",
                    "🌟".green(),
                    "Completed".green().bold(),
                    warp_str.cyan()
                );
            } else {
                println!("Completed tactical analysis for {}", warp_str);
            }

            let chokepoints_str = if chokepoints.is_empty() {
                "None".to_string()
            } else {
                chokepoints
                    .iter()
                    .enumerate()
                    .fold(String::new(), |mut acc, (i, s)| {
                        if i > 0 {
                            acc.push_str(", ");
                        }
                        acc.push_str(&s.to_string());
                        acc
                    })
            };

            let areas_str = format!(
                "{} area{}",
                areas.len(),
                if areas.len() == 1 { "" } else { "s" }
            );

            let mut table = comfy_table::Table::new();
            if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
                table
                    .load_preset(comfy_table::presets::UTF8_FULL)
                    .apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS);
                table.set_header(vec![
                    comfy_table::Cell::new("Feature")
                        .fg(comfy_table::Color::Cyan)
                        .add_attribute(comfy_table::Attribute::Bold),
                    comfy_table::Cell::new("Data")
                        .fg(comfy_table::Color::Cyan)
                        .add_attribute(comfy_table::Attribute::Bold),
                ]);
                table.add_row(vec![
                    comfy_table::Cell::new("Chokepoints"),
                    comfy_table::Cell::new(&chokepoints_str).fg(comfy_table::Color::Red),
                ]);
                table.add_row(vec![
                    comfy_table::Cell::new("Isolated Areas"),
                    comfy_table::Cell::new(&areas_str).fg(comfy_table::Color::Magenta),
                ]);
            } else {
                table.set_header(vec![
                    comfy_table::Cell::new("Feature"),
                    comfy_table::Cell::new("Data"),
                ]);
                table.add_row(vec![
                    comfy_table::Cell::new("Chokepoints"),
                    comfy_table::Cell::new(&chokepoints_str),
                ]);
                table.add_row(vec![
                    comfy_table::Cell::new("Isolated Areas"),
                    comfy_table::Cell::new(&areas_str),
                ]);
            }
            println!("{table}");
        }
        return Ok(());
    }

    if let Some(path_str) = &args.pathfind {
        let parts: Vec<&str> = path_str.split(',').collect();
        if parts.len() == 2 {
            if let (Ok(start), Ok(end)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                let graph = doom_map::SectorGraph::build(&level);
                if let Some(path) = graph.shortest_path(start, end) {
                    println!("Path found: {:?}", path);
                } else {
                    println!("No path found between sector {} and sector {}", start, end);
                }
            } else {
                println!(
                    "Invalid sector indices. Please provide two integers separated by a comma."
                );
            }
        } else {
            println!("Invalid format. Please use START,END (e.g. 0,5).");
        }
        return Ok(());
    }

    if args.map_stats {
        let mut gs = GameState::new(warp_str);
        doom_game::spawn_level_things(&mut gs, &level, Skill::Medium, false);
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
            use std::io::IsTerminal;
            if std::io::stdout().is_terminal() {
                use crossterm::style::Stylize;
                let formatted_json = format!(
                    r#"{{
  {}: "{}",
  {}: {},
  {}: {},
  {}: {},
  {}: {}
}}"#,
                    r#""map""#.cyan().bold(),
                    warp_str.yellow(),
                    r#""total_kills""#.cyan().bold(),
                    stats.total_kills.to_string().red(),
                    r#""total_items""#.cyan().bold(),
                    stats.total_items.to_string().green(),
                    r#""total_secrets""#.cyan().bold(),
                    stats.total_secrets.to_string().magenta(),
                    r#""par_time_tics""#.cyan().bold(),
                    stats.par_time_tics.to_string().blue()
                );
                println!("{formatted_json}");
            } else {
                println!("{json_data}");
            }
        } else {
            let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
            let mut table = comfy_table::Table::new();
            table
                .load_preset(comfy_table::presets::UTF8_FULL)
                .apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS);

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
                            .fg(comfy_table::Color::Red),
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
                        comfy_table::Cell::new("⏱️  Par Time (tics)"),
                        comfy_table::Cell::new(stats.par_time_tics.to_string())
                            .fg(comfy_table::Color::Blue),
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
                        comfy_table::Cell::new("Par Time (tics)"),
                        comfy_table::Cell::new(stats.par_time_tics.to_string()),
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
    spawn_level_things(&mut gs, &level, skill, false);

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
    let pain_sfx_id = sfx_lookup.get("DSPLPAIN").copied();

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
        let client = doom_net::NetClient::connect(addr_str, 0)
            .map_err(|e| anyhow::anyhow!("Failed to connect to server {addr_str}: {e}"))?;
        let mut net_app = net_mode::NetGameApp::new(app, client);

        let mut event_loop = DoomEventLoop::new()
            .map_err(|e| anyhow::anyhow!("Failed to initialize terminal: {e}"))?;
        event_loop.set_turn_based_mode(args.turn_based);
        event_loop
            .run(&mut net_app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Event loop error: {e}"))?;
        return Ok(());
    }

    // Start the terminal event loop and run until the user quits (Q or Esc).
    let mut event_loop =
        DoomEventLoop::new().map_err(|e| anyhow::anyhow!("Failed to initialize terminal: {e}"))?;
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
            .map_err(|e| anyhow::anyhow!("Event loop error: {e}"))?;
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
            .map_err(|e| anyhow::anyhow!("Event loop error: {e}"))?;
    } else {
        let mut app = app;
        event_loop
            .run(&mut app, &blit_palette)
            .map_err(|e| anyhow::anyhow!("Event loop error: {e}"))?;
    }

    Ok(())
}

fn main() {
    if let Err(err) = run_doom() {
        use crossterm::style::Stylize;
        if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            eprintln!("\n❌ {}: {}", "Fatal Error".red().bold(), err);

            let mut causes = err.chain().skip(1).peekable();
            if causes.peek().is_some() {
                eprintln!("\n↳ {}:", "Caused by".yellow().bold());
                for cause in causes {
                    eprintln!("    {}", cause);
                }
            }
            eprintln!();
        } else {
            eprintln!("Fatal Error: {}", err);

            let mut causes = err.chain().skip(1).peekable();
            if causes.peek().is_some() {
                eprintln!("Caused by:");
                for cause in causes {
                    eprintln!("    {}", cause);
                }
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
    ) -> std::collections::HashMap<String, std::sync::Arc<[u8]>> {
        let mut library = std::collections::HashMap::new();
        if let Some(lump) = music_lump_for_map(level_name) {
            library.insert(lump, std::sync::Arc::<[u8]>::from(data));
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

        game.tick(TicInput {
            buttons: doom_game::bt::BT_ATTACK,
            ..TicInput::default()
        });

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
            buttons: doom_game::bt::BT_CHANGE | (2u8 << 3),
            ..TicInput::default()
        });

        assert_eq!(
            game.gs.player.pending_weapon,
            Some(WeaponType::Shotgun),
            "weapon switch should stage the new weapon in the gameplay psprite state"
        );
        assert!(
            game.weapon_anim.current.lowering,
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
            game.player_view_height,
            app::DEAD_PLAYER_VIEW_HEIGHT,
            "dead player view height must clamp to the low vanilla death view"
        );
    }

    #[test]
    fn alive_player_view_height_resets_after_revival() {
        let mut game = make_doom_game();
        game.player_view_height = app::DEAD_PLAYER_VIEW_HEIGHT;

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
            library.get("D_INTER"),
            Some(&std::sync::Arc::<[u8]>::from(vec![1, 2, 3, 4]))
        );
        assert_eq!(
            library.get("D_DM2INT"),
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
            .unwrap()
            .momx = Fixed16_16::from_int(40);

        game.tick(TicInput {
            buttons: doom_game::bt::BT_ATTACK,
            ..TicInput::default()
        });
        game.tick(TicInput {
            buttons: doom_game::bt::BT_ATTACK,
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
            "D_INTER".to_string(),
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
            .unwrap()
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
        let args = args.unwrap();
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
            "--wad",
            "doom1.wad",
            "--connect",
            "127.0.0.1:5029",
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
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(args.deh.is_none(), "--deh must default to None");
    }

    // -----------------------------------------------------------------------
    // Test 30: CLI args --server defaults to None
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_server_defaults_to_none() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(args.server.is_none(), "--server must default to None");
    }

    // -----------------------------------------------------------------------
    // Test 31: CLI args --connect defaults to None
    // -----------------------------------------------------------------------

    #[test]
    fn cli_args_connect_defaults_to_none() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        assert!(args.connect.is_none(), "--connect must default to None");
    }

    #[test]
    fn cli_args_compat_defaults_to_extended() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad"]);
        assert!(args.is_ok());
        let args = args.unwrap();
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
        let args = args.unwrap();
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
        let args = args.unwrap();
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

        let temp_dir = tempfile::tempdir().unwrap();
        let out_path = temp_dir.path().join("pistol.wav");

        let result = export_sfx_wav_for_name(&stack, "DSPISTOL", &out_path);
        assert!(result.is_ok(), "SFX export should succeed");

        let wav_data = std::fs::read(&out_path).unwrap();
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
                doom_game::player::WeaponType::Pistol,
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
        let args = args.unwrap();
        assert_eq!(args.pathfind.unwrap(), "0,5");
    }

    #[test]
    fn cli_args_parse_analyze() {
        let args = Args::try_parse_from(["doom-app", "--wad", "doom1.wad", "--analyze"]);
        assert!(args.is_ok(), "args with --analyze must parse successfully");
        let args = args.unwrap();
        assert!(args.analyze);
    }
}
