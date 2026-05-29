#![allow(unused_imports)]
#![allow(dead_code)]
use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

use super::*;
use crate::mobj::MobjHandle;
use crate::movers::*;
use crate::state::*;

// tick_sector_specials (legacy, kept for backward compatibility)

/// Apply sector special damage to the actor each tic (legacy version).
///
/// Simplified port of `P_PlayerInSpecialSector`.
///
/// Sector containment is approximated: the actor is considered to be "in" a
/// special sector if `actor.z.to_int() == sector.floor_height as i32`.
///
/// Damage sectors update both the player state and player mobj health so
/// monster AI sees the same liveness the HUD does.
pub fn tick_sector_specials(gs: &mut GameState, level: &Level, handle: MobjHandle) {
    // Read actor position.
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let (az, _ax, _ay) = (mo.z.to_int(), mo.x.to_int(), mo.y.to_int());

    for sector in &level.sectors {
        if sector.special == 0 {
            continue;
        }

        // Only apply damage if actor is standing on this floor.
        if az != sector.floor_height as i32 {
            continue;
        }

        let dmg: i32 = match crate::state::SectorDamageType::from_repr(sector.special) {
            Some(crate::state::SectorDamageType::Hellslime) => LEGACY_DAMAGE_HELLSLIME,
            Some(crate::state::SectorDamageType::Nukage) => LEGACY_DAMAGE_NUKAGE,
            Some(crate::state::SectorDamageType::SuperHellslime) => LEGACY_DAMAGE_SUPER_HELLSLIME,
            _ => continue,
        };

        apply_sector_damage(gs, handle, dmg);

        // Only apply one sector's damage per tic (first match wins).
        return;
    }
}

// tick_sector_damage (periodic, with RadSuit and God exit)

/// Period in tics between sector damage applications (Doom standard: 32 tics).
const SECTOR_DAMAGE_PERIOD: u32 = 32;

/// Apply periodic sector damage to the player based on sector specials.
///
/// Damage is only applied every `SECTOR_DAMAGE_PERIOD` tics (based on `level_time`).
/// RadSuit (`powers[PW_IRONFEET] > 0`) prevents damage for types 4, 5, 7, 16
/// but NOT type 11 (God exit).
///
/// Sector specials:
/// - **4**: -20% health randomly (nukage, blink) — ~5 damage per period
/// - **5**: -10% health (hellslime) — ~5 damage per period
/// - **7**: -5% health (nukage, no blink) — ~2 damage per period
/// - **11**: -20% health + end level when health <= 10 (God exit) — RadSuit does NOT protect
/// - **16**: -20% health (super hellslime) — ~20 damage per period
pub fn tick_sector_damage(gs: &mut GameState, level: &Level) {
    // Only apply damage every SECTOR_DAMAGE_PERIOD tics.
    if !gs.stats.level_time.is_multiple_of(SECTOR_DAMAGE_PERIOD) {
        return;
    }

    let handle = gs.player.handle;

    // Read actor position.
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let az = mo.z.to_int();

    // Check if player has RadSuit active.
    let has_radsuit = gs.player.powers[crate::player::powers::PW_IRONFEET] > 0;

    for sector in &level.sectors {
        if sector.special == 0 {
            continue;
        }

        // Only apply damage if actor is standing on this floor.
        if az != sector.floor_height as i32 {
            continue;
        }

        let Some(damage_type) = crate::state::SectorDamageType::from_repr(sector.special) else {
            continue;
        };

        let (damage, ignores_radsuit) = match damage_type {
            crate::state::SectorDamageType::NukageBlink => (PERIODIC_DAMAGE_NUKAGE_BLINK, false),
            crate::state::SectorDamageType::Hellslime => (PERIODIC_DAMAGE_HELLSLIME, false),
            crate::state::SectorDamageType::Nukage => (PERIODIC_DAMAGE_NUKAGE, false),
            crate::state::SectorDamageType::GodExit => (PERIODIC_DAMAGE_GOD_EXIT, true),
            crate::state::SectorDamageType::SuperHellslime => {
                (PERIODIC_DAMAGE_SUPER_HELLSLIME, false)
            }
        };

        if ignores_radsuit || !has_radsuit {
            apply_sector_damage(gs, handle, damage);
        }

        // God exit specific behavior
        if damage_type == crate::state::SectorDamageType::GodExit {
            if let Some(mo) = gs.mobjslab.get(handle) {
                if mo.health <= 10 {
                    gs.exit_request = Some(ExitRequest::Normal);
                }
            }
        }

        // Only apply one sector's damage per period (first match wins).
        return;
    }
}

/// Apply damage to an actor from a sector special.
fn apply_sector_damage(gs: &mut GameState, handle: MobjHandle, damage: i32) {
    if handle == gs.player.handle {
        gs.damage_player(damage);
    } else if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.health -= damage;
        if mo.health < 0 {
            mo.health = 0;
        }
    }
}
