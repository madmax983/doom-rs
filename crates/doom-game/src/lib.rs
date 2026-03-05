//! Doom game simulation: thinker loop, MOBJ slab arena, player, monsters,
//! combat, sector specials, and rollback snapshots.
//!
//! # Architecture
//! - `GameState` owns `MobjSlab` + `PlayerState` + deterministic RNG.
//! - `tick(TicCmd)` is the single entry point for one simulation step.
//! - `save_snapshot` / `restore_snapshot` provide deep-copy rollback.
//!
//! # Invariants (Verus-verifiable)
//! - `player.health ≤ MAX_HEALTH` at all times.
//! - `player.ammo[i] ≤ MAX_AMMO[i]` for all i.
//! - Dead actors (`health ≤ 0`) never transition to attack states (batch 2).

pub mod actions;
pub mod automap;
pub mod cheats;
pub mod combat;
pub mod dehacked;
pub mod intermission;
pub mod linedef_dispatch;
pub mod menu;
pub mod mobj;
pub mod mobjinfo;
pub mod movement;
pub mod phase;
pub mod pickups;
pub mod player;
pub mod projectile;
pub mod random;
pub mod savegame;
pub mod sight;
pub mod snapshot;
pub mod sound;
pub mod spawn;
pub mod specials;
pub mod state;
pub mod states;
pub mod switch;
pub mod tic;
pub mod trace;
pub mod weapon_fire;
pub mod weapons;

pub use actions::{
    ACTION_BRUIS_ATTACK, ACTION_BSPI_ATTACK, ACTION_CHASE, ACTION_CPOS_ATTACK, ACTION_CYBER_ATTACK,
    ACTION_FACE_TARGET, ACTION_FAT_ATTACK1, ACTION_FAT_ATTACK2, ACTION_FAT_ATTACK3,
    ACTION_HEAD_ATTACK, ACTION_LOOK, ACTION_NONE, ACTION_PAIN_ATTACK, ACTION_SKEL_MISSILE,
    ACTION_SKULL_ATTACK, ACTION_SPID_ATTACK, dispatch_action, p_move, p_new_chase_dir,
};
pub use automap::{
    AutomapCanvas, AutomapState, TestCanvas, ThingCategory, classify_thing, draw_automap_full,
    draw_grid, draw_line, draw_thing_marker, init_seen_lines, line_color, mark_lines_seen,
    mark_subsector_lines_seen, thing_marker_color, world_to_screen,
};
pub use cheats::{CheatBuffer, CheatCode, apply_cheat, cheat_message, check_cheats};
pub use combat::{MELEERANGE, MISSILERANGE, damage_mobj, p_line_attack, p_radius_attack};
pub use dehacked::{
    AmmoPatch, DehError, DehPatch, FramePatch, MiscPatch, TextReplacement, ThingPatch, WeaponPatch,
};
pub use intermission::{IntermissionStats, par_time};
pub use linedef_dispatch::{
    LinedefEffect, TriggerType, check_cross_lines, classify_trigger, dispatch_linedef,
    linedef_effect,
};
pub use menu::{GameMenu, MenuAction, MenuItem, MenuPage, MenuResult, TitlePhase, TitleScreen};
pub use mobj::{Mobj, MobjHandle, MobjKind, MobjSlab, StateNum, flags};
pub use mobjinfo::{MOBJINFO, MobjInfo};
pub use movement::{MAX_STEP_HEIGHT, p_slide_move, p_try_move};
pub use phase::{GamePhase, GamePhaseController, MapId};
pub use pickups::{doomed_type_to_kind, p_check_pickups, p_touch_special_thing};
pub use player::{AmmoType, PlayerState, WEAPON_AMMO, WeaponType};
pub use projectile::{
    ProjectileInfo, p_move_projectiles, p_spawn_missile, p_spawn_player_missile, projectile_info,
};
pub use random::{p_damage_with_variance, p_missile_angle_spread, p_random_chance, randomize_tics};
pub use savegame::{
    MAX_SAVE_SLOTS, SAVE_MAGIC, SaveError, SaveGame, SaveHeader, load_game, save_game,
    save_slot_filename,
};
pub use sight::{
    p_aim_line_slope, p_check_sight, p_look_for_players, point_on_side, ray_crosses_linedef,
    sector_from_subsector,
};
pub use snapshot::Snapshot;
pub use sound::{
    ML_SOUNDBLOCK, adjacent_sectors, clear_sound_targets, get_sound_target, init_sound_state,
    monster_should_wake, p_noise_alert,
};
pub use spawn::{Skill, spawn_level_things};
pub use specials::{
    StairType, USE_RANGE, activate_linedef, ev_build_stairs, ev_ceiling_crush_and_raise,
    ev_ceiling_crush_raise_fast, ev_ceiling_crush_stop, ev_ceiling_lower_and_crush,
    ev_ceiling_lower_to_floor, ev_do_donut, ev_do_lift, ev_perpetual_platform, ev_teleport,
    highest_adjacent_floor, init_conveyors, init_scrolling_walls, init_sector_lights,
    lowest_adjacent_ceiling, lowest_adjacent_floor, next_highest_floor, p_use_lines,
    player_sector_index, sector_linedefs, tick_ceilings, tick_conveyors, tick_floors, tick_lifts,
    tick_platforms, tick_scrollers, tick_sector_damage, tick_sector_lights, tick_sector_specials,
};
pub use state::{
    CeilingMover, CeilingType, ConveyorBelt, DoomRng, ExitRequest, FloorMover, GameState,
    LiftMover, LiftStatus, LightEffectType, MoveDirection, PerpetualPlatform, PlatformStatus,
    RNG_TABLE, ScrollingWall, SectorLightEffect,
};
pub use states::STATES;
pub use states::sprite_names;
pub use switch::{
    KeyType, SWITCH_PAIRS, clear_linedef_special, find_switch_opposite, player_has_key,
    toggle_switch_texture,
};
pub use tic::{
    FRICTION, MAXMOVE, PLAYER_SPEED_SCALE, TicCmd, bt, p_set_mobj_state, tick_all_mobjs,
    tick_player, tick_world,
};
pub use trace::{TraceHit, TraceResult, trace_ray};
pub use weapon_fire::{
    AMMO_PER_SHOT, fire_current_weapon, p_fire_bfg, p_fire_chaingun, p_fire_chainsaw, p_fire_fist,
    p_fire_pistol, p_fire_plasma, p_fire_rocket, p_fire_shotgun, p_fire_super_shotgun,
    select_next_weapon, weapon_ammo_cost,
};
pub use weapons::{fire_weapon, player_can_fire};
