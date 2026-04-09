#![allow(unused_imports)]
use super::*;
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

use crate::audio_system::{AudioSystem, music_lump_for_map, sound_request_sfx};
use doom_audio::{GenmidiBank, MidiPlayer, MusScore, SfxEmitter, SfxPriority, compute_spatial};

// ---------------------------------------------------------------------------

pub(crate) const DEAD_PLAYER_VIEW_HEIGHT: i32 = 6;

#[inline]
pub(crate) fn next_player_view_height(current: i32, player_dead: bool) -> i32 {
    if player_dead {
        current.saturating_sub(1).max(DEAD_PLAYER_VIEW_HEIGHT)
    } else {
        PLAYER_HEIGHT
    }
}

pub(crate) struct DoomGame {
    pub(crate) gs: GameState,
    pub(crate) level: Level,
    pub(crate) cheat_detector: cheats::CheatDetector,
    /// Chatchar-based cheat buffer (doom-game's ring-buffer cheat detector).
    /// Processes raw `chatchar` bytes from TicInput for classic Doom cheat entry.
    pub(crate) cheat_buffer: game_cheats::CheatBuffer,
    /// Timed HUD message queue: (pickup notifications, level names, etc.).
    /// Displayed at the top of the screen and ticks down each frame.
    pub(crate) hud_messages: doom_renderer::HudMessageQueue,
    pub(crate) console: console::Console,
    /// Path used for quick save (F5) and quick load (F9).
    pub(crate) save_path: std::path::PathBuf,
    /// Stateful automap with zoom/pan/follow support.
    pub(crate) automap: AutomapState,
    /// Whether the IDDT cheat has toggled full automap reveal.
    pub(crate) automap_full_reveal: bool,
    /// Optional audio subsystem.  `None` when no audio device is available.
    pub(crate) audio: Option<AudioSystem>,
    /// WAD music lumps keyed by their canonical `D_*` lump names.
    pub(crate) music_library: std::collections::HashMap<String, std::sync::Arc<[u8]>>,
    /// Flat texture cache (floor/ceiling textures loaded from the WAD).
    pub(crate) flat_cache: Option<FlatCache>,
    /// Wall texture cache (TEXTURE1/TEXTURE2 composed textures from the WAD).
    pub(crate) tex_cache: Option<TextureCache>,
    /// Sprite frame cache (loaded from S_START..S_END).
    pub(crate) sprite_cache: Option<SpriteCache>,
    /// Colormap cache (COLORMAP lump, 34 × 256 bytes for light-level shading).
    pub(crate) colormap_cache: Option<ColormapCache>,
    /// Compatibility profile selected at startup.
    pub(crate) compat: CompatibilityProfile,
    /// Animated texture state (flat + wall animation sequences, ticked per tic).
    pub(crate) anim_state: AnimState,
    /// Palette flash controller (pain/pickup/rad-suit full-screen tints).
    pub(crate) palette_flash: PaletteFlash,
    /// Switch texture pair lookup (SW1xxx <-> SW2xxx bidirectional).
    #[cfg(test)]
    pub(crate) switch_list: SwitchList,
    /// Player health from the previous tic — used to detect damage for pain flash.
    pub(crate) prev_health: i32,
    /// First-person view height above the floor, lowered while the player is dead.
    pub(crate) player_view_height: i32,
    /// In-game menu (Esc toggles it).
    pub(crate) menu: doom_game::menu::GameMenu,
    /// Bitmap font for menu/console text rendering.
    pub(crate) bitmap_font: BitmapFont,
    /// WAD patch cache for menu/HUD graphics.
    pub(crate) patch_cache: PatchCache,
    /// WAD-based HU font (STCFN patches). `None` until WAD is attached.
    pub(crate) wad_font: Option<WadFont>,
    /// WAD stack for patch lookups and stacked map loads.
    pub(crate) wad_stack: WadStack,
    /// Mugshot face animation FSM.
    pub(crate) face_state: FaceState,
    /// Title screen state. `Some` = still on title screen, `None` = in gameplay.
    pub(crate) title_screen: Option<TitleScreen>,
    /// Optional plain-text debug event log (opened with --debug-log).
    pub(crate) debug_log: Option<std::fs::File>,
    /// SFX ID for the player pain sound (DSPLPAIN), resolved at startup.
    pub(crate) pain_sfx_id: Option<u16>,
    /// Name → SFX ID lookup built from the WAD at startup (same ordering as
    /// `SfxCache`).  Used to play monster wake/attack/death sounds by lump name.
    pub(crate) sfx_lookup: std::collections::HashMap<String, u16>,
    /// Current skill used when spawning the next map.
    pub(crate) skill: Skill,
    /// Top-level playing/intermission/finale controller.
    pub(crate) phase_controller: GamePhaseController,
    /// Animated intermission tally renderer, active only during intermission.
    pub(crate) intermission_renderer: Option<IntermissionRenderer>,
    /// Previous tic's held attack/use mask for transition skip edge detection.
    pub(crate) transition_buttons_down: u8,
    /// First-person weapon bob/raise/flash controller.
    pub(crate) weapon_anim: WeaponAnimState,
    /// Cogmind-mode rendering state (tile grid + visibility cache).
    pub(crate) cogmind_state: cogmind::CogmindState,
}

impl DoomGame {
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        gs: GameState,
        level: Level,
        audio: Option<AudioSystem>,
        music_library: std::collections::HashMap<String, std::sync::Arc<[u8]>>,
        flat_cache: Option<FlatCache>,
        tex_cache: Option<TextureCache>,
        sprite_cache: Option<SpriteCache>,
        colormap_cache: Option<ColormapCache>,
        show_title: bool,
        debug_log: Option<std::fs::File>,
        pain_sfx_id: Option<u16>,
        sfx_lookup: std::collections::HashMap<String, u16>,
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
    pub(crate) fn new_with_compat(
        mut gs: GameState,
        level: Level,
        audio: Option<AudioSystem>,
        music_library: std::collections::HashMap<String, std::sync::Arc<[u8]>>,
        flat_cache: Option<FlatCache>,
        tex_cache: Option<TextureCache>,
        sprite_cache: Option<SpriteCache>,
        colormap_cache: Option<ColormapCache>,
        show_title: bool,
        debug_log: Option<std::fs::File>,
        pain_sfx_id: Option<u16>,
        sfx_lookup: std::collections::HashMap<String, u16>,
        compat: CompatibilityProfile,
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

    pub(crate) fn current_fixed_colormap(&self) -> Option<&[u8; 256]> {
        let is_invulnerable =
            self.gs.player.powers[doom_game::player::powers::PW_INVULNERABILITY] > 0;
        let cache = self.colormap_cache.as_ref()?;
        is_invulnerable.then_some(cache.invulnerability_row(self.compat))
    }

    fn start_music_lump(&self, lump_name: &str) -> bool {
        let Some(audio) = &self.audio else {
            return false;
        };
        let Some(music) = self.music_library.get(lump_name) else {
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

    pub(crate) fn attach_wad_for_transitions(&mut self, skill: Skill, wad_stack: WadStack) {
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

    pub(crate) fn tick_weapon_anim(&mut self) {
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
    pub(crate) fn handle_sound_events(
        &mut self,
        events: impl IntoIterator<Item = doom_game::SoundRequest>,
    ) {
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

            let emitter = match ev {
                SoundRequest::MonsterWake(_, _, x, y)
                | SoundRequest::MonsterAttack(_, _, x, y)
                | SoundRequest::MonsterDie(_, _, x, y) => Some((x, y)),
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
                | SoundRequest::MonsterDie(_, handle, _, _) => Some(handle),
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
    pub(crate) fn dlog_live_enemies(&mut self) {
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
                    match result {
                        doom_game::menu::MenuResult::StartGame { episode: _, skill } => {
                            // Map skill index to Skill enum (0=Baby..4=Nightmare).
                            let sk = Skill::from_num(skill).unwrap_or(Skill::Medium);
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
                            if let Err(e) = savegame::save_game(
                                std::path::Path::new(&path),
                                &self.gs,
                                slot,
                                self.compat,
                            ) {
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
            self.load_map_after_intermission(map_id, true);
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
        let (px, py, angle) = match self.gs.mobjslab.get(handle) {
            Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.angle),
            None => (0, 0, Bam::ZERO),
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
                render_actors_with_masked_and_fixed_colormap_ex(
                    &actors,
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
        use doom_game::{AmmoType, WEAPON_AMMO};

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
            doom_game::WeaponType::Fist => "FIST",
            doom_game::WeaponType::Pistol => "PIST",
            doom_game::WeaponType::Shotgun => "SG",
            doom_game::WeaponType::Chaingun => "CG",
            doom_game::WeaponType::RocketLauncher => "RL",
            doom_game::WeaponType::PlasmaRifle => "PLAS",
            doom_game::WeaponType::Bfg => "BFG",
            doom_game::WeaponType::Chainsaw => "SAW",
            doom_game::WeaponType::SuperShotgun => "SSG",
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
        })
    }
}
