use super::constants::*;
use crate::mobj::MobjHandle;
use crate::state::*;
use doom_map::Level;

// ---------------------------------------------------------------------------
// tick_sector_specials (legacy, kept for backward compatibility)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// tick_sector_damage (periodic, with RadSuit and God exit)
// ---------------------------------------------------------------------------

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
pub fn apply_sector_damage(gs: &mut GameState, handle: MobjHandle, damage: i32) {
    if handle == gs.player.handle {
        gs.damage_player(damage);
    } else if let Some(mo) = gs.mobjslab.get_mut(handle) {
        mo.health -= damage;
        if mo.health < 0 {
            mo.health = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// player_sector_index
// ---------------------------------------------------------------------------

/// Find which sector the player is standing in.
///
/// Simple linear scan checking if the player's Z coordinate matches a sector's
/// floor height. Returns the index of the first matching sector, or `None` if
/// no match is found.
///
/// This is a simplified approximation: a proper implementation would use the
/// BSP tree or blockmap for point-in-sector queries.
pub fn player_sector_index(gs: &GameState, level: &Level) -> Option<usize> {
    let handle = gs.player.handle;
    let az = gs.mobjslab.get(handle)?.z.to_int();

    for (i, sector) in level.sectors.iter().enumerate() {
        if az == sector.floor_height as i32 {
            return Some(i);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Secret sector tracking
// ---------------------------------------------------------------------------

/// Detect when the player enters a sector with special 9 (secret sector) and
/// increment `secret_count`.  Clears the sector special to prevent double-counting.
///
/// Call once per tic after player movement is resolved.
pub fn tick_sector_secrets(gs: &mut GameState, level: &mut Level) {
    if let Some(sector_idx) = player_sector_index(gs, level) {
        if level.sectors[sector_idx].special == 9 {
            gs.stats.secret_count += 1;
            level.sectors[sector_idx].special = 0;
        }
    }
}
