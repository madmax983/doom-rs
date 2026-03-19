//! Doom engine entry point.
//!
//! Usage: doom-app --iwad doom1.wad [--pwad mod.wad] [--warp E1M1]

mod audio_system;
mod cheats;
mod console;
mod demo_mode;
mod net_mode;
mod savegame;

use anyhow::{Context, Result};
use clap::Parser;
use doom_demo::{DemoPlayer, DemoRecorder, LmpHeader};
use doom_game::FaceState;
use doom_game::LockedDoorColor;
use doom_game::cheats as game_cheats;
use doom_game::dehacked::DehPatch;
use doom_game::player::WeaponType;
use doom_game::{
    GamePhase, GamePhaseController, GameState, Skill, TicCmd, TitleScreen, init_conveyors,
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
    draw_title_screen_wad, draw_weapon_animated, render_actors_with_masked_ex,
    render_flag_from_state, render_level_with_view_height_and_extra_light, thing_sprite_prefix,
};
use doom_tui::{DoomApp, DoomEventLoop, TicInput};
use doom_types::{Bam, Fixed16_16};
use doom_wad::WadStack;

#[cfg(test)]
use doom_wad::WadFile;

#[cfg(test)]
use doom_renderer::SwitchList;

use audio_system::{AudioSystem, music_lump_for_map, sound_request_sfx};
use doom_audio::{SfxEmitter, SfxPriority, compute_spatial};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "doom-app", about = "Doom engine (doom-rs)")]
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
    /// WAD music lumps keyed by their canonical `D_*` lump names.
    music_library: std::collections::HashMap<String, Vec<u8>>,
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
    sfx_lookup: std::collections::HashMap<String, u16>,
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

impl DoomGame {
    #[allow(clippy::too_many_arguments)]
    fn new(
        mut gs: GameState,
        level: Level,
        audio: Option<AudioSystem>,
        music_library: std::collections::HashMap<String, Vec<u8>>,
        flat_cache: Option<FlatCache>,
        tex_cache: Option<TextureCache>,
        sprite_cache: Option<SpriteCache>,
        colormap_cache: Option<ColormapCache>,
        show_title: bool,
        debug_log: Option<std::fs::File>,
        pain_sfx_id: Option<u16>,
        sfx_lookup: std::collections::HashMap<String, u16>,
    ) -> Self {
        // Initialize scrolling wall and conveyor belt specials from level linedefs.
        init_scrolling_walls(&mut gs, &level);
        init_conveyors(&mut gs, &level);

        // Initialize dynamic sector lighting (blinking, strobe, fireflicker).
        init_sector_lights(&mut gs, &level);

        // Capture initial player health for pain-flash delta detection.
        let initial_health = gs.player.health();

        let mut menu = doom_game::menu::GameMenu::new(false); // false = Doom 1 mode
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
            cheat_message: None,
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
        };

        game.reset_weapon_anim();

        if game.title_screen.is_none() {
            game.start_level_music();
        }

        game
    }

    fn start_music_lump(&self, lump_name: &str) -> bool {
        let Some(audio) = &self.audio else {
            return false;
        };
        let Some(music) = self.music_library.get(lump_name) else {
            return false;
        };
        audio.start_music(music.clone());
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
        self.menu = doom_game::menu::GameMenu::new(false);
        self.menu.open();
        self.intermission_renderer = None;
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

    fn load_map_after_intermission(&mut self, map_id: doom_game::MapId, carry_player_state: bool) {
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

        let carried_player = carry_player_state.then(|| self.gs.player.clone());
        let mut gs = GameState::new(&map_name);
        let player_handle = spawn_level_things(&mut gs, &level, self.skill, false);

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
        self.sync_weapon_anim_from_player_psprites(false);
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

    fn sync_weapon_anim_from_player_psprites(&mut self, preserve_motion: bool) {
        use doom_game::player::psprite_slots;

        let weapon_psprite = self.gs.player.psprites[psprite_slots::WEAPON];
        let flash_psprite = self.gs.player.psprites[psprite_slots::FLASH];
        let previous_offset = preserve_motion.then_some(self.weapon_anim.raise_offset);

        self.weapon_anim.current.sx = weapon_psprite.sx;
        self.weapon_anim.current.sprite_name =
            psprite_patch_name(weapon_psprite.state).unwrap_or(*b"PISGA0\0\0");
        self.weapon_anim.current.flash_active = flash_psprite.state != doom_game::StateNum::NULL;
        self.weapon_anim.current.flash_sprite =
            psprite_patch_name(flash_psprite.state).unwrap_or([0; 8]);
        self.weapon_anim.current.flash_tics = flash_psprite.tics.max(0) as u32;
        self.weapon_anim.current.full_bright = self.weapon_anim.current.flash_active
            || psprite_state_is_fullbright(weapon_psprite.state);
        self.weapon_anim.raise_offset = weapon_psprite.sy;

        let (state_raising, state_lowering) =
            psprite_transition_flags(self.gs.player.weapon, weapon_psprite.state);

        if state_raising || state_lowering {
            self.weapon_anim.current.raising = state_raising;
            self.weapon_anim.current.lowering = state_lowering;
        } else if let Some(previous_offset) = previous_offset {
            self.weapon_anim.current.raising = weapon_psprite.sy < previous_offset;
            self.weapon_anim.current.lowering = weapon_psprite.sy > previous_offset;
        } else {
            self.weapon_anim.current.raising = false;
            self.weapon_anim.current.lowering = false;
        }
    }

    fn tick_weapon_anim(&mut self) {
        self.ensure_player_psprites_initialized();
        if self.gs.player.is_dead() {
            self.weapon_anim.bob.reset();
            self.sync_weapon_anim_from_player_psprites(false);
            return;
        }

        self.weapon_anim.bob.tick(self.player_weapon_anim_speed());
        self.sync_weapon_anim_from_player_psprites(true);
    }

    fn transition_input_pressed(&mut self, input: &TicInput) -> bool {
        let button_mask = input.buttons & (doom_game::bt::BT_ATTACK | doom_game::bt::BT_USE);
        let button_pressed = button_mask != 0 && self.transition_buttons_down == 0;
        self.transition_buttons_down = button_mask;
        input.menu_select || input.escape_pressed || button_pressed
    }
}

impl DoomGame {
    fn handle_sound_events(&mut self, events: impl IntoIterator<Item = doom_game::SoundRequest>) {
        use doom_game::SoundRequest;

        let events_vec: Vec<_> = events.into_iter().collect();

        for ev in &events_vec {
            if let SoundRequest::PlayerUseLockedDoor(color) = ev {
                self.cheat_message = Some((locked_door_message(*color).to_string(), 105));
            }
        }

        let Some(ref audio) = self.audio else {
            return;
        };

        let (pl_x, pl_y, pl_angle) = self
            .gs
            .mobjslab
            .get(self.gs.player.handle)
            .map(|mo| (mo.x, mo.y, mo.angle))
            .unwrap_or_default();
        let player_origin = Some(self.gs.player.handle);

        for ev in &events_vec {
            let Some((lump, priority)) = sound_request_sfx(*ev) else {
                continue;
            };

            let emitter = match ev {
                SoundRequest::MonsterWake(_, _, x, y)
                | SoundRequest::MonsterAttack(_, _, x, y)
                | SoundRequest::MonsterDie(_, _, x, y) => Some((*x, *y)),
                SoundRequest::PlayerWeaponFire(_)
                | SoundRequest::PlayerSuperShotgunOpen
                | SoundRequest::PlayerSuperShotgunLoad
                | SoundRequest::PlayerSuperShotgunClose => Some((pl_x, pl_y)),
                SoundRequest::PlayerDie
                | SoundRequest::PlayerUseFail
                | SoundRequest::PlayerUseLockedDoor(_) => None,
            };
            let origin = match ev {
                SoundRequest::MonsterWake(_, handle, _, _)
                | SoundRequest::MonsterAttack(_, handle, _, _)
                | SoundRequest::MonsterDie(_, handle, _, _) => Some(*handle),
                SoundRequest::PlayerWeaponFire(_)
                | SoundRequest::PlayerSuperShotgunOpen
                | SoundRequest::PlayerSuperShotgunLoad
                | SoundRequest::PlayerSuperShotgunClose => player_origin,
                SoundRequest::PlayerDie
                | SoundRequest::PlayerUseFail
                | SoundRequest::PlayerUseLockedDoor(_) => None,
            };

            if lump.is_empty() {
                continue;
            }

            if let Some(&id) = self.sfx_lookup.get(lump) {
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
        use doom_game::player::{AmmoType, WEAPON_AMMO};
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

        let msgs: Vec<String> = self
            .gs
            .mobjslab
            .iter_handles()
            .filter_map(|h| {
                let mo = self.gs.mobjslab.get(h)?;
                if mo.flags & doom_game::mobj::flags::MF_COUNTKILL == 0 {
                    return None;
                }
                let ex = mo.x.to_int();
                let ey = mo.y.to_int();
                let state_idx = mo.state.0;
                let flags = mo.flags;
                let is_dead = mo.health <= 0;
                let target = mo.target;
                Some(format!(
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
                ))
            })
            .collect();

        for msg in msgs {
            self.dlog(&msg);
        }
    }

    /// Log death events — enemies that fired A_Scream this tic (SCREAMED flag set).
    /// Clears the flag after logging so each death is logged exactly once.
    fn dlog_death_events(&mut self) {
        if self.debug_log.is_none() {
            return;
        }

        // Use a vector because we modify mobjs in the loop.
        let mut handles = Vec::with_capacity(self.gs.mobjslab.len());
        handles.extend(self.gs.mobjslab.iter_handles());
        for h in handles {
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
                let msg = format!(
                    "enemy_died {:?} pos=({},{}) death_state={}",
                    kind, x, y, state_idx
                );
                self.dlog(&msg);
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
                    match result {
                        doom_game::menu::MenuResult::StartGame { episode: _, skill } => {
                            // Map skill index to Skill enum (0=Baby..4=Nightmare).
                            let sk = match skill {
                                0 => Skill::Baby,
                                1 => Skill::Easy,
                                3 => Skill::Hard,
                                4 => Skill::Nightmare,
                                _ => Skill::Medium,
                            };
                            // Re-spawn the level with the chosen skill.
                            self.gs = GameState::new(&self.gs.level_name.clone());
                            spawn_level_things(&mut self.gs, &self.level, sk, false);
                            init_scrolling_walls(&mut self.gs, &self.level);
                            init_conveyors(&mut self.gs, &self.level);
                            init_sector_lights(&mut self.gs, &self.level);
                            self.player_view_height = PLAYER_HEIGHT;
                            self.prev_health = self.gs.player.health();
                            self.skill = sk;
                            self.phase_controller
                                .start_new_game(Self::map_id_from_level_name(
                                    self.gs.level_name.as_str(),
                                ));
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
                        _ => {}
                    }
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
                    self.load_map_after_intermission(map_id, true);
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
                    match result {
                        doom_game::menu::MenuResult::Quit => {
                            // Signal quit; can't reach event loop directly, so
                            // we just close the menu — user can press Q to exit.
                            self.menu.close();
                        }
                        doom_game::menu::MenuResult::LoadGame(slot) => {
                            let path = format!("doom_save_{slot}.bin");
                            match savegame::load_game(std::path::Path::new(&path)) {
                                Ok((_header, payload)) => {
                                    if let Err(e) = savegame::apply_save(&mut self.gs, &payload) {
                                        self.console.print(format!("Load failed: {e}"));
                                    } else {
                                        self.player_view_height = if self.gs.player.is_dead() {
                                            DEAD_PLAYER_VIEW_HEIGHT
                                        } else {
                                            PLAYER_HEIGHT
                                        };
                                        self.console.print("Game loaded.".to_string());
                                        self.start_level_music();
                                        self.menu.close();
                                    }
                                }
                                Err(e) => self.console.print(format!("Load failed: {e}")),
                            }
                        }
                        doom_game::menu::MenuResult::SaveGame(slot) => {
                            let path = format!("doom_save_{slot}.bin");
                            if let Err(e) =
                                savegame::save_game(std::path::Path::new(&path), &self.gs, slot)
                            {
                                self.console.print(format!("Save failed: {e}"));
                            } else {
                                self.console.print(format!("Saved to slot {slot}."));
                                self.menu.close();
                            }
                        }
                        _ => {}
                    }
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
                        self.reset_weapon_anim();
                        self.console.print("Game loaded.".to_string());
                        self.start_level_music();
                    }
                }
                Err(e) => {
                    self.console.print(format!("Load failed: {e}"));
                }
            }
        }

        let cmd = ticinput_to_ticcmd(input);

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
            self.load_map_after_intermission(map_id, true);
        }

        // Drain the game's sound event queue.  Each event maps to a DS* lump
        // name and a priority.  The SfxMixer's 8-channel priority system handles
        // contention — weapon-priority sounds always win; monster sounds compete
        // with each other, matching Doom's original S_StartSound behaviour.
        {
            let events: Vec<_> = self.gs.sound_queue.drain(..).collect();
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
            let RenderOut {
                z_buf,
                clip_top,
                clip_bot,
                clip_top_depth,
                clip_bot_depth,
                masked_columns,
            } = render_level_with_view_height_and_extra_light(
                &self.level,
                px,
                py,
                angle,
                self.player_view_height,
                fb,
                &palette,
                self.flat_cache.as_ref(),
                self.tex_cache.as_ref(),
                self.colormap_cache.as_ref(),
                None,
                false,
                self.gs.player.extra_light,
            );

            // Project live mobj positions as state-driven billboard sprites.
            // Uses ActorRenderInfo so animations play correctly.
            if let Some(ref cache) = self.sprite_cache {
                let player_handle = self.gs.player.handle;
                let actors: Vec<ActorRenderInfo> = self
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
                    })
                    .collect();
                render_actors_with_masked_ex(
                    &actors,
                    &self.level,
                    Fixed16_16::from_int(px),
                    Fixed16_16::from_int(py),
                    angle,
                    fb,
                    cache,
                    Some(&z_buf),
                    self.colormap_cache.as_ref(),
                    Some(SpriteClip {
                        top: &clip_top,
                        bottom: &clip_bot,
                        top_depth: &clip_top_depth,
                        bottom_depth: &clip_bot_depth,
                    }),
                    Some(&masked_columns),
                );
            }

            // Draw weapon sprite overlay using the player's current weapon.
            if let Some(ref cache) = self.sprite_cache
                && !self.gs.player.is_dead()
            {
                draw_weapon_animated(fb, &self.weapon_anim, cache, &IDENTITY_COLORMAP);
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
        if let Some((ref msg, _)) = self.cheat_message {
            draw_cheat_message_overlay(fb, msg);
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

    fn on_frame_timings(&mut self, tick_us: u64, render_us: u64, blit_us: u64) {
        if self.debug_log.is_none() {
            return;
        }
        let total_us = tick_us + render_us + blit_us;
        let msg = format!(
            "frame tick={tick_us}us render={render_us}us blit={blit_us}us total={total_us}us"
        );
        self.dlog(&msg);
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
    let msgs: Vec<&str> = console
        .messages
        .iter()
        .rev()
        .take(8)
        .map(|s| s.as_str())
        .collect();
    for (i, msg) in msgs.iter().enumerate() {
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

// `spawn_player` removed — replaced by `spawn_level_things` which spawns
// ALL map things (player, monsters, items, decorations, keys).

fn load_music_library(wad: &WadStack) -> std::collections::HashMap<String, Vec<u8>> {
    let mut music_library = std::collections::HashMap::new();

    for episode in 1..=4 {
        for map in 1..=9 {
            let map_name = format!("E{episode}M{map}");
            let Some(music_lump) = music_lump_for_map(&map_name) else {
                continue;
            };
            if let Some(mus_data) = wad.lump_data(&music_lump) {
                music_library.insert(music_lump, mus_data.to_vec());
            }
        }
    }

    for map in 1..=32 {
        let map_name = format!("MAP{map:02}");
        let Some(music_lump) = music_lump_for_map(&map_name) else {
            continue;
        };
        if let Some(mus_data) = wad.lump_data(&music_lump) {
            music_library.insert(music_lump, mus_data.to_vec());
        }
    }

    for lump in ["D_INTER", "D_DM2INT"] {
        if let Some(mus_data) = wad.lump_data(lump) {
            music_library.insert(lump.to_string(), mus_data.to_vec());
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

fn main() -> Result<()> {
    // Initialize trig tables (required for sin/cos in the game simulation).
    // SAFETY: called exactly once at startup, single-threaded, before any
    // Bam::sin() or Bam::cos() calls.
    unsafe {
        doom_types::Bam::init_trig_tables();
    }

    let args = Args::parse();

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

    // Create game state and spawn ALL level things (player, monsters, items, keys).
    let mut gs = GameState::new(warp_str);
    let skill = match args.skill {
        1 => Skill::Baby,
        2 => Skill::Easy,
        4 => Skill::Hard,
        5 => Skill::Nightmare,
        _ => Skill::Medium, // default: 3 = Hurt Me Plenty
    };
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
        eprintln!("DeHackEd: applied {count} modification(s) from {deh_path}");
    }

    // Load flat texture cache (floor/ceiling textures between F_START and F_END).
    let flat_cache = FlatCache::load_from_stack(&wad_stack);
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
        let cache = SpriteCache::load_from_stack(&wad_stack);
        if cache.is_empty() { None } else { Some(cache) }
    };

    // Load colormap cache (COLORMAP lump: 34 × 256 bytes, light-level shading).
    let colormap_cache = Some(ColormapCache::load(&wad_stack));

    // Try to open the audio subsystem.  Returns None in headless/CI environments.
    let audio = AudioSystem::try_open(&wad_stack);

    let music_library = load_music_library(&wad_stack);

    if let Err(message) = validate_mode_args(&args) {
        eprintln!("Error: {message}");
        std::process::exit(1);
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
                eprintln!("Warning: could not open debug log '{}': {e}", p.display());
                None
            }
        });

    // Build a name→ID map for all DS* lumps so monster sounds can be resolved
    // by lump name at play time without additional WAD scans.
    let sfx_lookup = audio_system::build_sfx_lookup(&wad_stack);

    // Resolve player pain SFX (DSPLPAIN) once at startup so we can fire it cheaply.
    let pain_sfx_id = sfx_lookup.get("DSPLPAIN").copied();

    let mut app = DoomGame::new(
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
    );
    app.attach_wad_for_transitions(skill, wad_stack);

    // Headless capture mode: tick N frames, render, save BMP, exit.
    if let Some(ref capture_path) = args.capture {
        let capture = if let Some(ref demo_path) = args.playdemo {
            let player = load_demo_player(demo_path)?;
            let mut playback_app = demo_mode::DemoPlaybackApp::new(app, player);
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
        eprintln!(
            "Captured frame {} to {}",
            args.capture_frames,
            capture_path.display()
        );
        return Ok(());
    }

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
    let mut event_loop =
        DoomEventLoop::new().map_err(|e| anyhow::anyhow!("Failed to initialize terminal: {e}"))?;

    // Enable graphics protocol (Sixel/Kitty/iTerm2) if the terminal supports it.
    // Our custom palette-aware Sixel encoder makes this fast even at 35 Hz.
    event_loop.set_graphics_protocol(true);

    if let Some(demo_path) = args.playdemo {
        // Load and parse the demo file.
        let player = load_demo_player(&demo_path)?;
        let mut playback_app = demo_mode::DemoPlaybackApp::new(app, player);
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
    use doom_game::{GameState, Mobj, MobjKind, PlayerState, flags};
    use doom_map::{Blockmap, Level, Reject, Sector};

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
    use doom_types::{Bam, Fixed16_16};
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
    ) -> std::collections::HashMap<String, Vec<u8>> {
        let mut library = std::collections::HashMap::new();
        if let Some(lump) = music_lump_for_map(level_name) {
            library.insert(lump, data);
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
        assert!(
            !game.automap_full_reveal,
            "automap_full_reveal must start false"
        );
        assert!(
            game.cheat_message.is_none(),
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
            game.cheat_message.is_some(),
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
            game.cheat_message.as_ref().map(|(msg, _)| msg.as_str()),
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

        assert_eq!(library.get("D_INTER"), Some(&vec![1, 2, 3, 4]));
        assert_eq!(library.get("D_DM2INT"), Some(&vec![5, 6, 7, 8]));
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
        music_library.insert("D_INTER".to_string(), vec![9, 10, 11, 12]);
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
}
