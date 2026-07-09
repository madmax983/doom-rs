//! Weapon firing — maps WeaponType to attack parameters and fires.
//!
//! Port of Doom's `p_pspr.c` weapon state machine.
//!
//! Each weapon dispatches to either `p_line_attack` (hitscan) or
//! `p_radius_attack` (splash) from `combat.rs`.

use doom_map::Level;
use doom_types::{Bam, Fixed16_16};

use crate::combat::{MISSILERANGE, p_line_attack};
use crate::mobj::{MobjHandle, StateNum};
use crate::player::{PlayerState, PspriteState, psprite_slots};
use crate::projectile::p_spawn_player_missile;
use crate::state::{GameState, SoundRequest};
use crate::states::{STATES, ids, sprite_names};
use doom_types::mobj_kind::MobjKind;
use doom_types::weapons::AmmoType;
use doom_types::weapons::WeaponType;
use doom_types::{TicCmd, bt};

// ---------------------------------------------------------------------------
// Weapon stat table
// ---------------------------------------------------------------------------

/// Per-weapon static firing parameters.
#[allow(dead_code)]
struct WeaponInfo {
    /// Ammo type consumed, or `None` for melee weapons (Fist, Chainsaw).
    ammo_type: Option<AmmoType>,
    /// Units of ammo consumed per firing event.
    ammo_use: u32,
    /// Minimum damage roll (inclusive).
    damage_lo: i32,
    /// Maximum damage roll (inclusive).
    damage_hi: i32,
    /// Number of hitscan rays per shot (0 = radius attack, e.g. rocket).
    pellets: u8,
    /// Maximum hitscan range in fixed-point map units.
    range: Fixed16_16,
    /// Angular spread between pellets in BAM units (0 = no spread).
    spread: u32,
    /// True if weapon is melee (Fist, Chainsaw).
    is_melee: bool,
}

/// Weapon info table indexed by `WeaponType as usize`.
///
/// Entries correspond to: Fist(0), Pistol(1), Shotgun(2), Chaingun(3),
/// RocketLauncher(4), PlasmaRifle(5), Bfg(6), Chainsaw(7), SuperShotgun(8).
static WEAPON_INFO: [WeaponInfo; 9] = [
    // 0 — Fist
    WeaponInfo {
        ammo_type: None,
        ammo_use: 0,
        damage_lo: 10,
        damage_hi: 110,
        pellets: 1,
        range: Fixed16_16(64 << 16),
        spread: 0,
        is_melee: true,
    },
    // 1 — Pistol
    WeaponInfo {
        ammo_type: Some(AmmoType::Bullets),
        ammo_use: 1,
        damage_lo: 5,
        damage_hi: 15,
        pellets: 1,
        range: MISSILERANGE,
        spread: 0,
        is_melee: false,
    },
    // 2 — Shotgun
    WeaponInfo {
        ammo_type: Some(AmmoType::Shells),
        ammo_use: 1,
        damage_lo: 5,
        damage_hi: 15,
        pellets: 7,
        range: MISSILERANGE,
        spread: 0x1400_0000, // ≈ 5.6° per pellet in BAM
        is_melee: false,
    },
    // 3 — Chaingun
    WeaponInfo {
        ammo_type: Some(AmmoType::Bullets),
        ammo_use: 1,
        damage_lo: 5,
        damage_hi: 15,
        pellets: 1,
        range: MISSILERANGE,
        spread: 0,
        is_melee: false,
    },
    // 4 — RocketLauncher (pellets=0 → radius attack)
    WeaponInfo {
        ammo_type: Some(AmmoType::Rockets),
        ammo_use: 1,
        damage_lo: 80,
        damage_hi: 160,
        pellets: 0,
        range: Fixed16_16::ZERO, // unused — radius attack
        spread: 0,
        is_melee: false,
    },
    // 5 — PlasmaRifle
    WeaponInfo {
        ammo_type: Some(AmmoType::Cells),
        ammo_use: 1,
        damage_lo: 5,
        damage_hi: 40,
        pellets: 1,
        range: MISSILERANGE,
        spread: 0,
        is_melee: false,
    },
    // 6 — BFG 9000
    WeaponInfo {
        ammo_type: Some(AmmoType::Cells),
        ammo_use: 40,
        damage_lo: 100,
        damage_hi: 800,
        pellets: 1,
        range: MISSILERANGE,
        spread: 0,
        is_melee: false,
    },
    // 7 — Chainsaw
    WeaponInfo {
        ammo_type: None,
        ammo_use: 0,
        damage_lo: 10,
        damage_hi: 110,
        pellets: 1,
        range: Fixed16_16(64 << 16),
        spread: 0,
        is_melee: true,
    },
    // 8 — SuperShotgun
    WeaponInfo {
        ammo_type: Some(AmmoType::Shells),
        ammo_use: 2,
        damage_lo: 5,
        damage_hi: 15,
        pellets: 20,
        range: MISSILERANGE,
        spread: 0x1400_0000, // same spread as regular shotgun
        is_melee: false,
    },
];

/// Normal fully-raised psprite Y offset.
pub const WEAPON_TOP: i32 = 0;
/// Fully-lowered psprite Y offset.
pub const WEAPON_BOTTOM: i32 = 128;
const RAISE_SPEED: i32 = 6;
const LOWER_SPEED: i32 = 6;

#[derive(Clone, Copy)]
struct WeaponPspriteInfo {
    up: StateNum,
    down: StateNum,
    ready: StateNum,
    attack: StateNum,
}

fn weapon_psprite_sprite(weapon: WeaponType) -> u16 {
    match weapon {
        WeaponType::Fist => sprite_names::SPR_PUNG,
        WeaponType::Pistol => sprite_names::SPR_PISG,
        WeaponType::Shotgun => sprite_names::SPR_SHTG,
        WeaponType::Chaingun => sprite_names::SPR_CHGG,
        WeaponType::RocketLauncher => sprite_names::SPR_ROCK,
        WeaponType::PlasmaRifle => sprite_names::SPR_PLSG,
        WeaponType::Bfg => sprite_names::SPR_BFGG,
        WeaponType::Chainsaw => sprite_names::SPR_SAWG,
        WeaponType::SuperShotgun => sprite_names::SPR_SHT2,
    }
}

fn weapon_psprite_info(weapon: WeaponType) -> WeaponPspriteInfo {
    match weapon {
        WeaponType::Fist => WeaponPspriteInfo {
            up: StateNum(ids::S_PUNCH_UP),
            down: StateNum(ids::S_PUNCH_DOWN),
            ready: StateNum(ids::S_PUNCH_READY),
            attack: StateNum(ids::S_PUNCH1),
        },
        WeaponType::Pistol => WeaponPspriteInfo {
            up: StateNum(ids::S_PISTOL_UP),
            down: StateNum(ids::S_PISTOL_DOWN),
            ready: StateNum(ids::S_PISTOL_READY),
            attack: StateNum(ids::S_PISTOL1),
        },
        WeaponType::Shotgun => WeaponPspriteInfo {
            up: StateNum(ids::S_SGUN_UP),
            down: StateNum(ids::S_SGUN_DOWN),
            ready: StateNum(ids::S_SGUN_READY),
            attack: StateNum(ids::S_SGUN1),
        },
        WeaponType::Chaingun => WeaponPspriteInfo {
            up: StateNum(ids::S_CHAIN_UP),
            down: StateNum(ids::S_CHAIN_DOWN),
            ready: StateNum(ids::S_CHAIN_READY),
            attack: StateNum(ids::S_CHAIN1),
        },
        WeaponType::RocketLauncher => WeaponPspriteInfo {
            up: StateNum(ids::S_MISSILE_UP),
            down: StateNum(ids::S_MISSILE_DOWN),
            ready: StateNum(ids::S_MISSILE_READY),
            attack: StateNum(ids::S_MISSILE1),
        },
        WeaponType::PlasmaRifle => WeaponPspriteInfo {
            up: StateNum(ids::S_PLASMA_UP),
            down: StateNum(ids::S_PLASMA_DOWN),
            ready: StateNum(ids::S_PLASMA_READY),
            attack: StateNum(ids::S_PLASMA1),
        },
        WeaponType::Bfg => WeaponPspriteInfo {
            up: StateNum(ids::S_BFG_UP),
            down: StateNum(ids::S_BFG_DOWN),
            ready: StateNum(ids::S_BFG_READY),
            attack: StateNum(ids::S_BFG1),
        },
        WeaponType::Chainsaw => WeaponPspriteInfo {
            up: StateNum(ids::S_SAW_UP),
            down: StateNum(ids::S_SAW_DOWN),
            ready: StateNum(ids::S_SAW_READY1),
            attack: StateNum(ids::S_SAW1),
        },
        WeaponType::SuperShotgun => WeaponPspriteInfo {
            up: StateNum(ids::S_DSGUN_UP),
            down: StateNum(ids::S_DSGUN_DOWN),
            ready: StateNum(ids::S_DSGUN_READY),
            attack: StateNum(ids::S_DSGUN1),
        },
    }
}

fn init_psprite_state(player: &mut PlayerState, slot: usize, state: StateNum) {
    let psprite = &mut player.psprites[slot];
    psprite.state = state;
    if state == StateNum::NULL {
        psprite.tics = 0;
        return;
    }

    let Some(entry) = STATES.get(state.0 as usize) else {
        psprite.state = StateNum::NULL;
        psprite.tics = 0;
        return;
    };

    psprite.tics = i32::from(entry.tics);
}

fn set_psprite_state(
    gs: &mut GameState,
    slot: usize,
    mut state: StateNum,
    cmd: TicCmd,
    level: Option<&Level>,
) {
    loop {
        if state == StateNum::NULL {
            gs.player.psprites[slot].state = StateNum::NULL;
            gs.player.psprites[slot].tics = 0;
            return;
        }

        let Some(entry) = STATES.get(state.0 as usize) else {
            gs.player.psprites[slot].state = StateNum::NULL;
            gs.player.psprites[slot].tics = 0;
            return;
        };

        gs.player.psprites[slot].state = state;
        gs.player.psprites[slot].tics = i32::from(entry.tics);

        if entry.action != crate::actions::Action::NoAction as u8 {
            dispatch_psprite_action(gs, entry.action, cmd, level);
            if gs.player.psprites[slot].state != state {
                return;
            }
        }

        if gs.player.psprites[slot].tics != 0 {
            return;
        }

        state = entry.next_state;
    }
}

fn bring_up_weapon(player: &mut PlayerState) {
    let weapon = player.pending_weapon.take().unwrap_or(player.weapon);
    let info = weapon_psprite_info(weapon);
    player.weapon = weapon;
    player.psprites[psprite_slots::WEAPON].sx = 0;
    player.psprites[psprite_slots::WEAPON].sy = WEAPON_BOTTOM;
    init_psprite_state(player, psprite_slots::WEAPON, info.up);
}

fn begin_lower_weapon(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    let info = weapon_psprite_info(gs.player.weapon);
    set_psprite_state(gs, psprite_slots::WEAPON, info.down, cmd, level);
}

fn check_ammo(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) -> bool {
    if player_can_fire(gs) {
        return true;
    }
    gs.player.refire = 0;
    gs.player.pending_weapon = crate::weapon_fire::select_next_weapon(gs);
    begin_lower_weapon(gs, cmd, level);
    false
}

fn queue_weapon_sound_and_noise(gs: &mut GameState, weapon: WeaponType, level: Option<&Level>) {
    gs.sound
        .sound_queue
        .push(SoundRequest::PlayerWeaponFire(weapon));
    if !matches!(weapon, WeaponType::Fist)
        && let Some(lv) = level
    {
        let handle = gs.player.handle;
        crate::sound::p_noise_alert(gs, lv, handle, handle);
    }
}

fn set_player_mobj_state(gs: &mut GameState, state: StateNum) {
    let tics = STATES.get(state.0 as usize).map_or(0, |entry| entry.tics);
    if let Some(player_mobj) = gs.mobjslab.get_mut(gs.player.handle) {
        player_mobj.state = state;
        player_mobj.tics = tics;
    }
}

fn ensure_player_mobj_state(gs: &mut GameState) {
    let needs_init = gs
        .mobjslab
        .get(gs.player.handle)
        .map(|mo| mo.state == StateNum::NULL)
        .unwrap_or(false);
    if needs_init {
        set_player_mobj_state(gs, StateNum(ids::S_PLAY));
    }
}

fn restore_player_ready_state(gs: &mut GameState) {
    gs.player.extra_light = 0;
    let state = gs.mobjslab.get(gs.player.handle).map(|mo| mo.state);
    if matches!(
        state,
        Some(StateNum(ids::S_PLAY_ATK1) | StateNum(ids::S_PLAY_ATK2))
    ) {
        set_player_mobj_state(gs, StateNum(ids::S_PLAY));
    }
}

fn begin_player_weapon_attack(gs: &mut GameState) {
    ensure_player_mobj_state(gs);
    set_player_mobj_state(gs, StateNum(ids::S_PLAY_ATK1));
}

fn start_weapon_flash(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    let weapon_state = gs.player.psprites[psprite_slots::WEAPON].state;
    let flash_state = match gs.player.weapon {
        WeaponType::Fist | WeaponType::Chainsaw => StateNum::NULL,
        WeaponType::Pistol => StateNum(ids::S_PISTOL_FLASH1),
        WeaponType::Shotgun => StateNum(ids::S_SGUN_FLASH1),
        WeaponType::SuperShotgun => StateNum(ids::S_DSGUN_FLASH1),
        WeaponType::Chaingun => {
            if weapon_state == StateNum(ids::S_CHAIN2) {
                StateNum(ids::S_CHAIN_FLASH2)
            } else {
                StateNum(ids::S_CHAIN_FLASH1)
            }
        }
        WeaponType::RocketLauncher => StateNum(ids::S_MISSILE_FLASH1),
        WeaponType::PlasmaRifle => {
            if gs.p_random() & 1 == 0 {
                StateNum(ids::S_PLASMA_FLASH1)
            } else {
                StateNum(ids::S_PLASMA_FLASH2)
            }
        }
        WeaponType::Bfg => StateNum(ids::S_BFG_FLASH1),
    };

    if flash_state == StateNum::NULL {
        return;
    }

    set_player_mobj_state(gs, StateNum(ids::S_PLAY_ATK2));
    let weapon_psprite = gs.player.psprites[psprite_slots::WEAPON];
    gs.player.psprites[psprite_slots::FLASH].sx = weapon_psprite.sx;
    gs.player.psprites[psprite_slots::FLASH].sy = WEAPON_TOP;
    set_psprite_state(gs, psprite_slots::FLASH, flash_state, cmd, level);
}

fn apply_pending_weapon_change(gs: &mut GameState, cmd: TicCmd) {
    if cmd.buttons & bt::BT_CHANGE == 0 {
        return;
    }
    let weapon_num = ((cmd.buttons & bt::BT_WEAPONMASK) >> 3) as usize;
    if let Some(weapon) = WeaponType::from_num(weapon_num)
        && gs.player.has_weapon(weapon)
        && weapon != gs.player.weapon
    {
        gs.player.pending_weapon = Some(weapon);
    }
}

fn a_weapon_ready(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    ensure_player_mobj_state(gs);
    restore_player_ready_state(gs);

    if gs.player.pending_weapon.is_some() || !player_can_fire(gs) {
        begin_lower_weapon(gs, cmd, level);
        return;
    }

    let attack_held = cmd.buttons & bt::BT_ATTACK != 0;
    if !attack_held {
        gs.player.refire = 0;
        return;
    }

    let may_fire =
        !gs.player.attack_down || crate::weapon_fire::weapon_allows_hold_fire(gs.player.weapon);
    if !may_fire {
        return;
    }

    gs.player.refire = if gs.player.attack_down {
        gs.player.refire.saturating_add(1)
    } else {
        0
    };
    begin_player_weapon_attack(gs);
    let info = weapon_psprite_info(gs.player.weapon);
    set_psprite_state(gs, psprite_slots::WEAPON, info.attack, cmd, level);
}

fn a_lower(gs: &mut GameState) {
    let weapon = &mut gs.player.psprites[psprite_slots::WEAPON];
    weapon.sy = (weapon.sy + LOWER_SPEED).min(WEAPON_BOTTOM);
    if weapon.sy == WEAPON_BOTTOM {
        bring_up_weapon(&mut gs.player);
    }
}

fn a_raise(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    let weapon = &mut gs.player.psprites[psprite_slots::WEAPON];
    weapon.sy = (weapon.sy - RAISE_SPEED).max(WEAPON_TOP);
    if weapon.sy == WEAPON_TOP {
        let ready = weapon_psprite_info(gs.player.weapon).ready;
        set_psprite_state(gs, psprite_slots::WEAPON, ready, cmd, level);
    }
}

fn a_gun_flash(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    start_weapon_flash(gs, cmd, level);
}

fn a_refire(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    let attack_held = cmd.buttons & bt::BT_ATTACK != 0;
    if attack_held && gs.player.pending_weapon.is_none() && !gs.player.is_dead() {
        gs.player.refire = gs.player.refire.saturating_add(1);
        begin_player_weapon_attack(gs);
        let info = weapon_psprite_info(gs.player.weapon);
        set_psprite_state(gs, psprite_slots::WEAPON, info.attack, cmd, level);
        return;
    }

    gs.player.refire = 0;
    let _ = check_ammo(gs, cmd, level);
}

fn a_check_reload(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    let _ = check_ammo(gs, cmd, level);
}

fn a_open_shotgun2(gs: &mut GameState) {
    gs.sound
        .sound_queue
        .push(SoundRequest::PlayerSuperShotgunOpen);
}

fn a_load_shotgun2(gs: &mut GameState) {
    gs.sound
        .sound_queue
        .push(SoundRequest::PlayerSuperShotgunLoad);
}

fn a_close_shotgun2(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    gs.sound
        .sound_queue
        .push(SoundRequest::PlayerSuperShotgunClose);
    a_refire(gs, cmd, level);
}

fn a_light0(gs: &mut GameState) {
    gs.player.extra_light = 0;
}

fn a_light1(gs: &mut GameState) {
    gs.player.extra_light = 1;
}

fn a_light2(gs: &mut GameState) {
    gs.player.extra_light = 2;
}

fn a_punch(gs: &mut GameState, _cmd: TicCmd, level: Option<&Level>) {
    crate::weapon_fire::p_fire_fist(gs, level);
    queue_weapon_sound_and_noise(gs, WeaponType::Fist, level);
}

fn a_fire_pistol(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    if !check_ammo(gs, cmd, level) {
        return;
    }
    crate::weapon_fire::p_fire_pistol(gs, level);
    queue_weapon_sound_and_noise(gs, WeaponType::Pistol, level);
    start_weapon_flash(gs, cmd, level);
}

fn a_fire_shotgun(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    if !check_ammo(gs, cmd, level) {
        return;
    }
    crate::weapon_fire::p_fire_shotgun(gs, level);
    queue_weapon_sound_and_noise(gs, WeaponType::Shotgun, level);
    start_weapon_flash(gs, cmd, level);
}

fn a_fire_shotgun2(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    if !check_ammo(gs, cmd, level) {
        return;
    }
    crate::weapon_fire::p_fire_super_shotgun(gs, level);
    queue_weapon_sound_and_noise(gs, WeaponType::SuperShotgun, level);
    start_weapon_flash(gs, cmd, level);
}

fn a_fire_cgun(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    if !check_ammo(gs, cmd, level) {
        return;
    }
    crate::weapon_fire::p_fire_chaingun(gs, level);
    queue_weapon_sound_and_noise(gs, WeaponType::Chaingun, level);
    start_weapon_flash(gs, cmd, level);
}

fn a_fire_missile(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    if !check_ammo(gs, cmd, level) {
        return;
    }
    crate::weapon_fire::p_fire_rocket(gs, level);
    queue_weapon_sound_and_noise(gs, WeaponType::RocketLauncher, level);
    let _ = cmd;
}

fn a_fire_plasma(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    if !check_ammo(gs, cmd, level) {
        return;
    }
    crate::weapon_fire::p_fire_plasma(gs, level);
    queue_weapon_sound_and_noise(gs, WeaponType::PlasmaRifle, level);
    start_weapon_flash(gs, cmd, level);
}

fn a_bfg_sound(gs: &mut GameState, _cmd: TicCmd, level: Option<&Level>) {
    queue_weapon_sound_and_noise(gs, WeaponType::Bfg, level);
}

fn a_fire_bfg(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    if !check_ammo(gs, cmd, level) {
        return;
    }
    crate::weapon_fire::p_fire_bfg(gs, level);
    start_weapon_flash(gs, cmd, level);
}

fn a_saw(gs: &mut GameState, _cmd: TicCmd, level: Option<&Level>) {
    crate::weapon_fire::p_fire_chainsaw(gs, level);
    queue_weapon_sound_and_noise(gs, WeaponType::Chainsaw, level);
}

fn dispatch_psprite_action(gs: &mut GameState, action: u8, cmd: TicCmd, level: Option<&Level>) {
    if let Some(a) = crate::actions::Action::from_repr(action) {
        match a {
            crate::actions::Action::WeaponReady => a_weapon_ready(gs, cmd, level),
            crate::actions::Action::Lower => a_lower(gs),
            crate::actions::Action::Raise => a_raise(gs, cmd, level),
            crate::actions::Action::GunFlash => a_gun_flash(gs, cmd, level),
            crate::actions::Action::Punch => a_punch(gs, cmd, level),
            crate::actions::Action::FirePistol => a_fire_pistol(gs, cmd, level),
            crate::actions::Action::FireShotgun => a_fire_shotgun(gs, cmd, level),
            crate::actions::Action::FireShotgun2 => a_fire_shotgun2(gs, cmd, level),
            crate::actions::Action::FireCgun => a_fire_cgun(gs, cmd, level),
            crate::actions::Action::FireMissile => a_fire_missile(gs, cmd, level),
            crate::actions::Action::FirePlasma => a_fire_plasma(gs, cmd, level),
            crate::actions::Action::BfgSound => a_bfg_sound(gs, cmd, level),
            crate::actions::Action::FireBfg => a_fire_bfg(gs, cmd, level),
            crate::actions::Action::Saw => a_saw(gs, cmd, level),
            crate::actions::Action::Refire => a_refire(gs, cmd, level),
            crate::actions::Action::CheckReload => a_check_reload(gs, cmd, level),
            crate::actions::Action::OpenShotgun2 => a_open_shotgun2(gs),
            crate::actions::Action::LoadShotgun2 => a_load_shotgun2(gs),
            crate::actions::Action::CloseShotgun2 => a_close_shotgun2(gs, cmd, level),
            crate::actions::Action::Light0 => a_light0(gs),
            crate::actions::Action::Light1 => a_light1(gs),
            crate::actions::Action::Light2 => a_light2(gs),
            _ => {}
        }
    }
}

fn tick_psprite_slot(gs: &mut GameState, slot: usize, cmd: TicCmd, level: Option<&Level>) {
    let state = gs.player.psprites[slot].state;
    if state == StateNum::NULL {
        return;
    }

    let tics = gs.player.psprites[slot].tics;
    if tics == -1 {
        return;
    }
    if tics > 0 {
        gs.player.psprites[slot].tics -= 1;
    }
    if gs.player.psprites[slot].tics != 0 {
        return;
    }

    let next_state = STATES
        .get(state.0 as usize)
        .map(|entry| entry.next_state)
        .unwrap_or(StateNum::NULL);
    set_psprite_state(gs, slot, next_state, cmd, level);
}

/// Initialize player psprites for a freshly-spawned or restored player.
pub fn setup_psprites(player: &mut PlayerState) {
    player.refire = 0;
    player.extra_light = 0;
    player.psprites = [PspriteState::default(); doom_types::limits::NUM_PSPRITES];
    player.pending_weapon = Some(player.weapon);
    bring_up_weapon(player);
    init_psprite_state(player, psprite_slots::FLASH, StateNum::NULL);
}

/// Tick the player's weapon and flash psprites for one game tic.
pub fn tick_psprites(gs: &mut GameState, cmd: TicCmd, level: Option<&Level>) {
    if !gs.player.is_dead() {
        ensure_player_mobj_state(gs);
    }
    if gs.player.psprites[psprite_slots::WEAPON].state == StateNum::NULL && !gs.player.is_dead() {
        setup_psprites(&mut gs.player);
    }
    if let Some(state) = STATES.get(gs.player.psprites[psprite_slots::WEAPON].state.0 as usize)
        && state.sprite != weapon_psprite_sprite(gs.player.weapon)
        && !gs.player.is_dead()
    {
        setup_psprites(&mut gs.player);
    }
    apply_pending_weapon_change(gs, cmd);
    tick_psprite_slot(gs, psprite_slots::FLASH, cmd, level);
    tick_psprite_slot(gs, psprite_slots::WEAPON, cmd, level);
    gs.player.attack_down = cmd.buttons & bt::BT_ATTACK != 0;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fire the player's current weapon once.
///
/// Called when `bt::BT_ATTACK` is set in a `TicCmd` and the player is alive.
///
/// # Ammo check
/// If the current weapon requires ammo and the player has insufficient ammo,
/// this function returns immediately without firing or consuming ammo.
///
/// # Projectile weapons
/// Rocket Launcher, Plasma Rifle, and BFG 9000 spawn projectile actors via
/// `p_spawn_player_missile`.  Projectile collision is handled by
/// `p_move_projectiles` during the tick loop.
///
/// # Hitscan weapons
/// Fires `pellets` separate rays via `p_line_attack`.  Each pellet's angle is
/// spread evenly around the actor's facing direction.
pub fn fire_weapon(gs: &mut GameState, level: Option<&Level>, handle: MobjHandle) {
    use doom_types::weapons::WeaponType;

    let weapon = gs.player.weapon;
    let info = &WEAPON_INFO[weapon as usize];

    // --- Ammo check ---
    if let Some(ammo_type) = info.ammo_type {
        let available = gs.player.ammo(ammo_type as usize);
        if available < info.ammo_use {
            return;
        }
        // We already verified available >= ammo_use above; this cannot fail.
        gs.player.use_ammo(ammo_type as usize, info.ammo_use);
    }

    // --- Projectile weapons: spawn a missile actor ---
    match weapon {
        WeaponType::RocketLauncher => {
            p_spawn_player_missile(gs, handle, MobjKind::Rocket);
            return;
        }
        WeaponType::PlasmaRifle => {
            p_spawn_player_missile(gs, handle, MobjKind::PlasmaBall);
            return;
        }
        WeaponType::Bfg => {
            p_spawn_player_missile(gs, handle, MobjKind::BfgBall);
            return;
        }
        _ => {} // hitscan weapons fall through
    }

    // --- Gather source angle (copy out before mutable borrows) ---
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let base_angle: Bam = mo.angle;

    let pellets = info.pellets;
    let spread = info.spread;
    let range = info.range;

    // Deterministic pseudo-random damage using tic_num.
    let tic = gs.tic_num;
    let damage_range = (info.damage_hi - info.damage_lo + 1) as u32;
    let damage_lo = info.damage_lo;

    // --- Hitscan: fire each pellet ---
    let mut intercepts = smallvec::SmallVec::new();
    for i in 0..pellets {
        // Compute per-pellet angle.
        // For single-pellet weapons spread=0, so this is just base_angle.
        // For multi-pellet (shotgun): center the spread around base_angle.
        //   offset = spread * i - spread * (pellets - 1) / 2
        let shot_angle: Bam = if pellets == 1 || spread == 0 {
            base_angle
        } else {
            // Center the spread: pellet i fires at base_angle + spread*(i) - spread*(pellets-1)/2
            let positive_offset = spread.wrapping_mul(i as u32);
            let center_offset = spread.wrapping_mul((pellets as u32).wrapping_sub(1)) / 2;
            Bam(base_angle
                .0
                .wrapping_add(positive_offset)
                .wrapping_sub(center_offset))
        };

        // Deterministic damage: vary by pellet index and tic_num.
        let damage = damage_lo + ((tic.wrapping_add(i as u32)) % damage_range) as i32;

        p_line_attack(
            gs,
            handle,
            shot_angle,
            range,
            damage,
            level,
            &mut intercepts,
        );
    }
}

/// Returns `true` if the player's current weapon has sufficient ammo to fire.
///
/// Always returns `true` for melee weapons (Fist, Chainsaw) regardless of
/// ammo counts.
pub fn player_can_fire(gs: &GameState) -> bool {
    let weapon = gs.player.weapon;
    let info = &WEAPON_INFO[weapon as usize];
    match info.ammo_type {
        None => true,
        Some(ammo_type) => gs.player.ammo(ammo_type as usize) >= info.ammo_use,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, flags};
    use crate::player::{PlayerState, psprite_slots};
    use crate::state::GameState;
    use crate::states::ids;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::weapons::WeaponType;
    use doom_types::{Bam, Fixed16_16};
    use doom_types::{TicCmd, bt};

    /// Build a minimal GameState with a live player Mobj at the origin.
    ///
    /// Mirrors the pattern used in `actions.rs` tests.
    fn make_game_state() -> GameState {
        let mut gs = GameState::new("test");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs
    }

    fn cmd_with_buttons(buttons: u8) -> TicCmd {
        TicCmd {
            buttons,
            ..Default::default()
        }
    }

    fn ready_player_psprites(gs: &mut GameState) {
        setup_psprites(&mut gs.player);
        for _ in 0..24 {
            tick_psprites(gs, TicCmd::default(), None);
        }
    }

    fn count_mobjs_of_kind(gs: &GameState, kind: MobjKind) -> usize {
        gs.mobjslab
            .iter_handles()
            .filter_map(|handle| gs.mobjslab.get(handle))
            .filter(|mo| mo.kind == kind)
            .count()
    }

    // -----------------------------------------------------------------------
    // Test 1: pistol consumes one bullet per shot
    // -----------------------------------------------------------------------

    #[test]
    fn pistol_consumes_one_clip_ammo() {
        let mut gs = make_game_state();
        // pistol_start gives 50 bullets; weapon defaults to Pistol.
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(
            after,
            before - 1,
            "pistol must consume exactly 1 bullet per shot"
        );
    }

    // -----------------------------------------------------------------------
    // Test 2: fist never consumes any ammo
    // -----------------------------------------------------------------------

    #[test]
    fn fist_never_consumes_ammo() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Fist;
        let before_bullets = gs.player.ammo(AmmoType::Bullets as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after_bullets = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(
            after_bullets, before_bullets,
            "fist must not consume any ammo"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: player_can_fire returns false when clip is empty
    // -----------------------------------------------------------------------

    #[test]
    fn player_can_fire_returns_false_when_no_ammo() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Pistol;
        // Drain all bullets by firing until empty, or use use_ammo for the known count.
        // pistol_start gives 50 bullets; drain exactly 50.
        let drained = gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        assert!(drained, "pre-condition: must drain 50 bullets successfully");
        assert_eq!(
            gs.player.ammo(AmmoType::Bullets as usize),
            0,
            "pre-condition: bullets must be 0"
        );
        assert!(
            !player_can_fire(&gs),
            "player_can_fire must return false with 0 bullets and Pistol"
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: player_can_fire always returns true for fist
    // -----------------------------------------------------------------------

    #[test]
    fn player_can_fire_returns_true_for_fist_always() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Fist;
        // Drain the bullets the pistol-start gives (50); ignore pools that are already 0.
        let _ = gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        assert!(
            player_can_fire(&gs),
            "fist must always be fireable regardless of ammo"
        );
    }

    // -----------------------------------------------------------------------
    // Test 5: shotgun consumes one shell per shot
    // -----------------------------------------------------------------------

    #[test]
    fn shotgun_consumes_one_shell() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Shotgun;
        // Give the player some shells.
        gs.player.give_ammo(AmmoType::Shells as usize, 10);
        let before = gs.player.ammo(AmmoType::Shells as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Shells as usize);
        assert_eq!(
            after,
            before - 1,
            "shotgun must consume exactly 1 shell per shot"
        );
    }

    // -----------------------------------------------------------------------
    // Test 6: BFG consumes 40 cells per shot
    // -----------------------------------------------------------------------

    #[test]
    fn bfg_consumes_40_cells() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Bfg;
        // Give enough cells to fire the BFG.
        gs.player.give_ammo(AmmoType::Cells as usize, 300);
        let before = gs.player.ammo(AmmoType::Cells as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Cells as usize);
        assert_eq!(
            after,
            before - 40,
            "BFG must consume exactly 40 cells per shot"
        );
    }

    // -----------------------------------------------------------------------
    // Additional: no-fire when insufficient ammo (ammo check)
    // -----------------------------------------------------------------------

    #[test]
    fn no_fire_when_insufficient_ammo() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Bfg;
        // Only 5 cells — not enough for BFG (needs 40).
        gs.player.give_ammo(AmmoType::Cells as usize, 5);
        let before = gs.player.ammo(AmmoType::Cells as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Cells as usize);
        assert_eq!(after, before, "no ammo must be consumed when insufficient");
    }

    // -----------------------------------------------------------------------
    // Additional: chainsaw never consumes ammo
    // -----------------------------------------------------------------------

    #[test]
    fn chainsaw_never_consumes_ammo() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Chainsaw;
        let before = gs.player.ammo(AmmoType::Bullets as usize);
        let handle = gs.player.handle;
        fire_weapon(&mut gs, None, handle);
        let after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(after, before, "chainsaw must not consume any ammo");
    }

    // -----------------------------------------------------------------------
    // Additional: player_can_fire with chainsaw always true
    // -----------------------------------------------------------------------

    #[test]
    fn player_can_fire_chainsaw_always_true() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Chainsaw;
        let _ = gs.player.use_ammo(AmmoType::Bullets as usize, 200);
        assert!(player_can_fire(&gs), "chainsaw must always be fireable");
    }

    #[test]
    fn setup_psprites_starts_with_weapon_raise_state() {
        let mut gs = make_game_state();

        setup_psprites(&mut gs.player);

        let weapon = gs.player.psprites[psprite_slots::WEAPON];
        assert_eq!(
            weapon.state,
            crate::mobj::StateNum(ids::S_PISTOL_UP),
            "setup must begin by raising the ready weapon from the bottom"
        );
        assert_eq!(weapon.sy, WEAPON_BOTTOM, "weapon should start lowered");
    }

    #[test]
    fn tick_psprites_raise_reaches_ready_state() {
        let mut gs = make_game_state();
        setup_psprites(&mut gs.player);

        for _ in 0..24 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }

        let weapon = gs.player.psprites[psprite_slots::WEAPON];
        assert_eq!(
            weapon.state,
            crate::mobj::StateNum(ids::S_PISTOL_READY),
            "raising should settle on the ready state"
        );
        assert_eq!(weapon.sy, WEAPON_TOP, "ready weapon should be fully raised");
    }

    #[test]
    fn tick_psprites_attack_enters_fire_and_flash_states() {
        let mut gs = make_game_state();
        setup_psprites(&mut gs.player);
        for _ in 0..24 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }

        tick_psprites(&mut gs, cmd_with_buttons(bt::BT_ATTACK), None);

        assert_eq!(
            gs.player.psprites[psprite_slots::WEAPON].state,
            crate::mobj::StateNum(ids::S_PISTOL1),
            "attack from ready should enter the pistol firing sequence"
        );
        assert_eq!(
            gs.player.psprites[psprite_slots::FLASH].state,
            crate::mobj::StateNum(ids::S_PISTOL_FLASH1),
            "attack should also start the muzzle-flash psprite"
        );
    }

    #[test]
    fn pistol_attack_sets_player_attack_state_and_extra_light() {
        let mut gs = make_game_state();
        ready_player_psprites(&mut gs);

        tick_psprites(&mut gs, cmd_with_buttons(bt::BT_ATTACK), None);

        let player_mobj = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player mobj must exist");
        assert_eq!(
            player_mobj.state,
            crate::mobj::StateNum(ids::S_PLAY_ATK2),
            "pistol attack should leave the player mobj in the flash attack state"
        );
        assert_eq!(
            gs.player.extra_light, 1,
            "pistol flash should apply the first extra-light level"
        );
    }

    #[test]
    fn fist_attack_uses_player_attack_state_without_extra_light_and_ready_resets_both() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Fist;
        gs.player.give_weapon(WeaponType::Fist);
        ready_player_psprites(&mut gs);

        tick_psprites(&mut gs, cmd_with_buttons(bt::BT_ATTACK), None);

        let player_mobj = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player mobj must exist");
        assert_eq!(
            player_mobj.state,
            crate::mobj::StateNum(ids::S_PLAY_ATK1),
            "melee attacks should put the player mobj into the primary attack state"
        );
        assert_eq!(
            gs.player.extra_light, 0,
            "melee attacks should not illuminate the scene"
        );

        for _ in 0..24 {
            tick_psprites(&mut gs, TicCmd::default(), None);
            if gs.player.psprites[psprite_slots::WEAPON].state
                == crate::mobj::StateNum(ids::S_PUNCH_READY)
            {
                break;
            }
        }

        let player_mobj = gs
            .mobjslab
            .get(gs.player.handle)
            .expect("player mobj must still exist");
        assert_eq!(
            player_mobj.state,
            crate::mobj::StateNum(ids::S_PLAY),
            "returning to the ready loop should restore the normal player state"
        );
        assert_eq!(
            gs.player.extra_light, 0,
            "weapon ready should clear any lingering extra-light bonus"
        );
    }

    #[test]
    fn tick_psprites_weapon_change_lowers_then_raises_new_weapon() {
        let mut gs = make_game_state();
        gs.player.give_weapon(WeaponType::Shotgun);
        gs.player.give_ammo(AmmoType::Shells as usize, 4);
        setup_psprites(&mut gs.player);
        for _ in 0..24 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }

        tick_psprites(&mut gs, cmd_with_buttons(bt::BT_CHANGE | (2u8 << 3)), None);

        assert_eq!(
            gs.player.pending_weapon,
            Some(WeaponType::Shotgun),
            "weapon change should stage the requested weapon as pending"
        );
        assert_eq!(
            gs.player.psprites[psprite_slots::WEAPON].state,
            crate::mobj::StateNum(ids::S_PISTOL_DOWN),
            "switching should lower the current weapon first"
        );

        for _ in 0..48 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }

        assert_eq!(
            gs.player.weapon,
            WeaponType::Shotgun,
            "ready weapon should change only after the lower/raise transition"
        );
        assert_eq!(
            gs.player.psprites[psprite_slots::WEAPON].state,
            crate::mobj::StateNum(ids::S_SGUN_READY),
            "the new weapon should end in its ready state"
        );
    }

    #[test]
    fn tick_psprites_resyncs_after_out_of_band_weapon_change() {
        let mut gs = make_game_state();
        setup_psprites(&mut gs.player);
        for _ in 0..24 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }

        gs.player.weapon = WeaponType::Shotgun;
        gs.player.give_weapon(WeaponType::Shotgun);
        gs.player.give_ammo(AmmoType::Shells as usize, 4);

        tick_psprites(&mut gs, TicCmd::default(), None);

        assert_eq!(
            STATES[gs.player.psprites[psprite_slots::WEAPON].state.0 as usize].sprite,
            sprite_names::SPR_SHTG,
            "psprite ticking must repair stale weapon overlay state after an out-of-band weapon change"
        );
    }

    #[test]
    fn plasma_attack_starts_flash_psprite() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::PlasmaRifle;
        gs.player.give_weapon(WeaponType::PlasmaRifle);
        gs.player.give_ammo(AmmoType::Cells as usize, 50);
        ready_player_psprites(&mut gs);

        let cells_before = gs.player.ammo(AmmoType::Cells as usize);
        let plasma_before = count_mobjs_of_kind(&gs, MobjKind::PlasmaBall);

        tick_psprites(&mut gs, cmd_with_buttons(bt::BT_ATTACK), None);

        assert_eq!(
            gs.player.psprites[psprite_slots::WEAPON].state,
            crate::mobj::StateNum(ids::S_PLASMA1),
            "plasma attack should enter the first plasma fire state"
        );
        assert_ne!(
            gs.player.psprites[psprite_slots::FLASH].state,
            StateNum::NULL,
            "plasma attack should start a dedicated muzzle-flash psprite"
        );
        assert_eq!(
            gs.player.ammo(AmmoType::Cells as usize),
            cells_before - 1,
            "plasma attack should consume one cell on the firing tic"
        );
        assert_eq!(
            count_mobjs_of_kind(&gs, MobjKind::PlasmaBall),
            plasma_before + 1,
            "plasma attack should spawn a plasma ball on the firing tic"
        );
    }

    #[test]
    fn held_plasma_fires_a_second_shot_six_tics_later() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::PlasmaRifle;
        gs.player.give_weapon(WeaponType::PlasmaRifle);
        gs.player.give_ammo(AmmoType::Cells as usize, 50);
        ready_player_psprites(&mut gs);

        let cells_before = gs.player.ammo(AmmoType::Cells as usize);
        let plasma_before = count_mobjs_of_kind(&gs, MobjKind::PlasmaBall);
        let attack = cmd_with_buttons(bt::BT_ATTACK);

        tick_psprites(&mut gs, attack, None);
        for _ in 0..6 {
            tick_psprites(&mut gs, attack, None);
        }

        assert_eq!(
            gs.player.ammo(AmmoType::Cells as usize),
            cells_before - 2,
            "held plasma should fire its second burst shot before returning to ready"
        );
        assert_eq!(
            count_mobjs_of_kind(&gs, MobjKind::PlasmaBall),
            plasma_before + 2,
            "held plasma should spawn a second plasma ball during the second firing state"
        );
    }

    #[test]
    fn chaingun_attack_cycle_fires_a_second_shot_before_returning_ready() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Chaingun;
        gs.player.give_weapon(WeaponType::Chaingun);
        gs.player.give_ammo(AmmoType::Bullets as usize, 50);
        ready_player_psprites(&mut gs);

        let bullets_before = gs.player.ammo(AmmoType::Bullets as usize);

        tick_psprites(&mut gs, cmd_with_buttons(bt::BT_ATTACK), None);
        assert_eq!(
            gs.player.ammo(AmmoType::Bullets as usize),
            bullets_before - 1,
            "the first chaingun firing state should consume one bullet immediately"
        );

        for _ in 0..4 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }

        assert_eq!(
            gs.player.ammo(AmmoType::Bullets as usize),
            bullets_before - 2,
            "the second chaingun firing state should also fire before returning to ready"
        );
    }

    #[test]
    fn bfg_attack_delays_projectile_and_ammo_until_the_second_attack_state() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::Bfg;
        gs.player.give_weapon(WeaponType::Bfg);
        gs.player.give_ammo(AmmoType::Cells as usize, 200);
        ready_player_psprites(&mut gs);

        let cells_before = gs.player.ammo(AmmoType::Cells as usize);
        let bfg_before = count_mobjs_of_kind(&gs, MobjKind::BfgBall);

        tick_psprites(&mut gs, cmd_with_buttons(bt::BT_ATTACK), None);

        assert_eq!(
            gs.player.psprites[psprite_slots::WEAPON].state,
            crate::mobj::StateNum(ids::S_BFG1),
            "BFG attack should enter its windup state first"
        );
        assert_eq!(
            gs.player.ammo(AmmoType::Cells as usize),
            cells_before,
            "BFG windup should not consume ammo before the actual fire state"
        );
        assert_eq!(
            count_mobjs_of_kind(&gs, MobjKind::BfgBall),
            bfg_before,
            "BFG windup should not spawn the projectile yet"
        );

        for _ in 0..20 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }

        assert_eq!(
            gs.player.psprites[psprite_slots::WEAPON].state,
            crate::mobj::StateNum(ids::S_BFG2),
            "after the windup, the BFG should advance into the actual firing state"
        );
        assert_eq!(
            gs.player.ammo(AmmoType::Cells as usize),
            cells_before - 40,
            "the BFG should consume its 40 cells when the fire state begins"
        );
        assert_eq!(
            count_mobjs_of_kind(&gs, MobjKind::BfgBall),
            bfg_before + 1,
            "the BFG projectile should spawn when the fire state begins"
        );
    }

    #[test]
    fn super_shotgun_reload_sequence_queues_open_load_and_close_sounds() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::SuperShotgun;
        gs.player.give_weapon(WeaponType::SuperShotgun);
        gs.player.give_ammo(AmmoType::Shells as usize, 4);
        ready_player_psprites(&mut gs);

        tick_psprites(&mut gs, cmd_with_buttons(bt::BT_ATTACK), None);
        gs.sound.sound_queue.clear();

        for _ in 0..17 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }
        assert!(
            gs.sound.sound_queue.is_empty(),
            "the super shotgun should not play reload sounds before the reload sequence starts"
        );

        for _ in 0..7 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }
        assert_eq!(
            gs.sound.sound_queue,
            vec![SoundRequest::PlayerSuperShotgunOpen]
        );
        gs.sound.sound_queue.clear();

        for _ in 0..6 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }
        assert_eq!(
            gs.sound.sound_queue,
            vec![SoundRequest::PlayerSuperShotgunLoad]
        );
        gs.sound.sound_queue.clear();

        for _ in 0..6 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }
        assert_eq!(
            gs.sound.sound_queue,
            vec![SoundRequest::PlayerSuperShotgunClose]
        );
    }

    #[test]
    fn super_shotgun_empty_after_firing_lowers_at_reload_check() {
        let mut gs = make_game_state();
        gs.player.weapon = WeaponType::SuperShotgun;
        gs.player.give_weapon(WeaponType::SuperShotgun);
        gs.player.give_ammo(AmmoType::Shells as usize, 2);
        ready_player_psprites(&mut gs);

        tick_psprites(&mut gs, cmd_with_buttons(bt::BT_ATTACK), None);

        for _ in 0..17 {
            tick_psprites(&mut gs, TicCmd::default(), None);
        }

        assert_eq!(
            gs.player.psprites[psprite_slots::WEAPON].state,
            StateNum(ids::S_DSGUN_DOWN),
            "the super shotgun should begin lowering as soon as its reload check sees no shells left"
        );
        assert_eq!(
            gs.player.pending_weapon,
            Some(WeaponType::Pistol),
            "running the super shotgun dry should stage the next usable weapon"
        );
    }
}
