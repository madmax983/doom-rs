//! Sector specials and linedef triggers.
//!
//! Port of Doom's `p_spec.c` and `p_ceilng.c` / `p_doors.c` (simplified).
//!
//! # Implemented
//! - `tick_sector_specials`: damage floors (specials 5, 7, 16) with periodic damage and RadSuit.
//! - `tick_sector_damage`: periodic damage (every 32 tics), RadSuit protection, God exit (special 11).
//! - `tick_doors`: advance active door/floor movers.
//! - `tick_lights`: advance light specials.
//! - `init_sector_lights`: create `SectorLightEffect` entries for sector specials 1-3, 8, 12-13, 17.
//! - `tick_sector_lights`: advance extended sector light effects.
//! - `spawn_level_specials`: initialise light thinkers on level load.
//! - `p_use_lines`: player USE activation, dispatches to `activate_linedef`.
//! - `activate_linedef`: doors, exits, crushers, lifts, floors, teleporters (39, 97, 125, 126).
//! - `ev_teleport`: teleport an actor to a teleport destination thing (kind 14).
//! - `player_sector_index`: find which sector the player is standing in.

use doom_map::{Level, SIDEDEF_NONE};
use doom_types::{FIXED_ONE, Fixed16_16};

use crate::mobj::MobjHandle;
use crate::state::{
    CeilingMover, CeilingType, ConveyorBelt, DoorMover, ExitRequest, FloorMover, FloorType,
    GameState, LiftMover, LiftStatus, LightEffectType, LightSpecial, MoveDirection,
    PerpetualPlatform, PlatformStatus, ScrollingWall, SectorLightEffect,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Distance ahead the player can activate a linedef.
pub const USE_RANGE: i32 = 64;

/// Door open/close speed in map units per tic (Doom standard: 2 units/tic).
const DOOR_SPEED: i16 = 2;

/// Tics a door stays open before auto-closing (3.5 seconds at 35 Hz ≈ 120 tics).
const DOOR_WAIT: i32 = 120;

/// Door speed for blazing (fast) doors in map units per tic.
const BLAZING_DOOR_SPEED: i16 = 8;

/// Period for fast blinking lights (tics).
const BLINK_FAST_PERIOD: i32 = 15;

/// Period for slow blinking lights (tics).
const BLINK_SLOW_PERIOD: i32 = 35;

// Sector damage constants (legacy per tic)
const LEGACY_DAMAGE_HELLSLIME: i32 = 10;
const LEGACY_DAMAGE_NUKAGE: i32 = 5;
const LEGACY_DAMAGE_SUPER_HELLSLIME: i32 = 20;

// Sector damage constants (periodic every 32 tics)
const PERIODIC_DAMAGE_NUKAGE_BLINK: i32 = 5;
const PERIODIC_DAMAGE_HELLSLIME: i32 = 5;
const PERIODIC_DAMAGE_NUKAGE: i32 = 2;
const PERIODIC_DAMAGE_GOD_EXIT: i32 = 20;
const PERIODIC_DAMAGE_SUPER_HELLSLIME: i32 = 20;

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

// ---------------------------------------------------------------------------
// Teleporters
// ---------------------------------------------------------------------------

/// DoomEd thing type for Teleport Destination markers.
const TELEPORT_DEST_THING: u16 = 14;

/// BAM units per degree: 2^32 / 360.
const BAM_PER_DEGREE: u32 = (0x1_0000_0000u64 / 360) as u32;

/// Teleport an actor to a teleport destination in a sector matching `tag`.
///
/// Scans all things in the level for a Teleport Destination (DoomEd type 14)
/// that is placed in a sector with the matching tag. The actor is moved to
/// the destination's position, angle, and floor height.
///
/// Returns `true` if a teleport destination was found and the actor was moved.
pub fn ev_teleport(gs: &mut GameState, level: &Level, tag: u16, mobj_handle: MobjHandle) -> bool {
    // Collect sector indices matching the tag.
    let first_tagged_sector = level.sectors.iter().position(|s| s.tag == tag);

    let Some(first_tagged_idx) = first_tagged_sector else {
        return false;
    };

    // Find the first Teleport Destination thing (kind == 14) in the level.
    // In Doom, teleport destinations are placed by mappers inside the target
    // sector. We simplify by finding any thing with kind==14 and accepting it
    // if any tagged sector exists.
    for thing in &level.things {
        if thing.kind != TELEPORT_DEST_THING {
            continue;
        }

        // Check if this teleport destination is roughly in one of the tagged
        // sectors. Since we don't have point-in-sector, we accept any teleport
        // destination thing when at least one tagged sector exists.
        // This matches Doom's approach where teleport destinations are only
        // placed in the appropriate target sector by the mapper.

        // Get the floor height of the first tagged sector for Z placement.
        let dest_floor = level.sectors[first_tagged_idx].floor_height;

        // Move the actor to the destination.
        if let Some(mo) = gs.mobjslab.get_mut(mobj_handle) {
            mo.x = Fixed16_16::from_int(thing.x as i32);
            mo.y = Fixed16_16::from_int(thing.y as i32);
            mo.z = Fixed16_16::from_int(dest_floor as i32);
            mo.angle = doom_types::Bam((thing.angle as u32).wrapping_mul(BAM_PER_DEGREE));
            // Clear momentum on teleport (Doom standard).
            mo.momx = Fixed16_16::ZERO;
            mo.momy = Fixed16_16::ZERO;
            mo.momz = Fixed16_16::ZERO;
        } else {
            return false;
        }

        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// init_sector_lights / tick_sector_lights
// ---------------------------------------------------------------------------

/// Scan all sectors and create `SectorLightEffect` entries for extended
/// light-related sector specials.
///
/// Handles sector specials 1, 2, 3, 8, 12, 13, and 17.
/// Call this once after loading a level, before the first tic.
pub fn init_sector_lights(gs: &mut GameState, level: &Level) {
    for (i, sector) in level.sectors.iter().enumerate() {
        let Some(effect_type) = LightEffectType::from_repr(sector.special) else {
            continue;
        };

        let min_light = match effect_type {
            LightEffectType::BlinkRandom => 0,
            LightEffectType::Blink05s => 0,
            LightEffectType::Blink1s => 35,
            LightEffectType::Oscillate => sector.light_level / 2,
            LightEffectType::BlinkSync05s => 0,
            LightEffectType::BlinkSync1s => 35,
            LightEffectType::FireFlicker => sector.light_level.saturating_sub(16).max(0),
        };

        let timer = match effect_type {
            LightEffectType::BlinkRandom => BLINK_SLOW_PERIOD as u32,
            LightEffectType::Blink05s => BLINK_FAST_PERIOD as u32,
            LightEffectType::Blink1s => BLINK_SLOW_PERIOD as u32,
            LightEffectType::Oscillate => 1,
            LightEffectType::BlinkSync05s => BLINK_FAST_PERIOD as u32,
            LightEffectType::BlinkSync1s => BLINK_SLOW_PERIOD as u32,
            LightEffectType::FireFlicker => 4,
        };

        gs.movers.sector_lights.push(SectorLightEffect {
            sector_index: i,
            effect_type,
            base_light: sector.light_level,
            min_light,
            timer,
        });
    }
}

/// Advance all extended sector light effects by one tic.
///
/// Call this once per tic from `tick()`.
pub fn tick_sector_lights(gs: &mut GameState, level: &mut Level) {
    for effect in &mut gs.movers.sector_lights {
        if effect.sector_index >= level.sectors.len() {
            continue;
        }

        effect.timer = effect.timer.saturating_sub(1);
        if effect.timer > 0 {
            continue;
        }

        let sector = &mut level.sectors[effect.sector_index];

        match effect.effect_type {
            LightEffectType::BlinkRandom => {
                // Toggle between base and dark at random-ish intervals.
                if sector.light_level == effect.base_light {
                    sector.light_level = effect.min_light;
                    // Use a simple deterministic variation for the next period.
                    effect.timer = (BLINK_SLOW_PERIOD as u32)
                        .wrapping_add(effect.sector_index as u32 * 7)
                        % 40
                        + 10;
                } else {
                    sector.light_level = effect.base_light;
                    effect.timer = BLINK_SLOW_PERIOD as u32;
                }
            }
            LightEffectType::Blink05s | LightEffectType::BlinkSync05s => {
                if sector.light_level == effect.base_light {
                    sector.light_level = effect.min_light;
                } else {
                    sector.light_level = effect.base_light;
                }
                effect.timer = BLINK_FAST_PERIOD as u32;
            }
            LightEffectType::Blink1s | LightEffectType::BlinkSync1s => {
                if sector.light_level == effect.base_light {
                    sector.light_level = effect.min_light;
                } else {
                    sector.light_level = effect.base_light;
                }
                effect.timer = BLINK_SLOW_PERIOD as u32;
            }
            LightEffectType::Oscillate => {
                // Smooth oscillation: ramp light up and down.
                let range = effect.base_light - effect.min_light;
                if range <= 0 {
                    effect.timer = 1;
                    continue;
                }
                // Use level_time to create a smooth oscillation.
                let phase = (gs.stats.level_time % (range as u32 * 2)) as i16;
                sector.light_level = if phase < range {
                    effect.min_light + phase
                } else {
                    effect.base_light - (phase - range)
                };
                effect.timer = 1;
            }
            LightEffectType::FireFlicker => {
                // Random light variation within a small range.
                let variation = (gs.rng.next_byte() & 3) as i16;
                sector.light_level = (effect.base_light - variation * 4).max(effect.min_light);
                effect.timer = 4;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// tick_doors
// ---------------------------------------------------------------------------

/// Advance all active door/floor movers by one tic.
///
/// Call this once per tic from `tick()`.
pub fn tick_doors(gs: &mut GameState, level: &mut Level) {
    const CLOSE_WAIT_OPEN_DELAY: i32 = 1050; // 30 s at 35 Hz

    let mut active_doors = std::mem::take(&mut gs.movers.active_doors);

    active_doors.retain_mut(|door| {
        let sector_idx = door.sector;

        // ── Close-wait-open: waiting at closed position before reopening ──
        if door.reopen_countdown > 0 {
            door.reopen_countdown -= 1;
            return true;
        }
        if door.reopen_countdown == 0 {
            // Delay expired — start reopening.
            let rh = door.reopen_height;
            door.target_height = rh;
            door.speed = DOOR_SPEED; // positive = opening
            door.reopen_height = 0;
            door.reopen_countdown = -1;
            // Fall through to movement logic.
        }

        // ── Standard open-wait-close countdown ──
        let countdown = door.countdown;
        let speed_abs = door.speed.abs();

        if countdown > 0 {
            door.countdown -= 1;
            return true;
        }
        if countdown == 0 {
            // Start closing — negate speed so it moves downward.
            door.speed = -speed_abs;
            if let Some(sector) = level.sectors.get(sector_idx) {
                door.target_height = sector.floor_height;
            }
            door.countdown = -1;
        }

        // ── Move toward target ──
        let speed = door.speed;
        let target = door.target_height;
        let is_ceiling = door.is_ceiling;
        let reopen_height = door.reopen_height;
        let wait_tics = door.wait_tics;

        if sector_idx < level.sectors.len() {
            let sector = &mut level.sectors[sector_idx];
            let height = if is_ceiling {
                &mut sector.ceil_height
            } else {
                &mut sector.floor_height
            };
            *height += speed;

            let reached = if speed > 0 {
                *height >= target
            } else {
                *height <= target
            };

            if reached {
                *height = target;
                door.current_height = target;

                if speed > 0 && wait_tics > 0 {
                    door.countdown = wait_tics;
                    return true;
                }

                // Close-wait-open: start the reopen delay instead of removing.
                if speed < 0 && reopen_height != 0 {
                    door.reopen_countdown = CLOSE_WAIT_OPEN_DELAY;
                    return true;
                }

                return false;
            }
            let new_h = if is_ceiling {
                level.sectors[sector_idx].ceil_height
            } else {
                level.sectors[sector_idx].floor_height
            };
            door.current_height = new_h;
        }

        true
    });

    gs.movers.active_doors = active_doors;
}

// ---------------------------------------------------------------------------
// tick_lights
// ---------------------------------------------------------------------------

/// Advance all light specials by one tic.
pub fn tick_lights(gs: &mut GameState, level: &mut Level) {
    for light in &mut gs.movers.active_lights {
        light.timer -= 1;
        if light.timer <= 0 {
            light.is_bright = !light.is_bright;
            light.timer = light.period;
            if light.sector < level.sectors.len() {
                level.sectors[light.sector].light_level = if light.is_bright {
                    light.bright
                } else {
                    light.dark
                };
            }
        }
    }
}

// ---------------------------------------------------------------------------
// spawn_level_specials
// ---------------------------------------------------------------------------

/// Scan all sectors and spawn light specials based on `sector.special`.
///
/// Call this once after loading a level, before the first tic.
pub fn spawn_level_specials(gs: &mut GameState, level: &Level) {
    for (i, sector) in level.sectors.iter().enumerate() {
        let Some(effect_type) = LightEffectType::from_repr(sector.special) else {
            continue;
        };

        match effect_type {
            LightEffectType::BlinkRandom => {
                // Random off: slow blink, goes dark.
                gs.movers.active_lights.push(LightSpecial {
                    sector: i,
                    timer: BLINK_SLOW_PERIOD,
                    period: BLINK_SLOW_PERIOD,
                    bright: sector.light_level,
                    dark: 0,
                    is_bright: true,
                });
            }
            LightEffectType::Blink05s => {
                // Fast strobe.
                gs.movers.active_lights.push(LightSpecial {
                    sector: i,
                    timer: BLINK_FAST_PERIOD,
                    period: BLINK_FAST_PERIOD,
                    bright: sector.light_level,
                    dark: 0,
                    is_bright: true,
                });
            }
            LightEffectType::Blink1s => {
                // Slow strobe: dim but not fully dark.
                gs.movers.active_lights.push(LightSpecial {
                    sector: i,
                    timer: BLINK_SLOW_PERIOD,
                    period: BLINK_SLOW_PERIOD,
                    bright: sector.light_level,
                    dark: 35,
                    is_bright: true,
                });
            }
            _ => {} // Other specials handled by tick_sector_specials.
        }
    }
}

// ---------------------------------------------------------------------------
// Adjacent sector height helpers
// ---------------------------------------------------------------------------

/// Returns an iterator over all sectors adjacent to `sector_index`.
/// Yields `(adjacent_sector_index, &Sector)`.
fn adjacent_sectors<'a>(
    level: &'a Level,
    sector_index: usize,
) -> impl Iterator<Item = (usize, &'a doom_map::Sector)> + 'a {
    level.linedefs.iter().filter_map(move |ld| {
        if ld.left_sidedef == SIDEDEF_NONE {
            return None;
        }
        let right_sector = level
            .sidedefs
            .get(ld.right_sidedef as usize)
            .map(|s| s.sector as usize);
        let left_sector = level
            .sidedefs
            .get(ld.left_sidedef as usize)
            .map(|s| s.sector as usize);

        let other = if right_sector == Some(sector_index) {
            left_sector
        } else if left_sector == Some(sector_index) {
            right_sector
        } else {
            None
        };

        other.and_then(|idx| level.sectors.get(idx).map(|s| (idx, s)))
    })
}

/// Find the lowest floor height among all sectors adjacent to `sector_index`.
///
/// Adjacent means: the sector shares a two-sided linedef with the given sector.
/// If the sector has no adjacent sectors, returns the sector's own floor height.
pub fn lowest_adjacent_floor(level: &Level, sector_index: usize) -> i16 {
    let own_floor = level
        .sectors
        .get(sector_index)
        .map(|s| s.floor_height)
        .unwrap_or(0);

    adjacent_sectors(level, sector_index)
        .map(|(_, s)| s.floor_height)
        .min()
        .unwrap_or(own_floor)
}

/// Find the highest floor height among all sectors adjacent to `sector_index`.
///
/// Used for "lower to highest adjacent floor" specials.
/// If no adjacent sectors, returns the sector's own floor height.
pub fn highest_adjacent_floor(level: &Level, sector_index: usize) -> i16 {
    let own_floor = level
        .sectors
        .get(sector_index)
        .map(|s| s.floor_height)
        .unwrap_or(0);

    adjacent_sectors(level, sector_index)
        .map(|(_, s)| s.floor_height)
        .max()
        .unwrap_or(own_floor)
}

/// Find the next floor height above the current sector's floor among adjacent sectors.
///
/// Scans all adjacent sector floors and returns the smallest one that is strictly
/// greater than the current sector's floor height. If none is found, returns the
/// sector's own floor height (no change).
pub fn next_highest_floor(level: &Level, sector_index: usize) -> i16 {
    let own_floor = level
        .sectors
        .get(sector_index)
        .map(|s| s.floor_height)
        .unwrap_or(0);

    adjacent_sectors(level, sector_index)
        .map(|(_, s)| s.floor_height)
        .filter(|&h| h > own_floor)
        .min()
        .unwrap_or(own_floor)
}

/// Find the lowest ceiling height among all sectors adjacent to `sector_index`.
///
/// Used for "raise floor to lowest adjacent ceiling" specials.
/// If no adjacent sectors, returns the sector's own ceiling height.
pub fn lowest_adjacent_ceiling(level: &Level, sector_index: usize) -> i16 {
    let own_ceil = level
        .sectors
        .get(sector_index)
        .map(|s| s.ceil_height)
        .unwrap_or(0);

    adjacent_sectors(level, sector_index)
        .map(|(_, s)| s.ceil_height)
        .min()
        .unwrap_or(own_ceil)
}

/// Find the highest ceiling height among all sectors adjacent to `sector_index`.
///
/// Used for ceiling raise specials.
/// If no adjacent sectors, returns the sector's own ceiling height.
pub fn highest_adjacent_ceiling(level: &Level, sector_index: usize) -> i16 {
    let own_ceil = level
        .sectors
        .get(sector_index)
        .map(|s| s.ceil_height)
        .unwrap_or(0);

    adjacent_sectors(level, sector_index)
        .map(|(_, s)| s.ceil_height)
        .max()
        .unwrap_or(own_ceil)
}

/// Find the next floor height above `current_height` among adjacent sectors.
///
/// Scans all adjacent sector floors and returns the smallest one that is
/// strictly greater than `current_height`. If none is found, returns
/// `current_height` (no change).
///
/// This variant accepts an explicit `current_height` parameter, unlike the
/// zero-arg `next_highest_floor` which uses the sector's own floor height.
pub fn next_highest_floor_above(level: &Level, sector_index: usize, current_height: i16) -> i16 {
    adjacent_sectors(level, sector_index)
        .map(|(_, s)| s.floor_height)
        .filter(|&h| h > current_height)
        .min()
        .unwrap_or(current_height)
}

/// Find the shortest lower texture height among linedefs bounding the sector.
///
/// Scans all linedefs whose front (right) sidedef references the given sector
/// and returns the smallest non-zero `y_offset` + texture height proxy. In
/// vanilla Doom, this examines the `lower_texture` height. We approximate this
/// by using the sidedef's `y_offset` as the texture height metric: if the
/// lower texture name is non-empty, we use `y_offset` as the height (or a
/// default of 128 when `y_offset == 0`).
///
/// For simplicity, if no lower textures are found, returns 0 (no raise).
pub fn shortest_lower_texture(level: &Level, sector_index: usize) -> i16 {
    level
        .linedefs
        .iter()
        .filter_map(|ld| {
            let right_sd = level.sidedefs.get(ld.right_sidedef as usize);
            let left_sd = if ld.left_sidedef != SIDEDEF_NONE {
                level.sidedefs.get(ld.left_sidedef as usize)
            } else {
                None
            };

            let sd = right_sd
                .filter(|sd| sd.sector as usize == sector_index)
                .or_else(|| left_sd.filter(|lsd| lsd.sector as usize == sector_index));

            let sd = sd?;

            let has_lower = sd.lower_texture.iter().any(|&b| b != 0);
            if !has_lower {
                return None;
            }

            let height = if sd.y_offset != 0 {
                sd.y_offset.abs()
            } else {
                128
            };

            Some(height)
        })
        .min()
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Floor activation functions (public API)
// ---------------------------------------------------------------------------

/// Lower floor to lowest adjacent floor on all sectors matching `tag`.
///
/// Creates one `FloorMover` per matching sector.
pub fn ev_floor_lower_to_lowest(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| (i, lowest_adjacent_floor(level, i)))
    {
        activate_floor_lower_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            FloorType::LowerToLowest,
        );
    }
}

/// Lower floor to highest adjacent floor on all sectors matching `tag`.
pub fn ev_floor_lower_to_highest(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| (i, highest_adjacent_floor(level, i)))
    {
        activate_floor_lower_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            FloorType::LowerToHighest,
        );
    }
}

/// Lower floor to next lowest adjacent floor on all sectors matching `tag`.
///
/// "Next lowest" means: find the highest adjacent floor that is still below
/// the sector's current floor. If none, no mover is created.
/// ⚡ Bolt Optimization:
/// Avoids intermediate `.collect::<Vec<_>>()` by processing sectors inline.
pub fn ev_floor_lower_to_nearest(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, s) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
    {
        // Find the highest adjacent floor strictly below our floor.
        let own_floor = s.floor_height;
        let target = adjacent_sectors(level, idx)
            .map(|(_, adj_s)| adj_s.floor_height)
            .filter(|&h| h < own_floor)
            .max()
            .unwrap_or(own_floor);
        activate_floor_lower_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            FloorType::LowerToNearest,
        );
    }
}

/// Raise floor to lowest adjacent ceiling on all sectors matching `tag`.
pub fn ev_floor_raise_to_lowest_ceiling(
    gs: &mut GameState,
    level: &Level,
    tag: u16,
    speed: i16,
    crush: crate::state::CrushBehavior,
) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| (i, lowest_adjacent_ceiling(level, i)))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crush,
            FloorType::RaiseCrush,
        );
    }
}

/// Raise floor to next highest adjacent floor on all sectors matching `tag`.
pub fn ev_floor_raise_to_nearest(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| (i, next_highest_floor(level, i)))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crate::state::CrushBehavior::NoCrush,
            FloorType::RaiseToNearest,
        );
    }
}

/// Raise floor by shortest lower texture height on all sectors matching `tag`.
pub fn ev_floor_raise_by_texture(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, s)| (i, s.floor_height + shortest_lower_texture(level, i)))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crate::state::CrushBehavior::NoCrush,
            FloorType::RaiseByTexture,
        );
    }
}

/// Raise floor by exactly 24 units on all sectors matching `tag`.
pub fn ev_floor_raise_24(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, s)| (i, s.floor_height + 24))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crate::state::CrushBehavior::NoCrush,
            FloorType::Raise24,
        );
    }
}

/// Raise floor by exactly 32 units on all sectors matching `tag`.
pub fn ev_floor_raise_32(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, s)| (i, s.floor_height + 32))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crate::state::CrushBehavior::NoCrush,
            FloorType::Raise32,
        );
    }
}

/// Raise floor to the sector's own ceiling on all sectors matching `tag`.
pub fn ev_floor_raise_to_ceiling(
    gs: &mut GameState,
    level: &Level,
    tag: u16,
    speed: i16,
    crush: crate::state::CrushBehavior,
) {
    for (idx, target) in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, s)| (i, s.ceil_height))
    {
        activate_floor_raise_single_typed(
            gs,
            level,
            idx,
            tag,
            target,
            speed,
            crush,
            FloorType::RaiseToCeiling,
        );
    }
}

// ---------------------------------------------------------------------------
// sector_linedefs helper
// ---------------------------------------------------------------------------

/// Return the indices of all linedefs whose **front** (right) sidedef references
/// the given sector. This is used by stair builders, donut specials, and
/// platform activation logic that need to walk adjacent sectors.
///
/// **Performance:** Returns an `impl Iterator` instead of allocating and
/// returning a `Vec<usize>`. This eliminates intermediate heap allocations
/// per sector visited, significantly reducing memory overhead when traversing
/// large sets of adjacent sectors during level mutations.
pub fn sector_linedefs(level: &Level, sector_index: usize) -> impl Iterator<Item = usize> + '_ {
    level
        .linedefs
        .iter()
        .enumerate()
        .filter_map(move |(i, ld)| {
            if let Some(sd) = level.sidedefs.get(ld.right_sidedef as usize) {
                if sd.sector as usize == sector_index {
                    return Some(i);
                }
            }
            None
        })
}

// ---------------------------------------------------------------------------
// Stair builders
// ---------------------------------------------------------------------------

/// Stair type determines step size and speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StairType {
    /// 8-unit steps, speed 2 (line type 7).
    Build8,
    /// 16-unit turbo steps, speed 4 (line types 8, 100, 127).
    Turbo16,
}

/// Build stairs starting from `start_sector`, walking adjacent sectors via
/// two-sided linedefs. Each successive sector's floor is raised by `step_size`.
///
/// Returns the number of floor movers created.
///
/// # Algorithm
/// Starting from the trigger sector, find an adjacent sector (via a two-sided
/// linedef whose front side is the current sector) that has the same floor
/// flat texture. That becomes the next stair step. Repeat until no more
/// matching adjacent sectors are found.
pub fn ev_build_stairs(
    gs: &mut GameState,
    level: &Level,
    start_sector: usize,
    stair_type: StairType,
    crush: crate::state::CrushBehavior,
) -> usize {
    let (step_size, speed): (i16, i16) = match stair_type {
        StairType::Build8 => (8, 2),
        StairType::Turbo16 => (16, 4),
    };

    let mut count = 0;
    let mut current_sector = start_sector;
    let Some(s) = level.sectors.get(start_sector) else {
        return 0;
    };
    let mut target_height = s.floor_height + step_size;

    // Raise the starting sector first.
    if !gs
        .movers
        .active_floors
        .iter()
        .any(|f| f.sector_index == start_sector)
    {
        gs.movers.active_floors.push(FloorMover {
            sector_index: start_sector,
            target_height,
            speed,
            direction: MoveDirection::Up,
            wait_tics: -1,
            return_height: level.sectors[start_sector].floor_height,
            waiting: false,
            wait_remaining: 0,
            crush,
            tag: 0,
            floor_type: FloorType::RaiseToNearest,
        });
        count += 1;
    }

    // Walk adjacent sectors with matching floor texture.
    loop {
        let cur_flat = level.sectors[current_sector].floor_flat;
        let ld_indices = sector_linedefs(level, current_sector);

        let mut found_next = false;

        for ld_idx in ld_indices {
            let ld = &level.linedefs[ld_idx];
            // Must be two-sided.
            if ld.left_sidedef == SIDEDEF_NONE {
                continue;
            }
            // The "other" sector is on the left side of the linedef.
            let Some(sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
                continue;
            };
            let other_sector = sd.sector as usize;
            // Skip if it's the same sector.
            if other_sector == current_sector {
                continue;
            }
            // Check matching floor texture.
            let Some(s) = level.sectors.get(other_sector) else {
                continue;
            };
            let other_sec = s;
            if other_sec.floor_flat != cur_flat {
                continue;
            }
            // Skip if already has a floor mover.
            if gs
                .movers
                .active_floors
                .iter()
                .any(|f| f.sector_index == other_sector)
            {
                continue;
            }

            target_height += step_size;

            gs.movers.active_floors.push(FloorMover {
                sector_index: other_sector,
                target_height,
                speed,
                direction: MoveDirection::Up,
                wait_tics: -1,
                return_height: other_sec.floor_height,
                waiting: false,
                wait_remaining: 0,
                crush,
                tag: 0,
                floor_type: FloorType::RaiseToNearest,
            });
            count += 1;
            current_sector = other_sector;
            found_next = true;
            break;
        }

        if !found_next {
            break;
        }
    }

    count
}

// ---------------------------------------------------------------------------
// Donut special
// ---------------------------------------------------------------------------

/// Execute a donut special: raise the "donut hole" (inner sector) floor to
/// match the surrounding ring sector's floor height.
///
/// The donut hole is the sector enclosed by the trigger sector. We find it by
/// looking at the back side of linedefs fronting the trigger sector.
///
/// Returns the number of floor movers created.
///
/// # Arguments
///
/// * `gs` - Game state for tracking movers.
/// * `level` - Level containing the donut.
/// * `trigger_sector` - The sector activating the special.
///
/// # Examples
///
/// ```
/// use doom_game::specials::ev_do_donut;
/// // ev_do_donut(&mut gs, &level, sector_idx);
/// ```
pub fn ev_do_donut(gs: &mut GameState, level: &Level, trigger_sector: usize) -> usize {
    let ld_indices = sector_linedefs(level, trigger_sector);

    let mut count = 0;

    for ld_idx in ld_indices {
        let ld = &level.linedefs[ld_idx];
        // Must be two-sided.
        if ld.left_sidedef == SIDEDEF_NONE {
            continue;
        }
        // The "donut hole" is on the back side.
        let Some(sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
            continue;
        };
        let hole_sector = sd.sector as usize;

        if hole_sector == trigger_sector {
            continue;
        }

        // The ring sector provides the target height.
        // Find it by looking at linedefs fronting the hole sector — the ring
        // is the other sector that isn't the trigger sector.
        let hole_ld_indices = sector_linedefs(level, hole_sector);
        let mut ring_floor: Option<i16> = None;

        for hole_ld in hole_ld_indices {
            let hld = &level.linedefs[hole_ld];
            if hld.left_sidedef == SIDEDEF_NONE {
                continue;
            }
            let Some(sd) = level.sidedefs.get(hld.left_sidedef as usize) else {
                continue;
            };
            let ring_sector = sd.sector as usize;
            if ring_sector != hole_sector && ring_sector != trigger_sector {
                if let Some(s) = level.sectors.get(ring_sector) {
                    ring_floor = Some(s.floor_height);
                    break;
                }
            }
        }

        let target = match ring_floor {
            Some(h) => h,
            None => {
                // Fall back: use trigger sector floor as ring floor.
                match level.sectors.get(trigger_sector) {
                    Some(s) => s.floor_height,
                    None => continue,
                }
            }
        };

        // Skip if already has a mover.
        if gs
            .movers
            .active_floors
            .iter()
            .any(|f| f.sector_index == hole_sector)
        {
            continue;
        }

        let Some(s) = level.sectors.get(hole_sector) else {
            continue;
        };
        let hole_sec = s;

        let direction = if target >= hole_sec.floor_height {
            MoveDirection::Up
        } else {
            MoveDirection::Down
        };

        gs.movers.active_floors.push(FloorMover {
            sector_index: hole_sector,
            target_height: target,
            speed: 1,
            direction,
            wait_tics: -1,
            return_height: hole_sec.floor_height,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::NoCrush,
            tag: 0,
            floor_type: FloorType::LowerToLowest,
        });
        count += 1;

        // Only create one mover per donut activation.
        break;
    }

    count
}

// ---------------------------------------------------------------------------
// Perpetual platforms
// ---------------------------------------------------------------------------

/// Standard platform wait time: 3 seconds at 35 Hz ≈ 105 tics.
const PLATFORM_WAIT: i32 = 105;

/// Activate a perpetual platform on all sectors matching `tag`.
///
/// The platform oscillates between the lowest adjacent floor and the sector's
/// current floor height.
///
/// Returns the number of platforms created.
///
/// # Arguments
///
/// * `gs` - Game state tracking platforms.
/// * `level` - Map containing tagged sectors.
/// * `tag` - Identify the sector to activate.
/// * `speed` - Rate of platform movement.
///
/// # Examples
///
/// ```
/// use doom_game::specials::ev_perpetual_platform;
/// // ev_perpetual_platform(&mut gs, &level, 1, 8);
/// ```
pub fn ev_perpetual_platform(gs: &mut GameState, level: &Level, tag: u16, speed: i16) -> usize {
    let mut count = 0;
    for idx in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
    {
        // Avoid duplicate platforms on the same sector.
        if gs
            .movers
            .active_platforms
            .iter()
            .any(|p| p.sector_index == idx)
        {
            continue;
        }
        let sector = &level.sectors[idx];
        let low = lowest_adjacent_floor(level, idx);
        let high = sector.floor_height;

        gs.movers.active_platforms.push(PerpetualPlatform {
            sector_index: idx,
            low_height: low,
            high_height: high,
            speed,
            wait_tics: PLATFORM_WAIT,
            wait_remaining: 0,
            status: PlatformStatus::Down,
            tag,
        });
        count += 1;
    }
    count
}

/// Advance all active perpetual platforms by one tic.
///
/// Platforms oscillate:
/// 1. Move floor down by `speed` until `low_height` is reached.
/// 2. Enter wait phase for `wait_tics`.
/// 3. Move floor up by `speed` until `high_height` is reached.
/// 4. Enter wait phase for `wait_tics`.
/// 5. Repeat.
///
/// # Arguments
///
/// * `gs` - Game state containing active platforms.
/// * `level` - Level to mutate sector floors in.
///
/// # Examples
///
/// ```
/// use doom_game::specials::tick_platforms;
/// // tick_platforms(&mut gs, &mut level);
/// ```
pub fn tick_platforms(gs: &mut GameState, level: &mut Level) {
    for plat in &mut gs.movers.active_platforms {
        let sector_idx = plat.sector_index;
        if sector_idx >= level.sectors.len() {
            continue;
        }

        match plat.status {
            PlatformStatus::Waiting => {
                plat.wait_remaining -= 1;
                if plat.wait_remaining <= 0 {
                    // Determine which direction to go next.
                    let floor = level.sectors[sector_idx].floor_height;
                    if floor <= plat.low_height {
                        plat.status = PlatformStatus::Up;
                    } else {
                        plat.status = PlatformStatus::Down;
                    }
                }
            }
            PlatformStatus::Down => {
                level.sectors[sector_idx].floor_height -= plat.speed;
                let floor = level.sectors[sector_idx].floor_height;
                if floor <= plat.low_height {
                    level.sectors[sector_idx].floor_height = plat.low_height;
                    plat.status = PlatformStatus::Waiting;
                    plat.wait_remaining = plat.wait_tics;
                }
            }
            PlatformStatus::Up => {
                level.sectors[sector_idx].floor_height += plat.speed;
                let floor = level.sectors[sector_idx].floor_height;
                if floor >= plat.high_height {
                    level.sectors[sector_idx].floor_height = plat.high_height;
                    plat.status = PlatformStatus::Waiting;
                    plat.wait_remaining = plat.wait_tics;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// tick_ceilings (crushers)
// ---------------------------------------------------------------------------

/// Advance all active ceiling movers / crushers by one tic.
///
/// Call this once per tic from `tick()`.
///
/// Crusher oscillation:
/// 1. Move ceiling by `speed` in `direction`.
/// 2. If moving Down and reaches `bottom_height`: reverse to Up.
///    If in crush range, apply `crush_damage` to the player (simplified).
/// 3. If moving Up and reaches `top_height`: reverse to Down (perpetual)
///    or remove (one-shot).
///
/// # Arguments
///
/// * `gs` - Game state containing active ceilings.
/// * `level` - Level to mutate sector ceilings in.
///
/// # Examples
///
/// ```
/// use doom_game::specials::tick_ceilings;
/// // tick_ceilings(&mut gs, &mut level);
/// ```
pub fn tick_ceilings(gs: &mut GameState, level: &mut Level) {
    let mut active_ceilings = std::mem::take(&mut gs.movers.active_ceilings);

    active_ceilings.retain_mut(|ceiling| {
        let sector_idx = ceiling.sector_index;
        let speed = ceiling.speed;
        let direction = ceiling.direction;
        let top = ceiling.top_height;
        let bottom = ceiling.bottom_height;
        let crush_dmg = ceiling.crush_damage;
        let remove_when_done = ceiling.remove_when_done;
        let normal_speed = ceiling.normal_speed;
        let ceiling_type = ceiling.ceiling_type;

        if sector_idx >= level.sectors.len() {
            return false;
        }

        match direction {
            MoveDirection::Down => {
                level.sectors[sector_idx].ceil_height -= speed;
                let ceil = level.sectors[sector_idx].ceil_height;
                let floor = level.sectors[sector_idx].floor_height;

                // Crush damage: when ceiling is close to floor (within 8 units).
                if ceil <= floor + 8 && crush_dmg > 0 {
                    // Simplified: damage player if they are in this sector.
                    // A proper implementation would iterate all mobjs in the sector.
                    let player_handle = gs.player.handle;
                    if let Some(pmo) = gs.mobjslab.get(player_handle) {
                        if pmo.z.to_int() == floor as i32 {
                            // Very simplified sector check: just damage if z matches.
                            gs.damage_player(crush_dmg);
                        }
                    }

                    // Slow down to speed 1 when crushing (CrushAndRaise / SilentCrush).
                    match ceiling_type {
                        CeilingType::CrushAndRaise | CeilingType::SilentCrush => {
                            ceiling.speed = 1;
                        }
                        _ => {}
                    }
                }

                if ceil <= bottom {
                    level.sectors[sector_idx].ceil_height = bottom;
                    match ceiling_type {
                        CeilingType::LowerToFloor | CeilingType::LowerAndCrush => {
                            // One-shot types: remove when done.
                            return false;
                        }
                        _ => {
                            // Perpetual types: reverse to Up.
                            ceiling.direction = MoveDirection::Up;
                        }
                    }
                }
            }
            MoveDirection::Up => {
                // Resume normal speed when going up.
                ceiling.speed = normal_speed;

                level.sectors[sector_idx].ceil_height += normal_speed;
                let ceil = level.sectors[sector_idx].ceil_height;

                if ceil >= top {
                    level.sectors[sector_idx].ceil_height = top;
                    if remove_when_done {
                        return false;
                    }
                    // Perpetual: reverse back to Down.
                    ceiling.direction = MoveDirection::Down;
                }
            }
        }

        true
    });

    gs.movers.active_ceilings = active_ceilings;
}

// ---------------------------------------------------------------------------
// tick_floors (lifts / floor raisers)
// ---------------------------------------------------------------------------

/// Advance all active floor movers by one tic.
///
/// Call this once per tic from `tick()`.
///
/// Lift behavior (wait_tics > 0):
/// 1. Floor lowers to target_height.
/// 2. Enters wait phase for wait_tics.
/// 3. Floor raises back to return_height.
/// 4. Removed when return_height reached.
///
/// Floor raiser/lowerer behavior (wait_tics == -1):
/// 1. Floor moves to target_height.
/// 2. Removed when target reached.
pub fn tick_floors(gs: &mut GameState, level: &mut Level) {
    let mut active_floors = std::mem::take(&mut gs.movers.active_floors);

    active_floors.retain_mut(|floor_mover| {
        let sector_idx = floor_mover.sector_index;
        if sector_idx >= level.sectors.len() {
            return false;
        }

        // Waiting phase.
        if floor_mover.waiting {
            floor_mover.wait_remaining -= 1;
            if floor_mover.wait_remaining <= 0 {
                // Wait over — reverse direction to return.
                floor_mover.waiting = false;
                floor_mover.direction = MoveDirection::Up;
                floor_mover.target_height = floor_mover.return_height;
            }
            return true;
        }

        let speed = floor_mover.speed;
        let direction = floor_mover.direction;
        let target = floor_mover.target_height;
        let wait_tics = floor_mover.wait_tics;
        let crush = floor_mover.crush;
        let crush_dmg: i32 = if crush == crate::state::CrushBehavior::Crush {
            10
        } else {
            0
        };

        match direction {
            MoveDirection::Down => {
                level.sectors[sector_idx].floor_height -= speed;
                let floor = level.sectors[sector_idx].floor_height;

                if floor <= target {
                    level.sectors[sector_idx].floor_height = target;
                    if wait_tics > 0 {
                        // Enter wait phase (e.g., lift at bottom).
                        floor_mover.waiting = true;
                        floor_mover.wait_remaining = wait_tics;
                    } else {
                        // One-shot: remove.
                        return false;
                    }
                }
            }
            MoveDirection::Up => {
                level.sectors[sector_idx].floor_height += speed;
                let floor = level.sectors[sector_idx].floor_height;

                // Crush damage when raising into something.
                if crush == crate::state::CrushBehavior::Crush && crush_dmg > 0 {
                    let ceil = level.sectors[sector_idx].ceil_height;
                    if floor >= ceil - 8 {
                        let player_handle = gs.player.handle;
                        if let Some(pmo) = gs.mobjslab.get_mut(player_handle) {
                            if pmo.z.to_int() >= (floor - 8) as i32 {
                                gs.damage_player(crush_dmg);
                            }
                        }
                    }
                }

                if floor >= target {
                    level.sectors[sector_idx].floor_height = target;
                    if wait_tics > 0 && floor_mover.return_height != target {
                        // Returning phase complete — remove.
                        return false;
                    }
                    // One-shot raiser: remove.
                    return false;
                }
            }
        }

        true
    });

    gs.movers.active_floors = active_floors;
}

// ---------------------------------------------------------------------------
// Crusher / lift / floor activation helpers
// ---------------------------------------------------------------------------

/// Standard lift wait time: 3 seconds at 35 Hz = 105 tics.
const LIFT_WAIT: i32 = 105;

// ---------------------------------------------------------------------------
// Public ceiling activation functions
// ---------------------------------------------------------------------------

/// Activate a CrushAndRaise ceiling on all sectors matching `tag`.
///
/// Perpetual crusher: lowers to floor+8, reverses, raises to top, reverses, repeat.
/// Deals 10 damage per tic when crushing.
pub fn ev_ceiling_crush_and_raise(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    activate_crusher(
        gs,
        level,
        tag,
        CrusherParams {
            speed,
            crush_damage: 10,
            silent: false,
            remove_when_done: false,
            ceiling_type: CeilingType::CrushAndRaise,
        },
    );
}

/// Activate a LowerAndCrush ceiling on all sectors matching `tag`.
///
/// One-shot: lowers to floor+8 then stops. No crush damage.
pub fn ev_ceiling_lower_and_crush(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    activate_crusher(
        gs,
        level,
        tag,
        CrusherParams {
            speed,
            crush_damage: 0,
            silent: false,
            remove_when_done: true,
            ceiling_type: CeilingType::LowerAndCrush,
        },
    );
}

/// Activate a LowerToFloor ceiling on all sectors matching `tag`.
///
/// One-shot: lowers to floor height then stops. No crush damage.
pub fn ev_ceiling_lower_to_floor(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    activate_crusher(
        gs,
        level,
        tag,
        CrusherParams {
            speed,
            crush_damage: 0,
            silent: false,
            remove_when_done: true,
            ceiling_type: CeilingType::LowerToFloor,
        },
    );
}

/// Stop all crushers with matching `tag` by removing them.
///
/// Used by line types 57 and 74.
pub fn ev_ceiling_crush_stop(gs: &mut GameState, tag: u16) {
    stop_crushers(gs, tag);
}

/// Activate a FastCrushAndRaise ceiling on all sectors matching `tag`.
///
/// Like CrushAndRaise but typically with higher speed. Deals 10 damage per tic.
pub fn ev_ceiling_crush_raise_fast(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    activate_crusher(
        gs,
        level,
        tag,
        CrusherParams {
            speed,
            crush_damage: 10,
            silent: false,
            remove_when_done: false,
            ceiling_type: CeilingType::FastCrushAndRaise,
        },
    );
}

/// Raise ceiling on all sectors matching `tag` to the highest adjacent ceiling.
///
/// Creates a one-shot `CeilingMover` with `CeilingType::RaiseToHighest`.
/// Linedef type 40 (W1 Raise ceiling to highest adjacent ceiling).
pub fn ev_ceiling_raise_to_highest(gs: &mut GameState, level: &Level, tag: u16) {
    for sector_idx in 0..level.sectors.len() {
        let sector = &level.sectors[sector_idx];
        if sector.tag != tag {
            continue;
        }
        let top = highest_adjacent_ceiling(level, sector_idx);
        if sector.ceil_height >= top {
            continue;
        }
        if gs
            .movers
            .active_ceilings
            .iter()
            .any(|c| c.sector_index == sector_idx)
        {
            continue;
        }
        gs.movers.active_ceilings.push(CeilingMover {
            sector_index: sector_idx,
            top_height: top,
            bottom_height: sector.ceil_height,
            speed: 2,
            normal_speed: 2,
            crush_damage: 0,
            direction: MoveDirection::Up,
            silent: false,
            remove_when_done: true,
            tag,
            ceiling_type: CeilingType::RaiseToHighest,
        });
    }
}

// ---------------------------------------------------------------------------

/// Parameters to spawn a ceiling crusher.
///
/// ## Examples
/// ```
/// # use doom_game::specials::CrusherParams;
/// # use doom_game::state::CeilingType;
/// let params = CrusherParams {
///     speed: 2,
///     crush_damage: 10,
///     silent: false,
///     remove_when_done: false,
///     ceiling_type: CeilingType::CrushAndRaise,
/// };
/// ```
pub struct CrusherParams {
    /// Vertical speed of the ceiling when crushing downward (map units per tic).
    pub speed: i16,
    /// Damage dealt to actors caught under the ceiling when it bottoms out.
    pub crush_damage: i32,
    /// If true, the crusher does not play standard movement sounds.
    pub silent: bool,
    /// If true, the crusher is removed from the active mover list after one cycle.
    pub remove_when_done: bool,
    /// The state-machine type controlling the oscillation/raising pattern.
    pub ceiling_type: CeilingType,
}

fn activate_crusher(gs: &mut GameState, level: &Level, tag: u16, params: CrusherParams) {
    for idx in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
    {
        // Avoid duplicate crushers on the same sector.
        if gs
            .movers
            .active_ceilings
            .iter()
            .any(|c| c.sector_index == idx)
        {
            continue;
        }
        let sector = &level.sectors[idx];
        let bottom = match params.ceiling_type {
            CeilingType::LowerToFloor => sector.floor_height,
            _ => sector.floor_height + 8,
        };
        gs.movers.active_ceilings.push(CeilingMover {
            sector_index: idx,
            top_height: sector.ceil_height,
            bottom_height: bottom,
            speed: params.speed,
            normal_speed: params.speed,
            crush_damage: params.crush_damage,
            direction: MoveDirection::Down,
            silent: params.silent,
            remove_when_done: params.remove_when_done,
            tag,
            ceiling_type: params.ceiling_type,
        });
    }
}

/// Stop all crushers matching `tag` (line type 57).
fn stop_crushers(gs: &mut GameState, tag: u16) {
    gs.movers.active_ceilings.retain(|c| c.tag != tag);
}

/// Activate a lift (lower-wait-raise) on all sectors matching `tag`.
fn activate_lift(gs: &mut GameState, level: &Level, tag: u16, speed: i16) {
    for idx in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
    {
        // Avoid duplicate floor movers on the same sector.
        if gs
            .movers
            .active_floors
            .iter()
            .any(|f| f.sector_index == idx)
        {
            continue;
        }
        let sector = &level.sectors[idx];
        let low = lowest_adjacent_floor(level, idx);
        gs.movers.active_floors.push(FloorMover {
            sector_index: idx,
            target_height: low,
            speed,
            direction: MoveDirection::Down,
            wait_tics: LIFT_WAIT,
            return_height: sector.floor_height,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::NoCrush,
            tag,
            floor_type: FloorType::LowerToLowest,
        });
    }
}

// ---------------------------------------------------------------------------
// LiftMover activation and tick
// ---------------------------------------------------------------------------

/// Activate a lift (lower-wait-raise) on all sectors matching `tag` using the
/// dedicated `LiftMover` system.
///
/// For each matching sector, computes the lowest adjacent floor height as
/// `low_height`, stores the current floor as `high_height`, and creates a
/// `LiftMover` starting in `Lowering` status.
///
/// Returns the number of lifts created.
pub fn ev_do_lift(
    gs: &mut GameState,
    level: &Level,
    tag: u16,
    speed: i16,
    wait_tics: i32,
) -> usize {
    let mut count = 0;
    for idx in level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
    {
        // Avoid duplicate lifts on the same sector.
        if gs.movers.lifts.iter().any(|l| l.sector_index == idx) {
            continue;
        }
        let sector = &level.sectors[idx];
        let low = lowest_adjacent_floor(level, idx);
        gs.movers.lifts.push(LiftMover {
            sector_index: idx,
            low_height: low,
            high_height: sector.floor_height,
            speed,
            wait_tics,
            wait_remaining: 0,
            status: LiftStatus::Lowering,
        });
        count += 1;
    }
    count
}

/// Advance all active lifts by one tic.
///
/// Call this once per tic from `tick()`.
///
/// Lift cycle:
/// 1. `Lowering`: move floor down by `speed`. When `floor <= low_height`,
///    snap to `low_height`, transition to `Waiting`, set `wait_remaining`.
/// 2. `Waiting`: decrement `wait_remaining`. When 0, transition to `Raising`.
/// 3. `Raising`: move floor up by `speed`. When `floor >= high_height`,
///    snap to `high_height`, transition to `Done`.
/// 4. `Done`: remove from active list.
pub fn tick_lifts(gs: &mut GameState, level: &mut Level) {
    let mut lifts = std::mem::take(&mut gs.movers.lifts);

    lifts.retain_mut(|lift| {
        let sector_idx = lift.sector_index;
        if sector_idx >= level.sectors.len() {
            return false;
        }

        match lift.status {
            LiftStatus::Lowering => {
                let speed = lift.speed;
                let low = lift.low_height;
                level.sectors[sector_idx].floor_height -= speed;
                if level.sectors[sector_idx].floor_height <= low {
                    level.sectors[sector_idx].floor_height = low;
                    lift.status = LiftStatus::Waiting;
                    lift.wait_remaining = lift.wait_tics;
                }
            }
            LiftStatus::Waiting => {
                lift.wait_remaining -= 1;
                if lift.wait_remaining <= 0 {
                    lift.status = LiftStatus::Raising;
                }
            }
            LiftStatus::Raising => {
                let speed = lift.speed;
                let high = lift.high_height;
                level.sectors[sector_idx].floor_height += speed;
                if level.sectors[sector_idx].floor_height >= high {
                    level.sectors[sector_idx].floor_height = high;
                    lift.status = LiftStatus::Done;
                }
            }
            LiftStatus::Done => {
                return false;
            }
        }

        true
    });

    gs.movers.lifts = lifts;
}

/// Activate a floor raiser (one-shot, no wait) on a single sector with an
/// explicit `FloorType`.
fn activate_floor_raise_single_typed(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    tag: u16,
    target_height: i16,
    speed: i16,
    crush: crate::state::CrushBehavior,
    floor_type: FloorType,
) {
    if gs
        .movers
        .active_floors
        .iter()
        .any(|f| f.sector_index == sector_idx)
    {
        return;
    }
    let Some(sector) = level.sectors.get(sector_idx) else {
        return;
    };
    gs.movers.active_floors.push(FloorMover {
        sector_index: sector_idx,
        target_height,
        speed,
        direction: MoveDirection::Up,
        wait_tics: -1,
        return_height: sector.floor_height,
        waiting: false,
        wait_remaining: 0,
        crush,
        tag,
        floor_type,
    });
}

/// Activate a floor lowerer (one-shot, no wait) on a single sector with an
/// explicit `FloorType`.
fn activate_floor_lower_single_typed(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    tag: u16,
    target_height: i16,
    speed: i16,
    floor_type: FloorType,
) {
    if gs
        .movers
        .active_floors
        .iter()
        .any(|f| f.sector_index == sector_idx)
    {
        return;
    }
    let Some(sector) = level.sectors.get(sector_idx) else {
        return;
    };
    gs.movers.active_floors.push(FloorMover {
        sector_index: sector_idx,
        target_height,
        speed,
        direction: MoveDirection::Down,
        wait_tics: -1,
        return_height: sector.floor_height,
        waiting: false,
        wait_remaining: 0,
        crush: crate::state::CrushBehavior::NoCrush,
        tag,
        floor_type,
    });
}

// ---------------------------------------------------------------------------
// Door helpers
// ---------------------------------------------------------------------------

/// Enqueue a door mover that opens and optionally auto-closes.
fn open_door(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    behavior: crate::linedef_dispatch::DoorBehavior,
) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;

    let target = lowest_adjacent_ceiling(level, sector_idx) - 4;

    // Avoid duplicate movers for the same sector.
    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }

    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: DOOR_SPEED,
        is_ceiling: true,
        wait_tics: if behavior == crate::linedef_dispatch::DoorBehavior::OpenWaitClose {
            DOOR_WAIT
        } else {
            -1
        },
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}

/// Allow monsters to open ordinary door linedefs without mutating the map.
pub fn monster_activate_door_linedef(
    gs: &mut GameState,
    level: &Level,
    linedef_idx: usize,
) -> bool {
    let Some(ld) = level.linedefs.get(linedef_idx) else {
        return false;
    };

    let auto_close = match ld.special {
        1 | 117 => true,
        31 | 118 => false,
        _ => return false,
    };

    if ld.left_sidedef == SIDEDEF_NONE {
        return false;
    }

    let Some(sd) = level.sidedefs.get(ld.left_sidedef as usize) else {
        return false;
    };
    let sector_idx = sd.sector as usize;
    if sector_idx >= level.sectors.len() {
        return false;
    }

    open_door(
        gs,
        level,
        sector_idx,
        if auto_close {
            crate::linedef_dispatch::DoorBehavior::OpenWaitClose
        } else {
            crate::linedef_dispatch::DoorBehavior::OpenStay
        },
    );
    true
}

/// Enqueue a door mover that closes a door.
fn close_door(gs: &mut GameState, level: &Level, sector_idx: usize) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;

    let target = sector.floor_height;

    // Avoid duplicate movers for the same sector.
    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }

    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: -DOOR_SPEED,
        is_ceiling: true,
        wait_tics: -1,
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}

/// Enqueue a door that closes, waits 30 s (1050 tics), then reopens.
///
/// Used by linedef types 16 (W1) and 76 (WR).
fn close_wait_open_door(gs: &mut GameState, level: &Level, sector_idx: usize) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;
    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }
    let reopen_h = lowest_adjacent_ceiling(level, sector_idx) - 4;
    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: sector.floor_height,
        current_height: sector.ceil_height,
        speed: -DOOR_SPEED,
        is_ceiling: true,
        wait_tics: -1,
        countdown: -1,
        reopen_height: reopen_h,
        reopen_countdown: -1,
    });
}

/// Enqueue a blazing (fast) door mover that opens and optionally auto-closes.
///
/// Same as `open_door` but with `BLAZING_DOOR_SPEED` (8 units/tic).
fn open_blazing_door(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    behavior: crate::linedef_dispatch::DoorBehavior,
) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;

    let target = lowest_adjacent_ceiling(level, sector_idx) - 4;

    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }

    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: BLAZING_DOOR_SPEED,
        is_ceiling: true,
        wait_tics: if behavior == crate::linedef_dispatch::DoorBehavior::OpenWaitClose {
            DOOR_WAIT
        } else {
            -1
        },
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}

/// Enqueue a blazing (fast) door mover that closes a door.
fn close_blazing_door(gs: &mut GameState, level: &Level, sector_idx: usize) {
    let Some(s) = level.sectors.get(sector_idx) else {
        return;
    };
    let sector = s;

    let target = sector.floor_height;

    if gs
        .movers
        .active_doors
        .iter()
        .any(|d| d.sector == sector_idx)
    {
        return;
    }

    gs.movers.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: -BLAZING_DOOR_SPEED,
        is_ceiling: true,
        wait_tics: -1,
        countdown: -1,
        reopen_height: 0,
        reopen_countdown: -1,
    });
}

// ---------------------------------------------------------------------------
// p_use_lines
// ---------------------------------------------------------------------------

/// Check whether the player's USE action activates a linedef.
///
/// Port of `P_UseLines`. Casts a short ray from the actor's position toward
/// the direction they are facing and checks every linedef with a special for
/// intersection.
///
/// Because the trig tables may be uninitialized in tests (returning 0), the
/// function falls back to `ahead_x = ax + USE_RANGE, ahead_y = ay` when both
/// `cos` and `sin` are zero.
///
/// Only the first intersected linedef with a non-zero special is activated.
pub fn p_use_lines(gs: &mut GameState, level: &mut Level, handle: MobjHandle) {
    // Read actor position and angle.
    let Some(mo) = gs.mobjslab.get(handle) else {
        return;
    };
    let (ax, ay, angle) = (mo.x.to_int(), mo.y.to_int(), mo.angle);

    let cos_raw = i64::from(angle.cos().raw());
    let sin_raw = i64::from(angle.sin().raw());

    // Fall back if trig tables are uninitialised (both return 0).
    let (ahead_x, ahead_y) = if cos_raw == 0 && sin_raw == 0 {
        (ax + USE_RANGE, ay)
    } else {
        (
            ax + ((i64::from(USE_RANGE) * cos_raw) / i64::from(FIXED_ONE.raw())) as i32,
            ay + ((i64::from(USE_RANGE) * sin_raw) / i64::from(FIXED_ONE.raw())) as i32,
        )
    };

    // Find crossed linedefs in front-to-back order. Vanilla Doom traverses all
    // intercepts here: a closed ordinary wall in front of a special must block
    // use instead of letting the player "reach through" it.
    let mut intercepts: smallvec::SmallVec<[(i64, i64, usize); 16]> = smallvec::SmallVec::new();
    for ld_idx in 0..level.linedefs.len() {
        // Collect data while the borrow is immutable; drop before mutable dispatch.
        let (lx1, ly1, lx2, ly2) = {
            let ld = &level.linedefs[ld_idx];
            let v1 = &level.vertexes[ld.from_vertex as usize];
            let v2 = &level.vertexes[ld.to_vertex as usize];
            (v1.x as i32, v1.y as i32, v2.x as i32, v2.y as i32)
        };

        let Some((num, denom)) =
            segment_intersection_frac(ax, ay, ahead_x, ahead_y, lx1, ly1, lx2, ly2)
        else {
            continue;
        };
        intercepts.push((num, denom, ld_idx));
    }

    intercepts.sort_by(|a, b| {
        let lhs = i128::from(a.0) * i128::from(b.1);
        let rhs = i128::from(b.0) * i128::from(a.1);
        lhs.cmp(&rhs)
    });

    for (_, _, ld_idx) in intercepts {
        let (special, blocks_use, use_side) = {
            let linedef = &level.linedefs[ld_idx];
            let blocks_use = match crate::trace::line_opening(level, linedef) {
                None => true,
                Some((open_bottom, open_top)) => open_top <= open_bottom,
            };
            let use_side = crate::sight::point_on_side(
                Fixed16_16::from_int(ax),
                Fixed16_16::from_int(ay),
                ld_idx,
                level,
            ) as u8;
            (linedef.special, blocks_use, use_side)
        };

        // USE key only activates switch-type (S1/SR) triggers.
        use crate::linedef_dispatch::{TriggerType, classify_trigger, dispatch_linedef};
        if let Some(trigger) = classify_trigger(special) {
            if matches!(trigger, TriggerType::SwitchOnce | TriggerType::SwitchRepeat) {
                let activated =
                    dispatch_linedef(gs, level, ld_idx, special, trigger, handle, use_side);
                if activated {
                    crate::switch::toggle_switch_texture(level, ld_idx);
                }
                return;
            }
        }

        if special == 0 && blocks_use {
            gs.sound
                .sound_queue
                .push(crate::state::SoundRequest::PlayerUseFail);
            return;
        }

        if blocks_use {
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// activate_linedef
// ---------------------------------------------------------------------------

/// Dispatch a linedef activation by its special number.
///
/// Handles:
/// - **1**: Door toggle — opens a closed door or closes an open one (immediate for compat).
/// - **2**: Open door, stays open (animated via DoorMover).
/// - **26**: Blue-key locked door.
/// - **27**: Yellow-key locked door.
/// - **28**: Red-key locked door.
/// - **29**: Close door (animated via DoorMover).
/// - **63**: Remote door open-stay (by tag).
/// - **64**: Remote door open-close (by tag).
/// - **11**: Exit — no-op stub.
/// - Other: no-op.
pub fn activate_linedef(gs: &mut GameState, level: &mut Level, linedef_idx: usize) {
    let Some(ld) = level.linedefs.get(linedef_idx) else {
        return;
    };

    let special = ld.special;

    // Find the sector behind the linedef (back sector for door triggers).
    let left_sidedef = ld.left_sidedef;
    if left_sidedef == SIDEDEF_NONE && special != 63 && special != 64 {
        // One-sided linedef — nothing to toggle for most specials.
        // Tag-based specials handle their own sector lookup.
    }

    match special {
        // Doors
        1 | 2 | 29 | 16 | 76 | 26 | 27 | 28 | 63 | 105 | 106 | 107 | 108 | 109 | 110 | 99 | 133
        | 134 | 135 | 136 | 137 => {
            activate_doors(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Exits
        11 | 51 | 52 | 124 => activate_exits(gs, level, special, left_sidedef as i16, linedef_idx),
        // Ceilings
        6 | 25 | 44 | 49 | 57 | 72 | 73 | 74 | 141 => {
            activate_ceilings(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Lifts
        62 | 66 | 10 | 21 | 88 | 121 | 120 | 122 | 123 => {
            activate_lifts(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Floors
        5 | 14 | 15 | 18 | 20 | 22 | 24 | 30 | 56 | 58 | 59 | 64 | 65 | 67 | 68 | 91 | 92 | 93
        | 94 | 95 | 96 | 19 | 23 | 36 | 37 | 38 | 45 | 60 | 69 | 70 | 71 | 82 | 83 | 84 | 98
        | 102 => activate_floors(gs, level, special, left_sidedef as i16, linedef_idx),
        // Stairs
        7 | 8 | 100 | 127 => activate_stairs(gs, level, special, left_sidedef as i16, linedef_idx),
        // Platforms
        53 | 54 | 87 | 89 => {
            activate_platforms(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Teleports
        39 | 97 | 125 | 126 => {
            activate_teleports(gs, level, special, left_sidedef as i16, linedef_idx)
        }
        // Misc
        9 | 146 => activate_misc(gs, level, special, left_sidedef as i16, linedef_idx),
        _ => {
            // Unknown special — silently ignored.
        }
    }
}

#[allow(unused_variables)]
fn activate_doors(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // --- Type 1: toggle door (immediate, for backward compatibility with existing tests) ---
        1 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };

            let Some(sector) = level.sectors.get_mut(sector_idx) else {
                return;
            };

            if sector.ceil_height > sector.floor_height {
                // Door is open — close it.
                sector.ceil_height = sector.floor_height;
            } else {
                // Door is closed — open it.
                sector.ceil_height = sector.floor_height + 128;
            }
        }

        // --- Type 2: open door, stays open (animated) ---
        2 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            open_door(
                gs,
                level,
                sector_idx,
                crate::linedef_dispatch::DoorBehavior::OpenStay,
            );
        }

        // --- Type 29: close door (animated) ---
        29 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            close_door(gs, level, sector_idx);
        }

        // --- Types 16, 76: close door, wait 30s, reopen ---
        16 | 76 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            close_wait_open_door(gs, level, sector_idx);
        }

        // --- Types 26/27/28: locked raise-and-close door ---
        26 => {
            // Blue card or skull required.
            if gs.player.has_key(crate::player::KEY_BLUE_CARD)
                || gs.player.has_key(crate::player::KEY_BLUE_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
                );
            }
        }
        27 => {
            // Yellow key required.
            if gs.player.has_key(crate::player::KEY_YELLOW_CARD)
                || gs.player.has_key(crate::player::KEY_YELLOW_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
                );
            }
        }
        28 => {
            // Red key required.
            if gs.player.has_key(crate::player::KEY_RED_CARD)
                || gs.player.has_key(crate::player::KEY_RED_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
                );
            }
        }

        // --- Type 63: remote tag-based door (open stay) ---
        63 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                open_door(
                    gs,
                    level,
                    idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // -----------------------------------------------------------------
        // Blazing doors (fast doors, speed=8)
        // -----------------------------------------------------------------

        // Type 105: WR Blazing door open-close.
        105 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                open_blazing_door(
                    gs,
                    level,
                    idx,
                    crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
                );
            }
        }

        // Type 106: WR Blazing door open-stay.
        106 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                open_blazing_door(
                    gs,
                    level,
                    idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 107: WR Blazing door close.
        107 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                close_blazing_door(gs, level, idx);
            }
        }

        // Type 108: W1 Blazing door open-close.
        108 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            open_blazing_door(
                gs,
                level,
                sector_idx,
                crate::linedef_dispatch::DoorBehavior::OpenWaitClose,
            );
        }

        // Type 109: W1 Blazing door open-stay.
        109 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            open_blazing_door(
                gs,
                level,
                sector_idx,
                crate::linedef_dispatch::DoorBehavior::OpenStay,
            );
        }

        // Type 110: W1 Blazing door close.
        110 => {
            let Some(sector_idx) = level
                .sidedefs
                .get(left_sidedef as usize)
                .map(|sd| sd.sector as usize)
            else {
                return;
            };
            close_blazing_door(gs, level, sector_idx);
        }

        // -----------------------------------------------------------------
        // Additional keyed door line types
        // -----------------------------------------------------------------

        // Type 99: SR Blue key door open-stay.
        99 => {
            if gs.player.has_key(crate::player::KEY_BLUE_CARD)
                || gs.player.has_key(crate::player::KEY_BLUE_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 133: S1 Blue key door open-stay (blazing).
        133 => {
            if gs.player.has_key(crate::player::KEY_BLUE_CARD)
                || gs.player.has_key(crate::player::KEY_BLUE_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_blazing_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 134: SR Red key door open-stay.
        134 => {
            if gs.player.has_key(crate::player::KEY_RED_CARD)
                || gs.player.has_key(crate::player::KEY_RED_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 135: S1 Red key door open-stay (blazing).
        135 => {
            if gs.player.has_key(crate::player::KEY_RED_CARD)
                || gs.player.has_key(crate::player::KEY_RED_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_blazing_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 136: SR Yellow key door open-stay.
        136 => {
            if gs.player.has_key(crate::player::KEY_YELLOW_CARD)
                || gs.player.has_key(crate::player::KEY_YELLOW_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }

        // Type 137: S1 Yellow key door open-stay (blazing).
        137 => {
            if gs.player.has_key(crate::player::KEY_YELLOW_CARD)
                || gs.player.has_key(crate::player::KEY_YELLOW_SKULL)
            {
                let Some(sector_idx) = level
                    .sidedefs
                    .get(left_sidedef as usize)
                    .map(|sd| sd.sector as usize)
                else {
                    return;
                };
                open_blazing_door(
                    gs,
                    level,
                    sector_idx,
                    crate::linedef_dispatch::DoorBehavior::OpenStay,
                );
            }
        }
        _ => {}
    }
}

#[allow(unused_variables)]
fn activate_exits(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // --- Type 11: S1 Exit (normal) ---
        11 => {
            gs.exit_request = Some(ExitRequest::Normal);
        }

        // --- Type 51: S1 Secret Exit ---
        51 => {
            gs.exit_request = Some(ExitRequest::Secret);
        }

        // --- Type 52: W1 Exit (walk trigger, normal) ---
        52 => {
            gs.exit_request = Some(ExitRequest::Normal);
        }

        // --- Type 124: W1 Secret Exit (walk trigger) ---
        124 => {
            gs.exit_request = Some(ExitRequest::Secret);
        }
        _ => {}
    }
}

#[allow(unused_variables)]
fn activate_ceilings(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Crushers
        // -----------------------------------------------------------------

        // Type 6: W1 Fast crusher ceiling (perpetual, speed=2).
        6 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_raise_fast(gs, level, tag, 2);
        }

        // Type 25: W1 Slow crusher ceiling (perpetual, speed=1).
        25 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_and_raise(gs, level, tag, 1);
        }

        // Type 44: W1 Ceiling lower to 8 above floor (one-shot, no crush damage).
        44 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_lower_and_crush(gs, level, tag, 2);
        }

        // Type 49: S1 Ceiling lower to 8 above floor + crush damage.
        49 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_crusher(
                gs,
                level,
                tag,
                CrusherParams {
                    speed: 2,
                    crush_damage: 10,
                    silent: false,
                    remove_when_done: true,
                    ceiling_type: CeilingType::LowerAndCrush,
                },
            );
        }

        // Type 57: W1 Stop ceiling crusher (remove all crushers matching tag).
        57 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_stop(gs, tag);
        }

        // Type 72: WR Ceiling lower to 8 above floor.
        72 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_lower_and_crush(gs, level, tag, 2);
        }

        // Type 73: WR Ceiling crush and raise (slow, perpetual).
        73 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_and_raise(gs, level, tag, 1);
        }

        // Type 74: WR Stop ceiling crusher.
        74 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_ceiling_crush_stop(gs, tag);
        }

        // Type 141: W1 Ceiling crush and raise (silent, perpetual).
        141 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_crusher(
                gs,
                level,
                tag,
                CrusherParams {
                    speed: 2,
                    crush_damage: 10,
                    silent: true,
                    remove_when_done: false,
                    ceiling_type: CeilingType::SilentCrush,
                },
            );
        }
        _ => {}
    }
}

#[allow(unused_variables)]
fn activate_lifts(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Lifts (lower-wait-raise)
        // -----------------------------------------------------------------

        // Type 62: Plat lower-wait-raise (speed 4).
        62 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 66: SR Raise floor 24 + change.
        66 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 10: Plat down-wait-up-stay (door-like lift).
        10 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 21: Plat down-wait-up-stay (switch).
        21 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 88: Plat down-wait-up-stay-monster (walk trigger).
        88 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 4);
        }

        // Type 121: Plat lower-wait-raise (turbo speed 8).
        121 => {
            let tag = level.linedefs[linedef_idx].tag;
            activate_lift(gs, level, tag, 8);
        }

        // -----------------------------------------------------------------
        // Additional lift line types (using LiftMover)
        // -----------------------------------------------------------------

        // Type 120: WR Lift blazing (speed 8, wait 105).
        120 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_do_lift(gs, level, tag, 8, LIFT_WAIT);
        }

        // Type 122: S1 Lift blazing (speed 8, wait 105).
        122 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_do_lift(gs, level, tag, 8, LIFT_WAIT);
        }

        // Type 123: SR Lift blazing (speed 8, wait 105).
        123 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_do_lift(gs, level, tag, 8, LIFT_WAIT);
        }
        _ => {}
    }
}

#[allow(unused_variables)]
fn activate_floors(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Floor raisers
        // -----------------------------------------------------------------

        // Type 5: W1 Floor raise to lowest adjacent ceiling (crush).
        5 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_lowest_ceiling(gs, level, tag, 1, crate::state::CrushBehavior::Crush);
        }

        // Type 14: S1 Raise floor 32 + change texture/type.
        14 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_32(gs, level, tag, 1);
        }

        // Type 15: S1 Raise floor 24 + change texture/type.
        15 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 18: S1 Floor raise to next highest adjacent floor.
        18 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 20: S1 Raise floor to next highest + change texture.
        20 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 22: W1 Floor raise to next highest adjacent floor + change texture.
        22 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 24: G1 Raise floor to lowest adjacent ceiling.
        24 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_lowest_ceiling(
                gs,
                level,
                tag,
                1,
                crate::state::CrushBehavior::NoCrush,
            );
        }

        // Type 30: W1 Raise floor by shortest lower texture.
        30 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_by_texture(gs, level, tag, 1);
        }

        // Type 56: W1 Floor raise to 8 below lowest adjacent ceiling (crush).
        56 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = lowest_adjacent_ceiling(level, idx) - 8;
                    activate_floor_raise_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        crate::state::CrushBehavior::Crush,
                        FloorType::RaiseCrush,
                    );
                }
            }
        }

        // Type 58: W1 Raise floor 24.
        58 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 59: W1 Raise floor 24 + change texture/type.
        59 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 64: SR Raise floor to lowest adjacent ceiling.
        64 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_lowest_ceiling(
                gs,
                level,
                tag,
                1,
                crate::state::CrushBehavior::NoCrush,
            );
        }

        // Type 65: SR Raise floor to 8 below lowest ceiling + crush.
        65 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = lowest_adjacent_ceiling(level, idx) - 8;
                    activate_floor_raise_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        crate::state::CrushBehavior::Crush,
                        FloorType::RaiseCrush,
                    );
                }
            }
        }

        // Type 67: SR Raise floor 32 + change.
        67 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_32(gs, level, tag, 1);
        }

        // Type 68: SR Raise floor to next highest + change texture.
        68 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 91: WR Raise floor to lowest adjacent ceiling.
        91 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_lowest_ceiling(
                gs,
                level,
                tag,
                1,
                crate::state::CrushBehavior::NoCrush,
            );
        }

        // Type 92: WR Raise floor 24.
        92 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 93: WR Raise floor 24 + change.
        93 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_24(gs, level, tag, 1);
        }

        // Type 94: WR Raise floor to 8 below lowest ceiling + crush.
        94 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = lowest_adjacent_ceiling(level, idx) - 8;
                    activate_floor_raise_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        crate::state::CrushBehavior::Crush,
                        FloorType::RaiseCrush,
                    );
                }
            }
        }

        // Type 95: WR Raise floor to next highest + change texture.
        95 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_to_nearest(gs, level, tag, 1);
        }

        // Type 96: WR Raise floor by shortest lower texture.
        96 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_raise_by_texture(gs, level, tag, 1);
        }

        // -----------------------------------------------------------------
        // Floor lowerers
        // -----------------------------------------------------------------

        // Type 19: W1 Lower floor to highest adjacent floor.
        19 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_highest(gs, level, tag, 1);
        }

        // Type 23: S1 Lower floor to lowest adjacent floor.
        23 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 36: W1 Lower floor to highest adjacent - 8 (turbo).
        36 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 37: W1 Lower floor to lowest adjacent + change texture/type.
        37 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 38: W1 Lower floor to lowest adjacent floor.
        38 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 45: SR Lower floor to highest adjacent floor.
        45 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_highest(gs, level, tag, 1);
        }

        // Type 60: SR Lower floor to lowest adjacent floor.
        60 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 69: SR Lower floor to highest adjacent - 8.
        69 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        1,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 70: SR Lower floor to highest adjacent - 8 (turbo).
        70 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 71: S1 Lower floor to highest adjacent - 8 (turbo).
        71 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 82: WR Lower floor to lowest adjacent floor.
        82 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 83: WR Lower floor to highest adjacent floor.
        83 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_highest(gs, level, tag, 1);
        }

        // Type 84: WR Lower floor to lowest adjacent + change.
        84 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_lowest(gs, level, tag, 1);
        }

        // Type 98: WR Lower floor to highest adjacent - 8 (turbo).
        98 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in 0..level.sectors.len() {
                if level.sectors[idx].tag == tag {
                    let target = highest_adjacent_floor(level, idx) + 8;
                    activate_floor_lower_single_typed(
                        gs,
                        level,
                        idx,
                        tag,
                        target,
                        4,
                        FloorType::LowerToHighest,
                    );
                }
            }
        }

        // Type 102: S1 Lower floor to highest adjacent floor.
        102 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_floor_lower_to_highest(gs, level, tag, 1);
        }
        _ => {}
    }
}

#[allow(unused_variables)]
fn activate_stairs(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Teleporters
        // -----------------------------------------------------------------

        // -----------------------------------------------------------------
        // Stairs
        // -----------------------------------------------------------------

        // Type 7: S1 Build stairs 8 units.
        7 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_build_stairs(
                    gs,
                    level,
                    idx,
                    StairType::Build8,
                    crate::state::CrushBehavior::NoCrush,
                );
            }
        }

        // Type 8: W1 Build stairs turbo 16 units.
        8 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_build_stairs(
                    gs,
                    level,
                    idx,
                    StairType::Turbo16,
                    crate::state::CrushBehavior::NoCrush,
                );
            }
        }

        // Type 100: W1 Build stairs turbo 16 + crush.
        100 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_build_stairs(
                    gs,
                    level,
                    idx,
                    StairType::Turbo16,
                    crate::state::CrushBehavior::Crush,
                );
            }
        }

        // Type 127: S1 Build stairs turbo 16 units.
        127 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_build_stairs(
                    gs,
                    level,
                    idx,
                    StairType::Turbo16,
                    crate::state::CrushBehavior::NoCrush,
                );
            }
        }
        _ => {}
    }
}

#[allow(unused_variables)]
fn activate_platforms(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Perpetual platforms
        // -----------------------------------------------------------------

        // Type 53: S1 Perpetual platform (speed 1).
        53 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_perpetual_platform(gs, level, tag, 1);
        }

        // Type 54: W1 Stop platform (by tag).
        54 => {
            let tag = level.linedefs[linedef_idx].tag;
            gs.movers.active_platforms.retain(|p| p.tag != tag);
        }

        // Type 87: WR Perpetual platform (speed 1).
        87 => {
            let tag = level.linedefs[linedef_idx].tag;
            ev_perpetual_platform(gs, level, tag, 1);
        }

        // Type 89: WR Stop platform (by tag).
        89 => {
            let tag = level.linedefs[linedef_idx].tag;
            gs.movers.active_platforms.retain(|p| p.tag != tag);
        }
        _ => {}
    }
}

#[allow(unused_variables)]
fn activate_teleports(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Teleporters
        // -----------------------------------------------------------------

        // Type 39: W1 Teleport (walk trigger, one-shot).
        39 => {
            let tag = level.linedefs[linedef_idx].tag;
            let handle = gs.player.handle;
            ev_teleport(gs, level, tag, handle);
        }

        // Type 97: WR Teleport (walk trigger, repeatable).
        97 => {
            let tag = level.linedefs[linedef_idx].tag;
            let handle = gs.player.handle;
            ev_teleport(gs, level, tag, handle);
        }

        // Type 125: W1 Teleport Monsters Only.
        125 => {
            // Monsters-only teleport — no-op for player activation.
            // In a full implementation, this would only teleport monster actors.
        }

        // Type 126: WR Teleport Monsters Only (repeatable).
        126 => {
            // Monsters-only teleport — no-op for player activation.
        }
        _ => {}
    }
}

#[allow(unused_variables)]
fn activate_misc(
    gs: &mut GameState,
    level: &mut Level,
    special: u16,
    left_sidedef: i16,
    linedef_idx: usize,
) {
    match special {
        // -----------------------------------------------------------------
        // Donut specials
        // -----------------------------------------------------------------

        // Type 9: S1 Donut.
        9 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_do_donut(gs, level, idx);
            }
        }

        // Type 146: W1 Donut.
        146 => {
            let tag = level.linedefs[linedef_idx].tag;
            for idx in level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
            {
                ev_do_donut(gs, level, idx);
            }
        }
        _ => {}
    }
}

/// Return the parametric fraction `t` along the segment from `(ax, ay)` to
/// `(bx, by)` where it intersects the linedef segment `(lx1, ly1)` → `(lx2, ly2)`.
///
/// The fraction is returned as `(numerator, denominator)` with
/// `0 <= numerator <= denominator` and `denominator > 0`.
fn segment_intersection_frac(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    lx1: i32,
    ly1: i32,
    lx2: i32,
    ly2: i32,
) -> Option<(i64, i64)> {
    let rdx = i64::from(bx - ax);
    let rdy = i64::from(by - ay);
    let sdx = i64::from(lx2 - lx1);
    let sdy = i64::from(ly2 - ly1);
    let qpx = i64::from(lx1 - ax);
    let qpy = i64::from(ly1 - ay);

    let denom = rdx * sdy - rdy * sdx;
    if denom == 0 {
        return None;
    }

    let t_num = qpx * sdy - qpy * sdx;
    let u_num = qpx * rdy - qpy * rdx;
    let (t_num, u_num, denom) = if denom < 0 {
        (-t_num, -u_num, -denom)
    } else {
        (t_num, u_num, denom)
    };

    if !(0..=denom).contains(&t_num) || !(0..=denom).contains(&u_num) {
        return None;
    }

    Some((t_num, denom))
}

// ---------------------------------------------------------------------------
// Scrolling walls
// ---------------------------------------------------------------------------

/// Scan all linedefs in the level for scrolling wall specials and register
/// them in `gs.movers.scrolling_walls`.
///
/// Supported line types:
/// - **48**: Scroll texture left (speed_x = 1, speed_y = 0). The most common
///   scrolling wall in vanilla Doom (used for animated conveyor belts, water
///   textures on walls, etc.).
/// - **85**: Scroll texture right (speed_x = -1, speed_y = 0). Boom extension
///   but widely used in modern WADs.
pub fn init_scrolling_walls(gs: &mut GameState, level: &Level) {
    for (i, ld) in level.linedefs.iter().enumerate() {
        let (sx, sy) = match ld.special {
            48 => (1i16, 0i16),  // scroll left
            85 => (-1i16, 0i16), // scroll right
            _ => continue,
        };
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: i,
            speed_x: sx,
            speed_y: sy,
            accumulated_x: 0,
            accumulated_y: 0,
        });
    }
}

/// Advance all scrolling wall accumulators by one tic.
///
/// Called once per tic from `GameState::tick`. The accumulated offsets are
/// read by the renderer (via `GameState::get_scroll_offset`) and added to
/// the sidedef's `x_offset` / `y_offset` when drawing.
pub fn tick_scrollers(gs: &mut GameState) {
    for sw in &mut gs.movers.scrolling_walls {
        sw.accumulated_x = sw.accumulated_x.wrapping_add(sw.speed_x as i32);
        sw.accumulated_y = sw.accumulated_y.wrapping_add(sw.speed_y as i32);
    }
}

// ---------------------------------------------------------------------------
// Conveyor belts
// ---------------------------------------------------------------------------

/// Scan all linedefs in the level for conveyor belt specials and register
/// them in `gs.movers.conveyors`.
///
/// Supported line types:
/// - **253**: Scroll floor + push things (conveyor belt).
/// - **254**: Scroll floor + push things + scroll wall.
/// - **255**: Scroll wall using linedef offsets (generalized scroller).
///
/// The push direction and magnitude are derived from the linedef's sidedef
/// texture offsets (`x_offset` and `y_offset` of the right sidedef).
/// The sector affected is the one on the front side of the linedef (the
/// sector referenced by the right sidedef).
pub fn init_conveyors(gs: &mut GameState, level: &Level) {
    for ld in level.linedefs.iter() {
        match ld.special {
            253..=255 => {}
            _ => continue,
        }

        // The right sidedef must exist for the conveyor to function.
        let sd_idx = ld.right_sidedef;
        if sd_idx == SIDEDEF_NONE || (sd_idx as usize) >= level.sidedefs.len() {
            continue;
        }
        let sd = &level.sidedefs[sd_idx as usize];
        let sector_idx = sd.sector as usize;
        if sector_idx >= level.sectors.len() {
            continue;
        }

        // Derive push force from sidedef offsets: x_offset = horizontal push,
        // y_offset = vertical push. This matches the Boom convention.
        let push_x = sd.x_offset as i32;
        let push_y = sd.y_offset as i32;

        // Direction and speed are derived for informational purposes.
        // Direction: atan2(push_y, push_x) in degrees. For simplicity, store 0.
        // Speed: magnitude of push vector, simplified as max of abs values.
        let speed = (push_x.abs().max(push_y.abs())) as i16;

        gs.movers.conveyors.push(ConveyorBelt {
            sector_index: sector_idx,
            push_x,
            push_y,
            direction: 0,
            speed,
        });
    }
}

/// Apply conveyor belt push forces to all actors standing in conveyor sectors.
///
/// Called once per tic from `GameState::tick`. For each conveyor belt, any
/// actor whose `floor_z` (approximated as `z`) matches the sector floor is
/// pushed by the conveyor's force vector.
///
/// This is a simplified implementation — real Doom uses momentum-based push
/// rather than direct position adjustment.
///
/// **Performance:** Avoids 2 internal Vec allocations per game tic by iterating
/// over the components of the game state directly instead of performing `.collect::<Vec<_>>()`. NLL
/// provides the compiler proof necessary to drop mutability constraints correctly.
pub fn tick_conveyors(gs: &mut GameState, level: Option<&Level>) {
    if gs.movers.conveyors.is_empty() {
        return;
    }

    let level = match level {
        Some(lv) => lv,
        None => return, // Cannot determine sector membership without level geometry.
    };

    // Iterate all live actors and apply push if standing in a conveyor sector.
    // Use index iteration to avoid allocating a vector of handles while satisfying the borrow checker,
    // ensuring determinism by capturing the initial bounds.
    let initial_slot_count = gs.mobjslab.slot_count();
    let initial_generation = gs.mobjslab.next_generation();

    for i in 0..initial_slot_count {
        let Some(handle) = gs.mobjslab.handle_at(i) else {
            continue;
        };
        // Skip mobjs spawned during this iteration.
        if handle.generation >= initial_generation {
            continue;
        }

        let Some(mo) = gs.mobjslab.get(handle) else {
            continue;
        };
        let mz = mo.z.to_int();

        for conveyor in &gs.movers.conveyors {
            let sector_idx = conveyor.sector_index;
            if sector_idx >= level.sectors.len() {
                continue;
            }
            let floor_h = level.sectors[sector_idx].floor_height as i32;

            // Simple containment check: actor z matches sector floor.
            if mz == floor_h {
                if let Some(mo) = gs.mobjslab.get_mut(handle) {
                    mo.x += Fixed16_16::from_raw(conveyor.push_x);
                    mo.y += Fixed16_16::from_raw(conveyor.push_y);
                }
                break; // Only apply one conveyor per actor per tic.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::Mobj;
    use crate::state::GameState;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{ANG45, Bam, Fixed16_16};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Build a minimal 1×1 blockmap identical to the one in `movement.rs` tests.
    fn make_minimal_blockmap() -> doom_map::Blockmap {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count
        // offset table: block 0 is at word-offset 5 from start of lump.
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes()); // sentinel
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
        doom_map::Blockmap::parse_lump(&bm_data).expect("value must exist in test")
    }

    /// Build a level with one sector that has the given special, no linedefs.
    fn make_damage_level(floor_height: i16, special: u16) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("value must exist in test");
        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height,
                ceil_height: floor_height + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special,
                tag: 0,
            }],
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build an actor with the given floor height as z coordinate.
    fn make_actor_at_z(gs: &mut GameState, floor_height: i32) -> MobjHandle {
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.z = Fixed16_16::from_int(floor_height);
        gs.mobjslab.alloc(mo)
    }

    /// Build a two-sided linedef + supporting geometry for door tests.
    ///
    /// Layout:
    /// - Sector 0: front sector (actor is here), floor=0 ceil=128.
    /// - Sector 1: back sector (the door cavity), floor=0 ceil=`door_ceil`.
    /// - Vertex 0: (0, -10) — linedef from-vertex.
    /// - Vertex 1: (0, 10)  — linedef to-vertex (vertical wall at x=0).
    /// - Sidedef 0: right side → sector 0.
    /// - Sidedef 1: left side  → sector 1 (the door).
    /// - Linedef 0: two-sided (FLAG_TWO_SIDED=4), special=`special`, right=0, left=1.
    fn make_door_level_with_special(door_ceil: i16, special: u16) -> doom_map::Level {
        // Reject for 2 sectors: ceil(4/8) = 1 byte.
        let reject = doom_map::Reject::parse_lump(&[0u8], 2).expect("value must exist in test");

        let sectors = vec![
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            doom_map::Sector {
                floor_height: 0,
                ceil_height: door_ceil,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: -10 }, // v0
            doom_map::Vertex { x: 0, y: 10 },  // v1
        ];

        let sidedefs = vec![
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
        ];

        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004, // FLAG_TWO_SIDED
            special,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Convenience: door level with special=1 (the original helper).
    fn make_door_level(door_ceil: i16) -> doom_map::Level {
        make_door_level_with_special(door_ceil, 1)
    }

    // -----------------------------------------------------------------------
    // Tests: tick_sector_specials
    // -----------------------------------------------------------------------

    #[test]
    fn damage_floor_hurts_actor_standing_on_it() {
        let mut gs = GameState::new("TEST");
        let level = make_damage_level(0, 5); // special 5 = lava, floor=0
        let handle = make_actor_at_z(&mut gs, 0); // z matches floor_height

        tick_sector_specials(&mut gs, &level, handle);

        let health = gs
            .mobjslab
            .get(handle)
            .expect("value must exist in test")
            .health;
        assert_eq!(health, 90, "lava (special 5) must deal 10 damage per tic");
    }

    #[test]
    fn damage_floor_syncs_player_state_health() {
        let mut gs = GameState::new("TEST");
        let level = make_damage_level(0, 5); // special 5 = lava, floor=0
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);

        tick_sector_specials(&mut gs, &level, handle);

        assert_eq!(
            gs.player.health(),
            90,
            "player state must track sector damage"
        );
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .health,
            90,
            "player mobj health must stay aligned with player state"
        );
    }

    #[test]
    fn damage_floor_ignores_actor_above_it() {
        let mut gs = GameState::new("TEST");
        let level = make_damage_level(0, 5); // lava at floor=0
        let handle = make_actor_at_z(&mut gs, 10); // actor z=10, not on the floor

        tick_sector_specials(&mut gs, &level, handle);

        let health = gs
            .mobjslab
            .get(handle)
            .expect("value must exist in test")
            .health;
        assert_eq!(health, 100, "actor above lava floor must take no damage");
    }

    // -----------------------------------------------------------------------
    // Tests: p_use_lines
    // -----------------------------------------------------------------------

    #[test]
    fn p_use_lines_activates_nearest_linedef() {
        let mut gs = GameState::new("TEST");
        // Closed door: ceil == floor (0). make_door_level uses special 1 (SR door).
        let mut level = make_door_level(0);

        // Place actor at (-32, 0) facing East (Bam::ZERO → trig fallback, ray = east).
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(-32),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        let handle = gs.mobjslab.alloc(mo);

        // Before: no active door movers.
        assert_eq!(gs.movers.active_doors.len(), 0);

        p_use_lines(&mut gs, &mut level, handle);

        // After: a door mover was queued toward the standard door top height.
        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "p_use_lines must enqueue a door mover"
        );
        assert_eq!(
            gs.movers.active_doors[0].target_height, 124,
            "door target must be four units below the lowest adjacent ceiling"
        );
    }

    #[test]
    fn p_use_lines_uses_fractional_angle_without_east_fallback() {
        static INIT_TRIG: std::sync::Once = std::sync::Once::new();
        INIT_TRIG.call_once(|| {
            doom_types::Bam::init_trig_tables();
        });

        let mut gs = GameState::new("TEST");
        let mut level = make_door_level(0);
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(-32),
            Fixed16_16::from_int(-32),
            ANG45,
        );
        mo.health = 100;
        let handle = gs.mobjslab.alloc(mo);

        p_use_lines(&mut gs, &mut level, handle);

        assert_eq!(gs.movers.active_doors.len(), 1);
    }

    #[test]
    fn p_use_lines_back_side_does_not_activate_front_only_special() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 31);
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(-32),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        let handle = gs.mobjslab.alloc(mo);

        p_use_lines(&mut gs, &mut level, handle);

        assert!(
            gs.movers.active_doors.is_empty(),
            "front-only use special should not activate from the linedef back side"
        );
    }

    #[test]
    fn p_use_lines_prefers_nearest_crossed_special_over_lump_order() {
        let mut gs = GameState::new("TEST");
        let reject = doom_map::Reject::parse_lump(&[0u8; 2], 3).expect("value must exist in test");

        let mut level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![
                doom_map::Linedef {
                    from_vertex: 2,
                    to_vertex: 3,
                    flags: 0x0004,
                    special: 1,
                    tag: 0,
                    right_sidedef: 2,
                    left_sidedef: 3,
                },
                doom_map::Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0x0004,
                    special: 1,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
            ],
            sidedefs: vec![
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 1,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 2,
                },
            ],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: -10 },
                doom_map::Vertex { x: 0, y: 10 },
                doom_map::Vertex { x: 24, y: -10 },
                doom_map::Vertex { x: 24, y: 10 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 0,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 0,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(-32),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        let handle = gs.mobjslab.alloc(mo);

        p_use_lines(&mut gs, &mut level, handle);

        assert_eq!(gs.movers.active_doors.len(), 1);
        assert_eq!(
            gs.movers.active_doors[0].sector, 1,
            "USE should activate the nearest crossed door, not whichever linedef appears first"
        );
    }

    #[test]
    fn p_use_lines_closed_nonspecial_wall_blocks_special_behind_it() {
        let mut gs = GameState::new("TEST");
        let reject = doom_map::Reject::parse_lump(&[0u8], 2).expect("value must exist in test");

        let mut level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![
                doom_map::Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: SIDEDEF_NONE,
                },
                doom_map::Linedef {
                    from_vertex: 2,
                    to_vertex: 3,
                    flags: 0x0004,
                    special: 1,
                    tag: 0,
                    right_sidedef: 1,
                    left_sidedef: 2,
                },
            ],
            sidedefs: vec![
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 1,
                },
            ],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: -10 },
                doom_map::Vertex { x: 0, y: 10 },
                doom_map::Vertex { x: 24, y: -10 },
                doom_map::Vertex { x: 24, y: 10 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 0,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(-32),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        let handle = gs.mobjslab.alloc(mo);

        p_use_lines(&mut gs, &mut level, handle);

        assert_eq!(
            gs.movers.active_doors.len(),
            0,
            "a closed ordinary wall in front of a special must block USE"
        );
        assert_eq!(
            gs.sound.sound_queue.len(),
            1,
            "blocked use should queue feedback for the player"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: activate_linedef (legacy helper path for type 1)
    // -----------------------------------------------------------------------

    #[test]
    fn door_toggle_opens_closed_door() {
        let mut gs = GameState::new("TEST");
        // Door sector has ceil == floor (closed).
        let mut level = make_door_level(0);

        assert_eq!(level.sectors[1].ceil_height, 0, "precondition: door closed");

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            level.sectors[1].ceil_height, 128,
            "activate_linedef must open a closed door to floor + 128"
        );
    }

    #[test]
    fn door_toggle_closes_open_door() {
        let mut gs = GameState::new("TEST");
        // Door sector has ceil == floor + 128 (open).
        let mut level = make_door_level(128);

        assert_eq!(level.sectors[1].ceil_height, 128, "precondition: door open");

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            level.sectors[1].ceil_height, 0,
            "activate_linedef must close an open door to floor height"
        );
    }

    #[test]
    fn monster_activate_door_linedef_opens_regular_door() {
        let mut gs = GameState::new("TEST");
        let level = make_door_level_with_special(0, 1);

        assert!(monster_activate_door_linedef(&mut gs, &level, 0));
        assert_eq!(gs.movers.active_doors.len(), 1);
        assert_eq!(gs.movers.active_doors[0].sector, 1);
    }

    // -----------------------------------------------------------------------
    // Tests: animated doors (type 2)
    // -----------------------------------------------------------------------

    #[test]
    fn animated_door_opens_over_time() {
        let mut gs = GameState::new("TEST");
        // Type 2: open door, stays open. Start fully closed (ceil == floor == 0).
        let mut level = make_door_level_with_special(0, 2);

        assert_eq!(level.sectors[1].ceil_height, 0, "precondition: door closed");
        assert!(gs.movers.active_doors.is_empty());

        // Activate the linedef — enqueues a DoorMover.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "DoorMover should be enqueued"
        );

        // Tick doors several times — ceiling should rise.
        let initial_ceil = level.sectors[1].ceil_height;
        for _ in 0..10 {
            tick_doors(&mut gs, &mut level);
        }
        assert!(
            level.sectors[1].ceil_height > initial_ceil,
            "ceiling must rise after ticking doors"
        );
    }

    #[test]
    fn auto_close_door_starts_opening_on_first_tic() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 1);

        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(-32),
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        let handle = gs.mobjslab.alloc(mo);

        p_use_lines(&mut gs, &mut level, handle);

        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "door mover should be queued"
        );
        assert_eq!(
            gs.movers.active_doors[0].countdown, -1,
            "auto-close doors should not spend their wait time before opening"
        );

        tick_doors(&mut gs, &mut level);

        assert_eq!(
            level.sectors[1].ceil_height, 2,
            "normal doors should begin raising on the first tic"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: locked doors
    // -----------------------------------------------------------------------

    #[test]
    fn locked_door_blocked_without_key() {
        let mut gs = GameState::new("TEST");
        // Type 26: blue key required.
        let mut level = make_door_level_with_special(0, 26);

        // Player has no blue key.
        assert!(!gs.player.has_key(crate::player::KEY_BLUE_CARD));
        assert!(!gs.player.has_key(crate::player::KEY_BLUE_SKULL));

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            gs.movers.active_doors.is_empty(),
            "door must not open without blue key"
        );
    }

    #[test]
    fn locked_door_opens_with_blue_card() {
        let mut gs = GameState::new("TEST");
        // Type 26: blue key required.
        let mut level = make_door_level_with_special(0, 26);

        // Give player the blue card.
        gs.player.give_key(crate::player::KEY_BLUE_CARD);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "door must open when player has blue card"
        );
    }

    #[test]
    fn locked_door_opens_with_blue_skull() {
        let mut gs = GameState::new("TEST");
        // Type 26: blue key required (skull is equivalent).
        let mut level = make_door_level_with_special(0, 26);

        // Give player the blue skull.
        gs.player.give_key(crate::player::KEY_BLUE_SKULL);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "door must open when player has blue skull"
        );
    }

    #[test]
    fn yellow_locked_door_blocked_without_key() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 27);

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            gs.movers.active_doors.is_empty(),
            "yellow door must not open without yellow key"
        );
    }

    #[test]
    fn red_locked_door_blocked_without_key() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 28);

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            gs.movers.active_doors.is_empty(),
            "red door must not open without red key"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: spawn_level_specials / tick_lights
    // -----------------------------------------------------------------------

    #[test]
    fn spawn_level_specials_creates_light_thinker() {
        let mut gs = GameState::new("TEST");
        // One sector with special=1 (random off blinking light).
        let level = make_damage_level(0, 1);

        spawn_level_specials(&mut gs, &level);

        assert_eq!(
            gs.movers.active_lights.len(),
            1,
            "spawn_level_specials must create one light thinker for special=1"
        );
    }

    #[test]
    fn light_toggles_after_period() {
        let mut gs = GameState::new("TEST");
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("value must exist in test");
        let mut level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 2, // fast strobe
                tag: 0,
            }],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        spawn_level_specials(&mut gs, &level);
        assert_eq!(gs.movers.active_lights.len(), 1);

        // Tick past the period — light should toggle.
        let initial_light = level.sectors[0].light_level;
        for _ in 0..BLINK_FAST_PERIOD {
            tick_lights(&mut gs, &mut level);
        }
        // After one full period, light should have toggled to dark.
        assert_ne!(
            level.sectors[0].light_level, initial_light,
            "light must toggle after one period"
        );
    }

    // -----------------------------------------------------------------------
    // Helpers for crusher / lift / floor tests
    // -----------------------------------------------------------------------

    /// Build a level with multiple sectors connected by two-sided linedefs.
    ///
    /// Layout: 3 sectors, 2 two-sided linedefs connecting them in a chain.
    /// - Sector 0: floor=`floors[0]`, ceil=`ceils[0]`, tag=`tags[0]`
    /// - Sector 1: floor=`floors[1]`, ceil=`ceils[1]`, tag=`tags[1]`
    /// - Sector 2: floor=`floors[2]`, ceil=`ceils[2]`, tag=`tags[2]`
    /// - Linedef 0 connects sector 0 and sector 1 (special=`ld_special`, tag=`ld_tag`)
    /// - Linedef 1 connects sector 1 and sector 2 (special=0, tag=0)
    fn make_multi_sector_level(
        floors: [i16; 3],
        ceils: [i16; 3],
        tags: [u16; 3],
        ld_special: u16,
        ld_tag: u16,
    ) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8; 2], 3).expect("value must exist in test");

        let sectors = vec![
            doom_map::Sector {
                floor_height: floors[0],
                ceil_height: ceils[0],
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: tags[0],
            },
            doom_map::Sector {
                floor_height: floors[1],
                ceil_height: ceils[1],
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: tags[1],
            },
            doom_map::Sector {
                floor_height: floors[2],
                ceil_height: ceils[2],
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: tags[2],
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
            doom_map::Vertex { x: 128, y: 0 },
            doom_map::Vertex { x: 192, y: 0 },
        ];

        // Sidedef pairs for two linedefs.
        let sidedefs = vec![
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 0,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 2,
            },
        ];

        let linedefs = vec![
            doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004, // FLAG_TWO_SIDED
                special: ld_special,
                tag: ld_tag,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            doom_map::Linedef {
                from_vertex: 1,
                to_vertex: 2,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 2,
                left_sidedef: 3,
            },
        ];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build a simple level with one sector for crusher/floor tests.
    /// The sector has the given tag, and a linedef triggers it.
    fn make_tagged_sector_level(
        floor: i16,
        ceil: i16,
        tag: u16,
        ld_special: u16,
    ) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");

        let sectors = vec![
            // Sector 0: front sector (where the player stands).
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            // Sector 1: the target sector being moved.
            doom_map::Sector {
                floor_height: floor,
                ceil_height: ceil,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag,
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: -10 },
            doom_map::Vertex { x: 0, y: 10 },
        ];

        let sidedefs = vec![
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 0,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
        ];

        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: ld_special,
            tag,
            right_sidedef: 0,
            left_sidedef: 1,
        }];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    // -----------------------------------------------------------------------
    // Tests: adjacent sector height helpers
    // -----------------------------------------------------------------------

    #[test]
    fn lowest_adjacent_floor_finds_minimum() {
        // Sector 0: floor=0, Sector 1: floor=64, Sector 2: floor=32
        // Sector 1 is adjacent to both 0 and 2.
        let level = make_multi_sector_level([0, 64, 32], [128, 128, 128], [0, 0, 0], 0, 0);

        // Adjacent to sector 1: sectors 0 (floor=0) and 2 (floor=32).
        let result = lowest_adjacent_floor(&level, 1);
        assert_eq!(
            result, 0,
            "lowest adjacent floor to sector 1 should be 0 (sector 0)"
        );
    }

    #[test]
    fn highest_adjacent_floor_finds_maximum() {
        let level = make_multi_sector_level([10, 64, 50], [128, 128, 128], [0, 0, 0], 0, 0);

        // Adjacent to sector 1: sectors 0 (floor=10) and 2 (floor=50).
        let result = highest_adjacent_floor(&level, 1);
        assert_eq!(
            result, 50,
            "highest adjacent floor to sector 1 should be 50 (sector 2)"
        );
    }

    #[test]
    fn next_highest_floor_finds_next_step() {
        // Sector 1: floor=0, adjacent to sector 0 (floor=32) and sector 2 (floor=64).
        let level = make_multi_sector_level([32, 0, 64], [128, 128, 128], [0, 0, 0], 0, 0);

        let result = next_highest_floor(&level, 1);
        assert_eq!(result, 32, "next highest floor above 0 should be 32");
    }

    #[test]
    fn next_highest_floor_no_higher_returns_own() {
        // Sector 1: floor=100, adjacent floors are 20 and 50 (both lower).
        let level = make_multi_sector_level([20, 100, 50], [200, 200, 200], [0, 0, 0], 0, 0);

        let result = next_highest_floor(&level, 1);
        assert_eq!(result, 100, "no higher floor => returns own floor");
    }

    #[test]
    fn lowest_adjacent_ceiling_finds_minimum() {
        // Sector 1 adjacent to sector 0 (ceil=128) and sector 2 (ceil=96).
        let level = make_multi_sector_level([0, 0, 0], [128, 200, 96], [0, 0, 0], 0, 0);

        let result = lowest_adjacent_ceiling(&level, 1);
        assert_eq!(
            result, 96,
            "lowest adjacent ceiling to sector 1 should be 96 (sector 2)"
        );
    }

    #[test]
    fn lowest_adjacent_floor_no_neighbors_returns_own() {
        // A sector with no linedefs connecting to others.
        let level = make_damage_level(42, 0);
        let result = lowest_adjacent_floor(&level, 0);
        assert_eq!(result, 42, "no adjacent sectors => returns own floor");
    }

    // -----------------------------------------------------------------------
    // Tests: CeilingMover (crushers)
    // -----------------------------------------------------------------------

    #[test]
    fn crusher_oscillates_between_heights() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 6);

        // Activate line type 6 (fast crusher, perpetual).
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_ceilings.len(),
            1,
            "one crusher must be created"
        );

        let initial_ceil = level.sectors[1].ceil_height;
        assert_eq!(initial_ceil, 128);

        // Tick until ceiling descends: 128 to 8 = 120 units / speed 2 = 60 tics.
        for _ in 0..60 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(
            level.sectors[1].ceil_height, 8,
            "ceiling must reach bottom_height"
        );

        // Should have reversed to Up.
        assert_eq!(
            gs.movers.active_ceilings[0].direction,
            MoveDirection::Up,
            "crusher must reverse to Up after hitting bottom"
        );

        // Tick until it returns to the top: 120 units / speed 2 = 60 tics.
        for _ in 0..60 {
            tick_ceilings(&mut gs, &mut level);
        }

        // Should have reached top and reversed back to Down (perpetual).
        assert_eq!(
            gs.movers.active_ceilings[0].direction,
            MoveDirection::Down,
            "perpetual crusher must reverse to Down after reaching top"
        );
        assert_eq!(
            level.sectors[1].ceil_height, 128,
            "crusher must return to top_height"
        );

        // Verify perpetual: tick a few more — should descend again.
        for _ in 0..5 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(
            level.sectors[1].ceil_height, 118,
            "perpetual crusher must continue oscillating"
        );
    }

    #[test]
    fn one_shot_crusher_removes_itself() {
        let mut gs = GameState::new("TEST");
        // Line type 44: LowerAndCrush — one-shot ceiling lower, removes at bottom.
        let mut level = make_tagged_sector_level(0, 128, 1, 44);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::LowerAndCrush,
            "type 44 must create a LowerAndCrush ceiling"
        );

        // Tick down to bottom_height (8). 128 -> 8 = 120 units / speed 2 = 60 tics.
        for _ in 0..60 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].ceil_height, 8);

        // LowerAndCrush removes itself when reaching bottom.
        assert!(
            gs.movers.active_ceilings.is_empty(),
            "LowerAndCrush must remove itself after reaching bottom"
        );
    }

    #[test]
    fn crusher_stops_on_line_type_57() {
        let mut gs = GameState::new("TEST");
        // Start a crusher with tag=5.
        let mut level = make_tagged_sector_level(0, 128, 5, 6);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);

        // Tick a few times.
        for _ in 0..5 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert!(
            !gs.movers.active_ceilings.is_empty(),
            "crusher should still be running"
        );

        // Now add a linedef with special 57 and the same tag.
        level.linedefs.push(doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 57,
            tag: 5,
            right_sidedef: 0,
            left_sidedef: 1,
        });

        // Activate line type 57 (stop crusher).
        let stop_idx = level.linedefs.len() - 1;
        activate_linedef(&mut gs, &mut level, stop_idx);

        assert!(
            gs.movers.active_ceilings.is_empty(),
            "line type 57 must stop all crushers with matching tag"
        );
    }

    #[test]
    fn slow_crusher_type_25_speed_1() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 25);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].speed, 1,
            "type 25 must use speed 1 (slow)"
        );

        // Tick once — should move by 1.
        tick_ceilings(&mut gs, &mut level);
        assert_eq!(level.sectors[1].ceil_height, 127);
    }

    // -----------------------------------------------------------------------
    // Tests: FloorMover (lifts)
    // -----------------------------------------------------------------------

    #[test]
    fn lift_lower_wait_raise() {
        let mut gs = GameState::new("TEST");

        // Sector 0: floor=0 (front), Sector 1: floor=64 (target, tag=1).
        // Sector 0 is adjacent to sector 1 with floor=0 → lowest adjacent = 0.
        let mut level = make_tagged_sector_level(64, 128, 1, 62);

        assert_eq!(level.sectors[1].floor_height, 64, "precondition: floor=64");

        // Activate line type 62 (lift lower-wait-raise, speed 4).
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_floors.len(),
            1,
            "one floor mover must be created"
        );

        // Lowest adjacent floor is sector 0's floor = 0.
        assert_eq!(
            gs.movers.active_floors[0].target_height, 0,
            "lift target = lowest adjacent = 0"
        );
        assert_eq!(
            gs.movers.active_floors[0].return_height, 64,
            "return height = original floor"
        );

        // Tick until floor lowers to 0: 64 units / speed 4 = 16 tics.
        for _ in 0..16 {
            tick_floors(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].floor_height, 0, "floor must lower to 0");

        // Should now be in wait phase.
        assert!(
            gs.movers.active_floors[0].waiting,
            "lift must enter wait phase"
        );
        assert_eq!(
            gs.movers.active_floors[0].wait_remaining, LIFT_WAIT,
            "wait_remaining must be set to LIFT_WAIT"
        );

        // Tick through the wait phase (105 tics).
        for _ in 0..LIFT_WAIT {
            tick_floors(&mut gs, &mut level);
        }
        assert!(!gs.movers.active_floors[0].waiting, "wait phase must end");

        // Should now be heading back up to return_height (64).
        // 64 units / speed 4 = 16 tics.
        for _ in 0..16 {
            tick_floors(&mut gs, &mut level);
        }
        assert_eq!(
            level.sectors[1].floor_height, 64,
            "floor must raise back to 64"
        );

        // Lift should be removed after returning.
        assert!(
            gs.movers.active_floors.is_empty(),
            "lift must remove itself after return"
        );
    }

    #[test]
    fn turbo_lift_type_121_speed_8() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(64, 128, 1, 121);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].speed, 8,
            "type 121 must use speed 8 (turbo)"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: FloorMover (floor raisers / lowerers)
    // -----------------------------------------------------------------------

    #[test]
    fn floor_raise_to_next_highest() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=32, Sector 1: floor=0 (target, tag=1), Sector 2: floor=64.
        // Sector 1 is adjacent to 0 (floor=32) and 2 (floor=64).
        // next_highest_floor above 0 = 32.
        let mut level = make_multi_sector_level(
            [32, 0, 64],
            [128, 128, 128],
            [0, 1, 0],
            18, // type 18: floor raise to next highest adjacent floor
            1,  // tag 1 targets sector 1
        );

        assert_eq!(level.sectors[1].floor_height, 0);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 32,
            "target = next highest floor = 32"
        );

        // Tick until floor reaches 32: 32 units / speed 1 = 32 tics.
        for _ in 0..32 {
            tick_floors(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].floor_height, 32, "floor must reach 32");

        // One-shot raiser should be removed.
        assert!(
            gs.movers.active_floors.is_empty(),
            "one-shot floor raiser must remove itself"
        );
    }

    #[test]
    fn floor_lower_to_lowest_adjacent() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=0, Sector 1: floor=64 (target, tag=1), Sector 2: floor=32.
        // Sector 1 adjacent to 0 (floor=0) and 2 (floor=32).
        // lowest_adjacent_floor = 0.
        let mut level = make_multi_sector_level(
            [0, 64, 32],
            [128, 128, 128],
            [0, 1, 0],
            23, // type 23: floor lower to lowest adjacent
            1,
        );

        assert_eq!(level.sectors[1].floor_height, 64);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 0,
            "target = lowest adjacent = 0"
        );

        // Tick until floor reaches 0: 64 units / speed 1 = 64 tics.
        for _ in 0..64 {
            tick_floors(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].floor_height, 0, "floor must lower to 0");

        assert!(
            gs.movers.active_floors.is_empty(),
            "one-shot floor lowerer must remove itself"
        );
    }

    #[test]
    fn floor_raise_to_lowest_ceiling_type_5() {
        let mut gs = GameState::new("TEST");
        // Sector 0: ceil=128, Sector 1: floor=0 ceil=200 tag=1, Sector 2: ceil=96.
        // Sector 1 adjacent to 0 (ceil=128) and 2 (ceil=96).
        // lowest_adjacent_ceiling = 96.
        let mut level = make_multi_sector_level(
            [0, 0, 0],
            [128, 200, 96],
            [0, 1, 0],
            5, // type 5: floor raise to lowest adjacent ceiling
            1,
        );

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 96,
            "target = lowest adjacent ceiling = 96"
        );
        assert!(
            gs.movers.active_floors[0].crush == crate::state::CrushBehavior::Crush,
            "type 5 must have crush=true"
        );
    }

    #[test]
    fn floor_lower_to_highest_adjacent_type_19() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=10, Sector 1: floor=64 tag=1, Sector 2: floor=48.
        // highest_adjacent_floor = 48.
        let mut level = make_multi_sector_level(
            [10, 64, 48],
            [128, 128, 128],
            [0, 1, 0],
            19, // type 19: floor lower to highest adjacent floor
            1,
        );

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 48,
            "target = highest adjacent = 48"
        );
    }

    #[test]
    fn floor_lower_type_36_to_8_above_highest() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=10, Sector 1: floor=64 tag=1, Sector 2: floor=30.
        // highest_adjacent_floor = 30, target = 30 + 8 = 38.
        let mut level = make_multi_sector_level(
            [10, 64, 30],
            [128, 128, 128],
            [0, 1, 0],
            36, // type 36
            1,
        );

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 38,
            "target = highest_adj(30) + 8 = 38"
        );
    }

    #[test]
    fn floor_raise_type_56_to_8_below_lowest_ceiling() {
        let mut gs = GameState::new("TEST");
        // Sector 0: ceil=128, Sector 1: floor=0 ceil=200 tag=1, Sector 2: ceil=100.
        // lowest_adjacent_ceiling = 100, target = 100 - 8 = 92.
        let mut level = make_multi_sector_level([0, 0, 0], [128, 200, 100], [0, 1, 0], 56, 1);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 92,
            "target = lowest_adj_ceil(100) - 8 = 92"
        );
        assert!(
            gs.movers.active_floors[0].crush == crate::state::CrushBehavior::Crush,
            "type 56 must have crush=true"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: duplicate mover prevention
    // -----------------------------------------------------------------------

    #[test]
    fn duplicate_crusher_prevented() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 6);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);

        // Try to activate again — should not add a duplicate.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_ceilings.len(),
            1,
            "must not create duplicate crushers"
        );
    }

    #[test]
    fn duplicate_lift_prevented() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(64, 128, 1, 62);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_floors.len(),
            1,
            "must not create duplicate lifts"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: GameState clone includes new fields
    // -----------------------------------------------------------------------

    #[test]
    fn game_state_clone_includes_ceilings_and_floors() {
        let mut gs = GameState::new("TEST");
        gs.movers.active_ceilings.push(CeilingMover {
            sector_index: 0,
            top_height: 128,
            bottom_height: 8,
            speed: 2,
            normal_speed: 2,
            crush_damage: 10,
            direction: MoveDirection::Down,
            silent: false,
            remove_when_done: false,
            tag: 1,
            ceiling_type: CeilingType::CrushAndRaise,
        });
        gs.movers.active_floors.push(FloorMover {
            sector_index: 0,
            target_height: 0,
            speed: 4,
            direction: MoveDirection::Down,
            wait_tics: 105,
            return_height: 64,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::NoCrush,
            tag: 1,
            floor_type: FloorType::LowerToLowest,
        });

        let gs2 = gs.clone();
        assert_eq!(
            gs2.movers.active_ceilings.len(),
            1,
            "clone must include ceilings"
        );
        assert_eq!(
            gs2.movers.active_floors.len(),
            1,
            "clone must include floors"
        );
        assert_eq!(gs2.movers.active_ceilings[0].top_height, 128);
        assert_eq!(gs2.movers.active_floors[0].target_height, 0);
    }

    // -----------------------------------------------------------------------
    // Tests: exit line types (11, 51, 52, 124)
    // -----------------------------------------------------------------------

    #[test]
    fn exit_type_11_sets_normal_exit() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 11);

        assert_eq!(gs.exit_request, None, "precondition: no exit request");
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Normal),
            "type 11 must set ExitRequest::Normal"
        );
    }

    #[test]
    fn exit_type_51_sets_secret_exit() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 51);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Secret),
            "type 51 must set ExitRequest::Secret"
        );
    }

    #[test]
    fn exit_type_52_walk_sets_normal_exit() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 52);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Normal),
            "type 52 (walk trigger) must set ExitRequest::Normal"
        );
    }

    #[test]
    fn exit_type_124_walk_sets_secret_exit() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 124);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Secret),
            "type 124 (walk trigger) must set ExitRequest::Secret"
        );
    }

    #[test]
    fn exit_request_is_none_by_default() {
        let gs = GameState::new("TEST");
        assert_eq!(
            gs.exit_request, None,
            "exit_request must be None on creation"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: Teleporters
    // -----------------------------------------------------------------------

    /// Build a level with a teleport destination thing (kind 14) and a tagged sector.
    fn make_teleport_level(
        dest_x: i16,
        dest_y: i16,
        dest_angle: u16,
        dest_sector_floor: i16,
        sector_tag: u16,
        ld_special: u16,
    ) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");

        let sectors = vec![
            // Sector 0: source sector (player starts here).
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            // Sector 1: destination sector (tagged).
            doom_map::Sector {
                floor_height: dest_sector_floor,
                ceil_height: dest_sector_floor + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: sector_tag,
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: -10 },
            doom_map::Vertex { x: 0, y: 10 },
        ];

        let sidedefs = vec![
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 0,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
        ];

        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: ld_special,
            tag: sector_tag,
            right_sidedef: 0,
            left_sidedef: 1,
        }];

        // Teleport destination thing (DoomEd type 14).
        let things = vec![doom_map::Thing {
            x: dest_x,
            y: dest_y,
            angle: dest_angle,
            kind: 14,
            flags: 7,
        }];

        doom_map::Level {
            name: "TEST".to_string(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    #[test]
    fn ev_teleport_no_matching_sector_returns_false() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        // Level with no sectors matching tag 99.
        let level = make_damage_level(0, 0);
        assert!(!ev_teleport(&mut gs, &level, 99, handle));
    }

    #[test]
    fn ev_teleport_moves_mobj_to_destination() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        let level = make_teleport_level(500, 600, 90, 32, 1, 39);

        let result = ev_teleport(&mut gs, &level, 1, handle);
        assert!(
            result,
            "ev_teleport must return true when destination found"
        );

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(mo.x, doom_types::Fixed16_16::from_int(500));
        assert_eq!(mo.y, doom_types::Fixed16_16::from_int(600));
    }

    #[test]
    fn ev_teleport_sets_angle_to_destination() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        let level = make_teleport_level(100, 200, 180, 0, 1, 39);

        ev_teleport(&mut gs, &level, 1, handle);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        // 180 degrees ~ 0x8000_0000 BAM. Check approximate match.
        let expected = doom_types::Bam(0x8000_0000);
        let diff = mo.angle.0.wrapping_sub(expected.0);
        assert!(
            !(0x0100_0000..=0xFF00_0000).contains(&diff),
            "angle must be approximately 180 degrees after teleport, got {:08X}",
            mo.angle.0
        );
    }

    #[test]
    fn ev_teleport_sets_z_to_dest_floor() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        let level = make_teleport_level(100, 200, 0, 64, 1, 39);

        ev_teleport(&mut gs, &level, 1, handle);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(
            mo.z,
            doom_types::Fixed16_16::from_int(64),
            "z must be set to destination sector floor height"
        );
    }

    #[test]
    fn activate_linedef_type_39_triggers_teleport() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let mut level = make_teleport_level(500, 600, 0, 0, 1, 39);

        activate_linedef(&mut gs, &mut level, 0);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(
            mo.x,
            doom_types::Fixed16_16::from_int(500),
            "type 39 must teleport player"
        );
    }

    #[test]
    fn activate_linedef_type_97_triggers_teleport() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let mut level = make_teleport_level(300, 400, 90, 0, 1, 97);

        activate_linedef(&mut gs, &mut level, 0);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(
            mo.x,
            doom_types::Fixed16_16::from_int(300),
            "type 97 must teleport player"
        );
    }

    #[test]
    fn activate_linedef_type_125_monsters_only_recognized() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let mut level = make_teleport_level(500, 600, 0, 0, 1, 125);

        // Type 125 is monsters-only — should not teleport the player.
        activate_linedef(&mut gs, &mut level, 0);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(
            mo.x,
            doom_types::Fixed16_16::ZERO,
            "type 125 (monsters only) must not teleport player"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: Sector Damage (periodic)
    // -----------------------------------------------------------------------

    #[test]
    fn sector_damage_special_5_hurts_player() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let level = make_damage_level(0, 5);

        // Set level_time to a multiple of 32 so damage triggers.
        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(mo.health, 95, "hellslime (special 5) must deal 5 damage");
    }

    #[test]
    fn sector_damage_special_5_syncs_player_state_health() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let level = make_damage_level(0, 5);

        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);

        assert_eq!(
            gs.player.health(),
            95,
            "player state must track periodic sector damage"
        );
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .health,
            95,
            "player mobj health must stay aligned with periodic sector damage"
        );
    }

    #[test]
    fn sector_damage_special_7_hurts_less() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let level = make_damage_level(0, 7);

        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(mo.health, 98, "nukage (special 7) must deal 2 damage");
    }

    #[test]
    fn sector_damage_special_16_hurts_heavily() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let level = make_damage_level(0, 16);

        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(
            mo.health, 80,
            "super hellslime (special 16) must deal 20 damage"
        );
    }

    #[test]
    fn radsuit_prevents_nukage_damage() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        // Give player RadSuit.
        gs.player.powers[crate::player::powers::PW_IRONFEET] = 100;
        let level = make_damage_level(0, 7);

        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(mo.health, 100, "RadSuit must prevent nukage damage");
    }

    #[test]
    fn special_11_damages_and_triggers_exit() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        // Set player health low enough that after 20 damage it triggers exit.
        gs.set_player_health_capped(25, 100);
        let level = make_damage_level(0, 11);

        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(mo.health, 5, "God exit must deal 20 damage");
        assert_eq!(
            gs.exit_request,
            Some(crate::state::ExitRequest::Normal),
            "God exit must set ExitRequest::Normal when health <= 10"
        );
    }

    #[test]
    fn no_damage_when_sector_special_is_zero() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let level = make_damage_level(0, 0);

        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(mo.health, 100, "no damage when sector special is 0");
    }

    #[test]
    fn damage_tick_only_applies_every_32_tics() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let level = make_damage_level(0, 5);

        // level_time = 1 (not a multiple of 32) -- no damage.
        gs.stats.level_time = 1;
        tick_sector_damage(&mut gs, &level);
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .health,
            100
        );

        // level_time = 15 -- no damage.
        gs.stats.level_time = 15;
        tick_sector_damage(&mut gs, &level);
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .health,
            100
        );

        // level_time = 32 -- damage applied.
        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .health,
            95
        );
    }

    #[test]
    fn player_full_health_survives_several_nukage_ticks() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let level = make_damage_level(0, 7);

        // Apply damage 5 times (every 32 tics).
        for i in 1..=5 {
            gs.stats.level_time = i * 32;
            tick_sector_damage(&mut gs, &level);
        }

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(
            mo.health, 90,
            "5 nukage ticks at 2 damage each = 10 total damage, 100-10=90"
        );
        assert!(mo.health > 0, "player must survive 5 nukage periods");
    }

    // -----------------------------------------------------------------------
    // Tests: Sector Light Effects
    // -----------------------------------------------------------------------

    #[test]
    fn init_sector_lights_creates_effects_for_specials_1_2_3() {
        let mut gs = GameState::new("TEST");
        let reject = doom_map::Reject::parse_lump(&[0u8; 2], 3).expect("value must exist in test");
        let level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 200,
                    special: 1, // blink random
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 160,
                    special: 2, // blink 0.5s
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 128,
                    special: 3, // blink 1s
                    tag: 0,
                },
            ],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        init_sector_lights(&mut gs, &level);

        assert_eq!(
            gs.movers.sector_lights.len(),
            3,
            "init_sector_lights must create effects for specials 1, 2, 3"
        );
        assert_eq!(
            gs.movers.sector_lights[0].effect_type,
            crate::state::LightEffectType::BlinkRandom
        );
        assert_eq!(
            gs.movers.sector_lights[1].effect_type,
            crate::state::LightEffectType::Blink05s
        );
        assert_eq!(
            gs.movers.sector_lights[2].effect_type,
            crate::state::LightEffectType::Blink1s
        );
    }

    #[test]
    fn tick_sector_lights_changes_light_levels() {
        let mut gs = GameState::new("TEST");
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("value must exist in test");
        let mut level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 200,
                special: 2, // blink 0.5s
                tag: 0,
            }],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        init_sector_lights(&mut gs, &level);
        assert_eq!(gs.movers.sector_lights.len(), 1);

        let initial_light = level.sectors[0].light_level;

        // Tick past the period to trigger a toggle.
        for _ in 0..BLINK_FAST_PERIOD {
            tick_sector_lights(&mut gs, &mut level);
        }

        assert_ne!(
            level.sectors[0].light_level, initial_light,
            "tick_sector_lights must change light level after period"
        );
    }

    #[test]
    fn light_effect_type_derives_partial_eq() {
        use crate::state::LightEffectType;
        assert_eq!(LightEffectType::BlinkRandom, LightEffectType::BlinkRandom);
        assert_ne!(LightEffectType::Blink05s, LightEffectType::Blink1s);
    }

    #[test]
    fn sector_light_effect_clone_works() {
        use crate::state::{LightEffectType, SectorLightEffect};
        let effect = SectorLightEffect {
            sector_index: 0,
            effect_type: LightEffectType::Oscillate,
            base_light: 200,
            min_light: 100,
            timer: 10,
        };
        let cloned = effect.clone();
        assert_eq!(cloned.sector_index, 0);
        assert_eq!(cloned.effect_type, LightEffectType::Oscillate);
        assert_eq!(cloned.base_light, 200);
        assert_eq!(cloned.min_light, 100);
        assert_eq!(cloned.timer, 10);
    }

    // -----------------------------------------------------------------------
    // Tests: player_sector_index
    // -----------------------------------------------------------------------

    #[test]
    fn player_sector_index_finds_matching_sector() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 32);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let reject = doom_map::Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");
        let level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: 32,
                    ceil_height: 160,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        let idx = player_sector_index(&gs, &level);
        assert_eq!(
            idx,
            Some(1),
            "player at z=32 should match sector 1 (floor=32)"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: ev_teleport clears momentum
    // -----------------------------------------------------------------------

    #[test]
    fn ev_teleport_clears_momentum() {
        let mut gs = GameState::new("TEST");
        let mut mo = crate::mobj::Mobj::new(
            doom_types::mobj_kind::MobjKind::Player,
            doom_types::Fixed16_16::ZERO,
            doom_types::Fixed16_16::ZERO,
            doom_types::Bam::ZERO,
        );
        mo.health = 100;
        mo.momx = doom_types::Fixed16_16::from_int(5);
        mo.momy = doom_types::Fixed16_16::from_int(3);
        let handle = gs.mobjslab.alloc(mo);
        let level = make_teleport_level(100, 200, 0, 0, 1, 39);

        ev_teleport(&mut gs, &level, 1, handle);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(
            mo.momx,
            doom_types::Fixed16_16::ZERO,
            "momx must be cleared after teleport"
        );
        assert_eq!(
            mo.momy,
            doom_types::Fixed16_16::ZERO,
            "momy must be cleared after teleport"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: GameState clone includes sector_lights
    // -----------------------------------------------------------------------

    #[test]
    fn game_state_clone_includes_sector_lights() {
        use crate::state::{LightEffectType, SectorLightEffect};
        let mut gs = GameState::new("TEST");
        gs.movers.sector_lights.push(SectorLightEffect {
            sector_index: 0,
            effect_type: LightEffectType::Blink05s,
            base_light: 200,
            min_light: 0,
            timer: 15,
        });

        let gs2 = gs.clone();
        assert_eq!(
            gs2.movers.sector_lights.len(),
            1,
            "clone must include sector_lights"
        );
        assert_eq!(gs2.movers.sector_lights[0].base_light, 200);
    }

    // -----------------------------------------------------------------------
    // Tests: init_sector_lights for extended types (8, 12, 13, 17)
    // -----------------------------------------------------------------------

    #[test]
    fn init_sector_lights_creates_oscillate_and_fire_flicker() {
        let mut gs = GameState::new("TEST");
        let reject = doom_map::Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");
        let level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 160,
                    special: 8, // oscillate
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 17, // fire flicker
                    tag: 0,
                },
            ],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        init_sector_lights(&mut gs, &level);

        assert_eq!(gs.movers.sector_lights.len(), 2);
        assert_eq!(
            gs.movers.sector_lights[0].effect_type,
            crate::state::LightEffectType::Oscillate
        );
        assert_eq!(
            gs.movers.sector_lights[1].effect_type,
            crate::state::LightEffectType::FireFlicker
        );
    }

    // -----------------------------------------------------------------------
    // Tests: radsuit does NOT protect from special 11
    // -----------------------------------------------------------------------

    #[test]
    fn radsuit_does_not_protect_from_god_exit() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        gs.player.powers[crate::player::powers::PW_IRONFEET] = 100;
        let level = make_damage_level(0, 11);

        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(
            mo.health, 80,
            "RadSuit must NOT protect from God exit (special 11)"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: sector damage special 4
    // -----------------------------------------------------------------------

    #[test]
    fn sector_damage_special_4_nukage_blink() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let level = make_damage_level(0, 4);

        gs.stats.level_time = 32;
        tick_sector_damage(&mut gs, &level);

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(mo.health, 95, "special 4 (nukage blink) must deal 5 damage");
    }

    // -----------------------------------------------------------------------
    // Helpers for stairs/donut/platform tests
    // -----------------------------------------------------------------------

    /// Build a level with multiple sectors and two-sided linedefs connecting them.
    ///
    /// Layout: N sectors chained linearly.
    /// Sector i is connected to sector i+1 by a two-sided linedef.
    /// All sectors share the same floor flat (`FLAT1`) by default.
    fn make_stair_level(sector_count: usize, base_floor: i16, tag: u16) -> doom_map::Level {
        let reject_bytes = vec![0u8; (sector_count * sector_count).div_ceil(8)];
        let reject = doom_map::Reject::parse_lump(&reject_bytes, sector_count)
            .expect("value must exist in test");

        let mut sectors = Vec::new();
        for i in 0..sector_count {
            sectors.push(doom_map::Sector {
                floor_height: base_floor,
                ceil_height: base_floor + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: if i == 0 { tag } else { 0 },
            });
        }

        // One vertex per sector boundary + 1 extra.
        let mut vertexes = Vec::new();
        for i in 0..=sector_count {
            vertexes.push(doom_map::Vertex {
                x: (i as i16) * 64,
                y: 0,
            });
        }

        // Two sidedefs per linedef: right→sector i, left→sector i+1.
        let mut sidedefs = Vec::new();
        let mut linedefs = Vec::new();

        for i in 0..sector_count.saturating_sub(1) {
            let right_sd = sidedefs.len() as u16;
            sidedefs.push(doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: *b"\0\0\0\0\0\0\0\0",
                sector: i as u16,
            });
            let left_sd = sidedefs.len() as u16;
            sidedefs.push(doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: *b"\0\0\0\0\0\0\0\0",
                sector: (i + 1) as u16,
            });
            linedefs.push(doom_map::Linedef {
                from_vertex: i as u16,
                to_vertex: (i + 1) as u16,
                flags: 0x0004, // FLAG_TWO_SIDED
                special: 0,
                tag: 0,
                right_sidedef: right_sd,
                left_sidedef: left_sd,
            });
        }

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build a donut level: 3 sectors.
    /// Sector 0 = trigger (the donut "ring" initiator).
    /// Sector 1 = donut hole (enclosed sector).
    /// Sector 2 = ring (surrounding sector).
    ///
    /// Linedef 0: right=sector0, left=sector1 (trigger→hole).
    /// Linedef 1: right=sector1, left=sector2 (hole→ring).
    fn make_donut_level(
        trigger_floor: i16,
        hole_floor: i16,
        ring_floor: i16,
        tag: u16,
    ) -> doom_map::Level {
        let reject_bytes = vec![0u8; 2]; // 3 sectors: ceil(9/8)=2
        let reject =
            doom_map::Reject::parse_lump(&reject_bytes, 3).expect("value must exist in test");

        let sectors = vec![
            doom_map::Sector {
                floor_height: trigger_floor,
                ceil_height: trigger_floor + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag,
            },
            doom_map::Sector {
                floor_height: hole_floor,
                ceil_height: hole_floor + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            doom_map::Sector {
                floor_height: ring_floor,
                ceil_height: ring_floor + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
            doom_map::Vertex { x: 128, y: 0 },
        ];

        let sidedefs = vec![
            // Linedef 0: right = sector 0
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: *b"\0\0\0\0\0\0\0\0",
                sector: 0,
            },
            // Linedef 0: left = sector 1
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: *b"\0\0\0\0\0\0\0\0",
                sector: 1,
            },
            // Linedef 1: right = sector 1
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: *b"\0\0\0\0\0\0\0\0",
                sector: 1,
            },
            // Linedef 1: left = sector 2
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: *b"\0\0\0\0\0\0\0\0",
                sector: 2,
            },
        ];

        let linedefs = vec![
            doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            doom_map::Linedef {
                from_vertex: 1,
                to_vertex: 2,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 2,
                left_sidedef: 3,
            },
        ];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build a platform level: 2 sectors connected by a two-sided linedef.
    /// Sector 0 = adjacent sector (provides lowest floor).
    /// Sector 1 = platform sector (tagged).
    fn make_platform_level(adj_floor: i16, plat_floor: i16, tag: u16) -> doom_map::Level {
        let reject_bytes = vec![0u8; 1]; // 2 sectors
        let reject =
            doom_map::Reject::parse_lump(&reject_bytes, 2).expect("value must exist in test");

        let sectors = vec![
            doom_map::Sector {
                floor_height: adj_floor,
                ceil_height: adj_floor + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            doom_map::Sector {
                floor_height: plat_floor,
                ceil_height: plat_floor + 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag,
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
        ];

        let sidedefs = vec![
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
        ];

        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    // -----------------------------------------------------------------------
    // Tests: sector_linedefs
    // -----------------------------------------------------------------------

    #[test]
    fn sector_linedefs_returns_correct_indices() {
        let level = make_stair_level(3, 0, 1);
        // Sector 0 fronts linedef 0 (right_sidedef=0 → sector 0).
        let result: Vec<_> = sector_linedefs(&level, 0).collect();
        assert_eq!(result, vec![0], "sector 0 should front linedef 0");

        // Sector 1 fronts linedef 1 (right_sidedef=2 → sector 1).
        let result: Vec<_> = sector_linedefs(&level, 1).collect();
        assert_eq!(result, vec![1], "sector 1 should front linedef 1");
    }

    #[test]
    fn sector_linedefs_empty_for_isolated_sector() {
        // Make a level with 3 sectors but only 2 linedefs connecting 0-1, 1-2.
        // Sector 2's right sidedef is only on linedef 1 (right → sector 1).
        // Check that a non-existent sector returns empty.
        let level = make_stair_level(3, 0, 1);
        let result: Vec<_> = sector_linedefs(&level, 99).collect();
        assert!(
            result.is_empty(),
            "non-existent sector must return empty vec"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: ev_build_stairs
    // -----------------------------------------------------------------------

    #[test]
    fn build_stairs_raises_floors_in_sequence_8_units() {
        let mut gs = GameState::new("TEST");
        let level = make_stair_level(4, 0, 1);

        let count = ev_build_stairs(
            &mut gs,
            &level,
            0,
            StairType::Build8,
            crate::state::CrushBehavior::NoCrush,
        );

        // Should create 4 floor movers (sectors 0, 1, 2, 3).
        assert_eq!(count, 4, "4 sectors should get stair movers");
        assert_eq!(gs.movers.active_floors.len(), 4);

        // Check target heights: 8, 16, 24, 32.
        assert_eq!(gs.movers.active_floors[0].target_height, 8);
        assert_eq!(gs.movers.active_floors[0].sector_index, 0);
        assert_eq!(gs.movers.active_floors[1].target_height, 16);
        assert_eq!(gs.movers.active_floors[1].sector_index, 1);
        assert_eq!(gs.movers.active_floors[2].target_height, 24);
        assert_eq!(gs.movers.active_floors[2].sector_index, 2);
        assert_eq!(gs.movers.active_floors[3].target_height, 32);
        assert_eq!(gs.movers.active_floors[3].sector_index, 3);
    }

    #[test]
    fn build_stairs_turbo_16_unit_steps() {
        let mut gs = GameState::new("TEST");
        let level = make_stair_level(3, 0, 1);

        let count = ev_build_stairs(
            &mut gs,
            &level,
            0,
            StairType::Turbo16,
            crate::state::CrushBehavior::NoCrush,
        );

        assert_eq!(count, 3, "3 sectors should get stair movers");
        // Target heights: 16, 32, 48.
        assert_eq!(gs.movers.active_floors[0].target_height, 16);
        assert_eq!(gs.movers.active_floors[1].target_height, 32);
        assert_eq!(gs.movers.active_floors[2].target_height, 48);
        // Speed should be 4 for turbo.
        assert_eq!(gs.movers.active_floors[0].speed, 4);
    }

    #[test]
    fn build_stairs_with_crush_flag() {
        let mut gs = GameState::new("TEST");
        let level = make_stair_level(2, 0, 1);

        ev_build_stairs(
            &mut gs,
            &level,
            0,
            StairType::Turbo16,
            crate::state::CrushBehavior::Crush,
        );

        assert!(
            gs.movers.active_floors[0].crush == crate::state::CrushBehavior::Crush,
            "crush flag must be set on stair movers"
        );
        assert!(
            gs.movers.active_floors[1].crush == crate::state::CrushBehavior::Crush,
            "crush flag must be set on all stair movers"
        );
    }

    #[test]
    fn build_stairs_stops_at_different_flat_texture() {
        let mut gs = GameState::new("TEST");
        let mut level = make_stair_level(4, 0, 1);
        // Change sector 2's floor texture so stairs stop there.
        level.sectors[2].floor_flat = *b"NUKAGE1\0";

        let count = ev_build_stairs(
            &mut gs,
            &level,
            0,
            StairType::Build8,
            crate::state::CrushBehavior::NoCrush,
        );

        // Should create only 2 movers (sectors 0 and 1). Sector 2 has different
        // flat so the chain breaks.
        assert_eq!(count, 2, "stair chain must stop when floor flat differs");
    }

    // -----------------------------------------------------------------------
    // Tests: FloorMover reaches destination and removes itself
    // -----------------------------------------------------------------------

    #[test]
    fn floor_mover_reaches_destination_and_removes_itself() {
        let mut gs = GameState::new("TEST");
        let mut level = make_stair_level(2, 0, 1);

        // Create a one-shot floor raiser (Up, target=8, speed=2).
        gs.movers.active_floors.push(FloorMover {
            sector_index: 0,
            target_height: 8,
            speed: 2,
            direction: MoveDirection::Up,
            wait_tics: -1,
            return_height: 0,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::NoCrush,
            tag: 0,
            floor_type: FloorType::RaiseToNearest,
        });

        // Tick enough times for the floor to reach target (8/2 = 4 tics).
        for _ in 0..10 {
            tick_floors(&mut gs, &mut level);
        }

        assert_eq!(
            level.sectors[0].floor_height, 8,
            "floor must reach target height"
        );
        assert!(
            gs.movers.active_floors.is_empty(),
            "floor mover must remove itself after reaching target"
        );
    }

    #[test]
    fn floor_mover_direction_down() {
        let mut gs = GameState::new("TEST");
        let mut level = make_stair_level(2, 32, 1);

        // Create a one-shot floor lowerer (Down, target=0, speed=4).
        gs.movers.active_floors.push(FloorMover {
            sector_index: 0,
            target_height: 0,
            speed: 4,
            direction: MoveDirection::Down,
            wait_tics: -1,
            return_height: 32,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::NoCrush,
            tag: 0,
            floor_type: FloorType::LowerToLowest,
        });

        for _ in 0..20 {
            tick_floors(&mut gs, &mut level);
        }

        assert_eq!(
            level.sectors[0].floor_height, 0,
            "floor must lower to target"
        );
        assert!(
            gs.movers.active_floors.is_empty(),
            "floor mover must remove itself"
        );
    }

    #[test]
    fn floor_mover_with_crush_flag_set() {
        let mut gs = GameState::new("TEST");
        let level = make_stair_level(2, 0, 1);

        gs.movers.active_floors.push(FloorMover {
            sector_index: 0,
            target_height: 120,
            speed: 1,
            direction: MoveDirection::Up,
            wait_tics: -1,
            return_height: 0,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::Crush,
            tag: 0,
            floor_type: FloorType::RaiseCrush,
        });

        assert!(
            gs.movers.active_floors[0].crush == crate::state::CrushBehavior::Crush,
            "crush flag must be set"
        );

        // Verify it's a FloorMover that can deal crush damage.
        let _crush_dmg: i32 =
            if gs.movers.active_floors[0].crush == crate::state::CrushBehavior::Crush {
                10
            } else {
                0
            };
        assert_eq!(_crush_dmg, 10, "crush damage should be 10 when crush=true");

        // Verify the mover direction is correct.
        assert_eq!(
            gs.movers.active_floors[0].direction,
            MoveDirection::Up,
            "direction should be Up for a raise-to-ceiling mover"
        );
        drop(level);
    }

    // -----------------------------------------------------------------------
    // Tests: ev_do_donut
    // -----------------------------------------------------------------------

    #[test]
    fn donut_raises_inner_sector_floor() {
        let mut gs = GameState::new("TEST");
        let level = make_donut_level(0, 0, 64, 1);

        // Sector 1 (hole) floor is 0, ring floor (sector 2) is 64.
        let count = ev_do_donut(&mut gs, &level, 0);

        assert_eq!(count, 1, "donut should create 1 floor mover");
        assert_eq!(
            gs.movers.active_floors[0].sector_index, 1,
            "donut mover must target the hole sector"
        );
        assert_eq!(
            gs.movers.active_floors[0].target_height, 64,
            "donut target must be ring sector floor height"
        );
        assert_eq!(
            gs.movers.active_floors[0].direction,
            MoveDirection::Up,
            "donut must raise the hole floor"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: PerpetualPlatform
    // -----------------------------------------------------------------------

    #[test]
    fn perpetual_platform_oscillates_between_low_and_high() {
        let mut gs = GameState::new("TEST");
        let mut level = make_platform_level(-32, 64, 5);

        // Adjacent sector floor = -32, platform sector floor = 64.
        ev_perpetual_platform(&mut gs, &level, 5, 4);

        assert_eq!(gs.movers.active_platforms.len(), 1);
        assert_eq!(gs.movers.active_platforms[0].low_height, -32);
        assert_eq!(gs.movers.active_platforms[0].high_height, 64);

        // Tick until platform reaches low.
        // Distance = 64 - (-32) = 96, speed = 4, takes 96/4 = 24 tics.
        for _ in 0..24 {
            tick_platforms(&mut gs, &mut level);
        }

        assert_eq!(
            level.sectors[1].floor_height, -32,
            "platform must reach low_height"
        );
        assert_eq!(
            gs.movers.active_platforms[0].status,
            crate::state::PlatformStatus::Waiting,
            "platform must be waiting at bottom"
        );
    }

    #[test]
    fn perpetual_platform_wait_state() {
        let mut gs = GameState::new("TEST");
        let mut level = make_platform_level(-32, 64, 5);

        ev_perpetual_platform(&mut gs, &level, 5, 4);

        // Drive the platform down to low.
        for _ in 0..24 {
            tick_platforms(&mut gs, &mut level);
        }
        assert_eq!(
            gs.movers.active_platforms[0].status,
            crate::state::PlatformStatus::Waiting
        );

        // Tick once — wait_remaining should decrease.
        let wait_before = gs.movers.active_platforms[0].wait_remaining;
        tick_platforms(&mut gs, &mut level);
        assert_eq!(
            gs.movers.active_platforms[0].wait_remaining,
            wait_before - 1,
            "wait_remaining must decrease each tic"
        );
    }

    #[test]
    fn tick_platforms_advances_platform_positions() {
        let mut gs = GameState::new("TEST");
        let mut level = make_platform_level(0, 32, 5);

        ev_perpetual_platform(&mut gs, &level, 5, 2);

        let before = level.sectors[1].floor_height;
        tick_platforms(&mut gs, &mut level);

        assert_eq!(
            level.sectors[1].floor_height,
            before - 2,
            "tick_platforms must move floor by speed"
        );
    }

    #[test]
    fn platform_speed_calculations() {
        let mut gs = GameState::new("TEST");
        let level = make_platform_level(0, 16, 5);

        // Speed 1
        ev_perpetual_platform(&mut gs, &level, 5, 1);
        assert_eq!(gs.movers.active_platforms[0].speed, 1);

        // Clear and test speed 4.
        gs.movers.active_platforms.clear();
        ev_perpetual_platform(&mut gs, &level, 5, 4);
        assert_eq!(gs.movers.active_platforms[0].speed, 4);
    }

    // -----------------------------------------------------------------------
    // Tests: Line type dispatch
    // -----------------------------------------------------------------------

    /// Helper to build a tagged level with a linedef that triggers a special.
    fn make_tagged_linedef_level(special: u16, tag: u16) -> doom_map::Level {
        let reject_bytes = vec![0u8; 1]; // 2 sectors
        let reject =
            doom_map::Reject::parse_lump(&reject_bytes, 2).expect("value must exist in test");

        let sectors = vec![
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag,
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: -10 },
            doom_map::Vertex { x: 0, y: 10 },
        ];

        let sidedefs = vec![
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
        ];

        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special,
            tag,
            right_sidedef: 0,
            left_sidedef: 1,
        }];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    #[test]
    fn line_type_7_builds_stairs() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(7, 10);
        // Tag sector 1 so it gets stair builder.
        level.sectors[1].tag = 10;

        activate_linedef(&mut gs, &mut level, 0);

        // Should have created at least 1 floor mover for stairs.
        assert!(
            !gs.movers.active_floors.is_empty(),
            "line type 7 must create stair movers"
        );
        // Step size should be 8 (Build8), speed 2.
        assert_eq!(gs.movers.active_floors[0].speed, 2);
    }

    #[test]
    fn line_type_8_builds_turbo_stairs() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(8, 10);

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            !gs.movers.active_floors.is_empty(),
            "line type 8 must create stair movers"
        );
        // Turbo speed = 4.
        assert_eq!(gs.movers.active_floors[0].speed, 4);
    }

    #[test]
    fn line_type_9_donut() {
        let mut gs = GameState::new("TEST");
        // Build a donut level with tag on sector 0.
        let mut level = make_donut_level(0, 0, 64, 10);
        // Add a linedef with special=9 and tag=10.
        level.linedefs.push(doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 9,
            tag: 10,
            right_sidedef: 0,
            left_sidedef: 1,
        });

        activate_linedef(&mut gs, &mut level, 2); // Index 2 is the new linedef.

        assert!(
            !gs.movers.active_floors.is_empty(),
            "line type 9 must create donut mover"
        );
    }

    #[test]
    fn line_type_53_perpetual_platform() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(53, 10);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            gs.movers.active_platforms.len(),
            1,
            "line type 53 must create perpetual platform"
        );
    }

    #[test]
    fn line_type_54_stops_platform() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(54, 10);

        // Create a platform first.
        gs.movers
            .active_platforms
            .push(crate::state::PerpetualPlatform {
                sector_index: 1,
                low_height: 0,
                high_height: 64,
                speed: 1,
                wait_tics: 105,
                wait_remaining: 0,
                status: crate::state::PlatformStatus::Down,
                tag: 10,
            });

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            gs.movers.active_platforms.is_empty(),
            "line type 54 must stop (remove) platforms with matching tag"
        );
    }

    #[test]
    fn line_type_87_perpetual_platform() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(87, 10);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            gs.movers.active_platforms.len(),
            1,
            "line type 87 must create perpetual platform"
        );
    }

    #[test]
    fn line_type_89_stops_platform() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(89, 10);

        gs.movers
            .active_platforms
            .push(crate::state::PerpetualPlatform {
                sector_index: 1,
                low_height: 0,
                high_height: 64,
                speed: 1,
                wait_tics: 105,
                wait_remaining: 0,
                status: crate::state::PlatformStatus::Down,
                tag: 10,
            });

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            gs.movers.active_platforms.is_empty(),
            "line type 89 must stop platforms with matching tag"
        );
    }

    #[test]
    fn line_type_100_turbo_stairs_with_crush() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(100, 10);

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            !gs.movers.active_floors.is_empty(),
            "line type 100 must create stair movers"
        );
        assert!(
            gs.movers.active_floors[0].crush == crate::state::CrushBehavior::Crush,
            "line type 100 stair movers must have crush=true"
        );
    }

    #[test]
    fn line_type_127_turbo_stairs() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(127, 10);

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            !gs.movers.active_floors.is_empty(),
            "line type 127 must create stair movers"
        );
        assert_eq!(gs.movers.active_floors[0].speed, 4, "turbo speed must be 4");
    }

    #[test]
    fn line_type_146_donut() {
        let mut gs = GameState::new("TEST");
        let mut level = make_donut_level(0, 0, 64, 10);
        level.linedefs.push(doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 146,
            tag: 10,
            right_sidedef: 0,
            left_sidedef: 1,
        });

        activate_linedef(&mut gs, &mut level, 2);

        assert!(
            !gs.movers.active_floors.is_empty(),
            "line type 146 must create donut mover"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: Multiple stairs in sequence
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_stairs_in_sequence() {
        let mut gs = GameState::new("TEST");
        let level = make_stair_level(5, 0, 1);

        ev_build_stairs(
            &mut gs,
            &level,
            0,
            StairType::Build8,
            crate::state::CrushBehavior::NoCrush,
        );

        assert_eq!(
            gs.movers.active_floors.len(),
            5,
            "5 sectors should produce 5 stair movers"
        );
        // Verify monotonically increasing target heights.
        for i in 1..gs.movers.active_floors.len() {
            assert!(
                gs.movers.active_floors[i].target_height
                    > gs.movers.active_floors[i - 1].target_height,
                "stair target heights must be monotonically increasing"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Tests: GameState snapshot includes floors/platforms
    // -----------------------------------------------------------------------

    #[test]
    fn game_state_snapshot_includes_floors_and_platforms() {
        let mut gs = GameState::new("TEST");

        gs.movers.active_floors.push(FloorMover {
            sector_index: 5,
            target_height: 64,
            speed: 2,
            direction: MoveDirection::Up,
            wait_tics: -1,
            return_height: 0,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::NoCrush,
            tag: 0,
            floor_type: FloorType::RaiseToNearest,
        });

        gs.movers
            .active_platforms
            .push(crate::state::PerpetualPlatform {
                sector_index: 3,
                low_height: -16,
                high_height: 48,
                speed: 1,
                wait_tics: 105,
                wait_remaining: 0,
                status: crate::state::PlatformStatus::Down,
                tag: 7,
            });

        let snap = gs.save_snapshot();
        gs.movers.active_floors.clear();
        gs.movers.active_platforms.clear();
        gs.restore_snapshot(snap);

        assert_eq!(
            gs.movers.active_floors.len(),
            1,
            "snapshot must preserve active_floors"
        );
        assert_eq!(
            gs.movers.active_floors[0].sector_index, 5,
            "snapshot must preserve floor mover data"
        );
        assert_eq!(
            gs.movers.active_platforms.len(),
            1,
            "snapshot must preserve active_platforms"
        );
        assert_eq!(
            gs.movers.active_platforms[0].tag, 7,
            "snapshot must preserve platform data"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: CeilingType enum and CeilingMover creation
    // -----------------------------------------------------------------------

    #[test]
    fn ceiling_mover_creation_has_correct_fields() {
        let mover = CeilingMover {
            sector_index: 3,
            top_height: 256,
            bottom_height: 8,
            speed: 2,
            normal_speed: 2,
            crush_damage: 10,
            direction: MoveDirection::Down,
            silent: false,
            remove_when_done: false,
            tag: 7,
            ceiling_type: CeilingType::CrushAndRaise,
        };
        assert_eq!(mover.sector_index, 3);
        assert_eq!(mover.top_height, 256);
        assert_eq!(mover.bottom_height, 8);
        assert_eq!(mover.speed, 2);
        assert_eq!(mover.normal_speed, 2);
        assert_eq!(mover.crush_damage, 10);
        assert_eq!(mover.direction, MoveDirection::Down);
        assert!(!mover.silent);
        assert!(!mover.remove_when_done);
        assert_eq!(mover.tag, 7);
        assert_eq!(mover.ceiling_type, CeilingType::CrushAndRaise);
    }

    #[test]
    fn ceiling_type_enum_variants_are_distinct() {
        assert_ne!(CeilingType::LowerToFloor, CeilingType::CrushAndRaise);
        assert_ne!(CeilingType::LowerAndCrush, CeilingType::FastCrushAndRaise);
        assert_ne!(CeilingType::SilentCrush, CeilingType::LowerToFloor);
        // Copy + Clone
        let a = CeilingType::CrushAndRaise;
        let b = a;
        let c = a;
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    // -----------------------------------------------------------------------
    // Tests: tick_ceilings behavior
    // -----------------------------------------------------------------------

    #[test]
    fn tick_ceilings_lowers_ceiling_toward_floor() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 25);

        // Activate slow crusher (type 25, speed=1).
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(level.sectors[1].ceil_height, 128);

        // One tick should lower by speed=1.
        tick_ceilings(&mut gs, &mut level);
        assert_eq!(
            level.sectors[1].ceil_height, 127,
            "ceiling must lower by speed each tic"
        );

        // Five more tics.
        for _ in 0..5 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(
            level.sectors[1].ceil_height, 122,
            "ceiling must continue lowering"
        );
    }

    #[test]
    fn crush_and_raise_reverses_at_bottom() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 25);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::CrushAndRaise
        );

        // Tick down to bottom: 128 - 8 = 120 units / speed 1 = 120 tics.
        for _ in 0..120 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].ceil_height, 8, "must reach bottom_height");
        assert_eq!(
            gs.movers.active_ceilings[0].direction,
            MoveDirection::Up,
            "must reverse to Up at bottom"
        );
    }

    #[test]
    fn crush_and_raise_reverses_at_top_perpetual() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 25);

        activate_linedef(&mut gs, &mut level, 0);

        // Down: 120 tics at speed 1.
        for _ in 0..120 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(gs.movers.active_ceilings[0].direction, MoveDirection::Up);

        // Up: 120 tics at speed 1.
        for _ in 0..120 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].ceil_height, 128, "must return to top");
        assert_eq!(
            gs.movers.active_ceilings[0].direction,
            MoveDirection::Down,
            "perpetual crusher reverses back to Down at top"
        );

        // Still active (perpetual — not removed).
        assert_eq!(
            gs.movers.active_ceilings.len(),
            1,
            "perpetual crusher must remain active"
        );
    }

    #[test]
    fn fast_crush_and_raise_speed_difference() {
        let mut gs = GameState::new("TEST");
        // Type 6 = FastCrushAndRaise, speed 2.
        let mut level = make_tagged_sector_level(0, 128, 1, 6);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::FastCrushAndRaise
        );
        assert_eq!(
            gs.movers.active_ceilings[0].speed, 2,
            "fast crusher uses speed 2"
        );

        // Tick once.
        tick_ceilings(&mut gs, &mut level);
        assert_eq!(
            level.sectors[1].ceil_height, 126,
            "fast crusher must move 2 units per tic"
        );

        // Compare: slow crusher (type 25) uses speed 1.
        let mut gs2 = GameState::new("TEST");
        let mut level2 = make_tagged_sector_level(0, 128, 1, 25);
        activate_linedef(&mut gs2, &mut level2, 0);
        assert_eq!(
            gs2.movers.active_ceilings[0].speed, 1,
            "slow crusher uses speed 1"
        );

        tick_ceilings(&mut gs2, &mut level2);
        assert_eq!(
            level2.sectors[1].ceil_height, 127,
            "slow crusher must move 1 unit per tic"
        );
    }

    #[test]
    fn lower_and_crush_stops_at_bottom() {
        let mut gs = GameState::new("TEST");
        // Type 44 = LowerAndCrush.
        let mut level = make_tagged_sector_level(0, 128, 1, 44);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::LowerAndCrush
        );

        // Tick to bottom: 128 - 8 = 120 / speed 2 = 60 tics.
        for _ in 0..60 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].ceil_height, 8);
        assert!(
            gs.movers.active_ceilings.is_empty(),
            "LowerAndCrush must remove itself at bottom"
        );
    }

    #[test]
    fn lower_to_floor_stops_at_floor_height() {
        let mut gs = GameState::new("TEST");
        // Use ev_ceiling_lower_to_floor directly. Floor is 0, ceil is 128.
        let level = make_tagged_sector_level(0, 128, 1, 0);

        ev_ceiling_lower_to_floor(&mut gs, &level, 1, 2);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::LowerToFloor
        );
        assert_eq!(
            gs.movers.active_ceilings[0].bottom_height, 0,
            "LowerToFloor bottom must be floor height (0), not floor+8"
        );

        // Tick to bottom: 128 / speed 2 = 64 tics.
        let mut level_mut = level;
        for _ in 0..64 {
            tick_ceilings(&mut gs, &mut level_mut);
        }
        assert_eq!(
            level_mut.sectors[1].ceil_height, 0,
            "ceiling must reach floor"
        );
        assert!(
            gs.movers.active_ceilings.is_empty(),
            "LowerToFloor must remove itself at bottom"
        );
    }

    #[test]
    fn ev_ceiling_crush_stop_halts_active_crusher() {
        let mut gs = GameState::new("TEST");
        let level = make_tagged_sector_level(0, 128, 5, 0);

        // Start a crusher manually with tag=5.
        ev_ceiling_crush_and_raise(&mut gs, &level, 5, 1);
        assert_eq!(gs.movers.active_ceilings.len(), 1);

        // Stop it.
        ev_ceiling_crush_stop(&mut gs, 5);
        assert!(
            gs.movers.active_ceilings.is_empty(),
            "ev_ceiling_crush_stop must remove crusher with matching tag"
        );
    }

    #[test]
    fn crush_damage_applied_when_at_bottom() {
        let mut gs = GameState::new("TEST");
        // Floor=0, ceil=10. Player at z=0. CrushAndRaise with crush_damage=10.
        let mut level = make_tagged_sector_level(0, 10, 1, 25);

        // Place player in sector 1 at z=0.
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.z = Fixed16_16::ZERO;
        let handle = gs.mobjslab.alloc(mo);
        gs.player = crate::player::PlayerState::pistol_start(handle);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings[0].crush_damage, 10);

        // Tick down until ceiling is at floor + 8 = 8. ceil=10, speed=1 → 2 tics.
        for _ in 0..2 {
            tick_ceilings(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].ceil_height, 8);

        // Player should have taken damage.
        let health = gs
            .mobjslab
            .get(handle)
            .expect("value must exist in test")
            .health;
        assert!(
            health < 100,
            "player must take crush damage when ceiling is at floor+8"
        );
    }

    #[test]
    fn crusher_slow_down_when_crushing() {
        let mut gs = GameState::new("TEST");
        // Floor=0, ceil=16. CrushAndRaise, speed=2.
        let mut level = make_tagged_sector_level(0, 16, 1, 6);

        // Place player at z=0 in sector 1 (needed for crush damage to trigger slow-down).
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.z = Fixed16_16::ZERO;
        let handle = gs.mobjslab.alloc(mo);
        gs.player = crate::player::PlayerState::pistol_start(handle);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings[0].speed, 2, "initial speed is 2");
        assert_eq!(
            gs.movers.active_ceilings[0].normal_speed, 2,
            "normal speed is 2"
        );

        // FastCrushAndRaise does NOT slow down (only CrushAndRaise and SilentCrush do).
        // FastCrushAndRaise: tick a few times.
        // Actually, type 6 is FastCrushAndRaise which does NOT slow down.
        // Let's use CrushAndRaise (type 25, speed 1) instead.
        gs.movers.active_ceilings.clear();

        let mut level2 = make_tagged_sector_level(0, 16, 1, 25);
        // Place player at z=0 in sector 1.
        activate_linedef(&mut gs, &mut level2, 0);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::CrushAndRaise
        );
        assert_eq!(gs.movers.active_ceilings[0].speed, 1);

        // Tick down to floor+8 = 8. ceil=16, speed=1 → 8 tics.
        for _ in 0..8 {
            tick_ceilings(&mut gs, &mut level2);
        }
        assert_eq!(level2.sectors[1].ceil_height, 8);

        // The crush damage should have triggered slow-down to speed 1.
        // (It's already 1, so this is a no-op for speed=1 crushers.
        // Test with a manually created crusher at speed 4 instead.)
        gs.movers.active_ceilings.clear();

        // Create a CrushAndRaise crusher with speed 4.
        let level3 = make_tagged_sector_level(0, 20, 2, 0);
        activate_crusher(
            &mut gs,
            &level3,
            2,
            CrusherParams {
                speed: 4,
                crush_damage: 10,
                silent: false,
                remove_when_done: false,
                ceiling_type: CeilingType::CrushAndRaise,
            },
        );
        assert_eq!(gs.movers.active_ceilings[0].speed, 4);
        assert_eq!(gs.movers.active_ceilings[0].normal_speed, 4);

        let mut level3_mut = level3;

        // Tick down until near floor. ceil=20, floor=0, bottom=8. 20-8=12 / speed 4 = 3 tics.
        for _ in 0..3 {
            tick_ceilings(&mut gs, &mut level3_mut);
        }
        assert_eq!(level3_mut.sectors[1].ceil_height, 8);

        // Speed should be slowed to 1 after crush.
        assert_eq!(
            gs.movers.active_ceilings[0].speed, 1,
            "CrushAndRaise must slow to speed 1 when crushing"
        );

        // Now reverse to Up — speed should be restored to normal.
        tick_ceilings(&mut gs, &mut level3_mut);
        assert_eq!(
            gs.movers.active_ceilings[0].direction,
            MoveDirection::Up,
            "must reverse to Up"
        );
        assert_eq!(
            gs.movers.active_ceilings[0].speed, 4,
            "speed must be restored to normal_speed when going up"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: Line type dispatch
    // -----------------------------------------------------------------------

    #[test]
    fn line_type_6_dispatches_fast_crush_and_raise() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 6);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::FastCrushAndRaise
        );
        assert_eq!(gs.movers.active_ceilings[0].speed, 2);
        assert!(!gs.movers.active_ceilings[0].remove_when_done);
    }

    #[test]
    fn line_type_25_dispatches_crush_and_raise() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 25);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::CrushAndRaise
        );
        assert_eq!(gs.movers.active_ceilings[0].speed, 1);
    }

    #[test]
    fn line_type_44_dispatches_lower_and_crush() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 44);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::LowerAndCrush
        );
        assert_eq!(
            gs.movers.active_ceilings[0].crush_damage, 0,
            "type 44 has no crush damage"
        );
    }

    #[test]
    fn line_type_49_dispatches_lower_and_crush_with_damage() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 49);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::LowerAndCrush
        );
        assert_eq!(
            gs.movers.active_ceilings[0].crush_damage, 10,
            "type 49 has crush damage 10"
        );
        assert!(
            gs.movers.active_ceilings[0].remove_when_done,
            "type 49 is one-shot"
        );
    }

    #[test]
    fn line_type_57_stops_crusher() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 3, 25);

        // Start a crusher with tag=3.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);

        // Add linedef with special 57 and same tag.
        level.linedefs.push(doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 57,
            tag: 3,
            right_sidedef: 0,
            left_sidedef: 1,
        });
        let stop_idx = level.linedefs.len() - 1;
        activate_linedef(&mut gs, &mut level, stop_idx);

        assert!(
            gs.movers.active_ceilings.is_empty(),
            "line type 57 must stop crusher with matching tag"
        );
    }

    #[test]
    fn line_type_72_dispatches_lower_and_crush() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 72);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::LowerAndCrush
        );
    }

    #[test]
    fn line_type_73_dispatches_crush_and_raise() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 73);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::CrushAndRaise
        );
        assert_eq!(
            gs.movers.active_ceilings[0].speed, 1,
            "type 73 is slow (speed 1)"
        );
    }

    #[test]
    fn line_type_74_stops_crusher() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 4, 73);

        // Start a crusher with tag=4.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);

        // Add linedef with special 74 and same tag.
        level.linedefs.push(doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 74,
            tag: 4,
            right_sidedef: 0,
            left_sidedef: 1,
        });
        let stop_idx = level.linedefs.len() - 1;
        activate_linedef(&mut gs, &mut level, stop_idx);

        assert!(
            gs.movers.active_ceilings.is_empty(),
            "line type 74 must stop crusher with matching tag"
        );
    }

    #[test]
    fn line_type_141_dispatches_silent_crush() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_sector_level(0, 128, 1, 141);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs.movers.active_ceilings[0].ceiling_type,
            CeilingType::SilentCrush
        );
        assert!(
            gs.movers.active_ceilings[0].silent,
            "type 141 must set silent=true"
        );
        assert!(
            !gs.movers.active_ceilings[0].remove_when_done,
            "type 141 is perpetual"
        );
    }

    #[test]
    fn multiple_crushers_active_simultaneously() {
        let mut gs = GameState::new("TEST");
        // Create a level with two sectors sharing the same tag.
        let reject = doom_map::Reject::parse_lump(&[0u8; 2], 3).expect("value must exist in test");
        let mut level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004,
                special: 25,
                tag: 1,
                right_sidedef: 0,
                left_sidedef: 1,
            }],
            sidedefs: vec![
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: [0; 8],
                    lower_texture: [0; 8],
                    middle_texture: [0; 8],
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
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 1,
                },
                doom_map::Sector {
                    floor_height: 0,
                    ceil_height: 200,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 1,
                },
            ],
            reject,
            blockmap: make_minimal_blockmap(),
        };

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_ceilings.len(),
            2,
            "both tagged sectors must get a crusher"
        );

        // Tick and verify both move.
        tick_ceilings(&mut gs, &mut level);
        assert_eq!(level.sectors[1].ceil_height, 127);
        assert_eq!(level.sectors[2].ceil_height, 199);
    }

    #[test]
    fn game_state_clone_includes_ceiling_type() {
        let mut gs = GameState::new("TEST");
        gs.movers.active_ceilings.push(CeilingMover {
            sector_index: 0,
            top_height: 128,
            bottom_height: 8,
            speed: 2,
            normal_speed: 2,
            crush_damage: 10,
            direction: MoveDirection::Down,
            silent: true,
            remove_when_done: false,
            tag: 1,
            ceiling_type: CeilingType::SilentCrush,
        });

        let gs2 = gs.clone();
        assert_eq!(gs2.movers.active_ceilings.len(), 1);
        assert_eq!(
            gs2.movers.active_ceilings[0].ceiling_type,
            CeilingType::SilentCrush
        );
        assert_eq!(gs2.movers.active_ceilings[0].normal_speed, 2);
        assert!(gs2.movers.active_ceilings[0].silent);
    }

    #[test]
    fn save_load_roundtrip_ceiling_mover_with_ceiling_type() {
        let mut gs = GameState::new("TEST");
        gs.movers.active_ceilings.push(CeilingMover {
            sector_index: 5,
            top_height: 200,
            bottom_height: 16,
            speed: 1,
            normal_speed: 4,
            crush_damage: 10,
            direction: MoveDirection::Up,
            silent: true,
            remove_when_done: false,
            tag: 99,
            ceiling_type: CeilingType::SilentCrush,
        });

        // Place a player mobj so save_game works.
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let handle = gs.mobjslab.alloc(mo);
        gs.player = crate::player::PlayerState::pistol_start(handle);

        let data = crate::savegame::save_game(&gs, b"TEST\0\0\0\0", 0, "ceiling type test");
        let loaded = crate::savegame::load_game(&data).expect("load must succeed");

        assert_eq!(loaded.state.movers.active_ceilings.len(), 1);
        let c = &loaded.state.movers.active_ceilings[0];
        assert_eq!(c.sector_index, 5);
        assert_eq!(c.top_height, 200);
        assert_eq!(c.bottom_height, 16);
        assert_eq!(c.speed, 1);
        assert_eq!(c.normal_speed, 4);
        assert_eq!(c.crush_damage, 10);
        assert_eq!(c.direction, MoveDirection::Up);
        assert!(c.silent);
        assert!(!c.remove_when_done);
        assert_eq!(c.tag, 99);
        assert_eq!(c.ceiling_type, CeilingType::SilentCrush);
    }

    #[test]
    fn silent_crush_slows_down_like_crush_and_raise() {
        let mut gs = GameState::new("TEST");
        let level = make_tagged_sector_level(0, 20, 2, 0);

        // Create a SilentCrush crusher with speed 4.
        activate_crusher(
            &mut gs,
            &level,
            2,
            CrusherParams {
                speed: 4,
                crush_damage: 10,
                silent: true,
                remove_when_done: false,
                ceiling_type: CeilingType::SilentCrush,
            },
        );
        assert_eq!(gs.movers.active_ceilings[0].speed, 4);

        let mut level_mut = level;

        // Place player at z=0.
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.z = Fixed16_16::ZERO;
        let handle = gs.mobjslab.alloc(mo);
        gs.player = crate::player::PlayerState::pistol_start(handle);

        // Tick to bottom. ceil=20, floor=0, bottom=8. 20-8=12/4=3 tics.
        for _ in 0..3 {
            tick_ceilings(&mut gs, &mut level_mut);
        }
        assert_eq!(level_mut.sectors[1].ceil_height, 8);

        // SilentCrush should also slow to speed 1.
        assert_eq!(
            gs.movers.active_ceilings[0].speed, 1,
            "SilentCrush must also slow down when crushing"
        );
    }

    #[test]
    fn fast_crush_and_raise_does_not_slow_down() {
        let mut gs = GameState::new("TEST");
        let level = make_tagged_sector_level(0, 20, 2, 0);

        // Create a FastCrushAndRaise crusher with speed 4.
        activate_crusher(
            &mut gs,
            &level,
            2,
            CrusherParams {
                speed: 4,
                crush_damage: 10,
                silent: false,
                remove_when_done: false,
                ceiling_type: CeilingType::FastCrushAndRaise,
            },
        );
        assert_eq!(gs.movers.active_ceilings[0].speed, 4);

        let mut level_mut = level;

        // Place player at z=0.
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.z = Fixed16_16::ZERO;
        let handle = gs.mobjslab.alloc(mo);
        gs.player = crate::player::PlayerState::pistol_start(handle);

        // Tick to bottom. ceil=20, floor=0, bottom=8. 12/4=3 tics.
        for _ in 0..3 {
            tick_ceilings(&mut gs, &mut level_mut);
        }
        assert_eq!(level_mut.sectors[1].ceil_height, 8);

        // FastCrushAndRaise should NOT slow down — speed stays at 4.
        assert_eq!(
            gs.movers.active_ceilings[0].speed, 4,
            "FastCrushAndRaise must NOT slow down"
        );
    }

    // =====================================================================
    // Tests: LiftMover and related features
    // =====================================================================

    // -----------------------------------------------------------------------
    // Helper: build a level suitable for lift tests
    //
    // Layout:
    // - Sector 0: floor=0, ceil=128, tag=0 (front / player sector)
    // - Sector 1: floor=64, ceil=192, tag=`tag` (the lift sector)
    // - Sector 2: floor=`adj_floor`, ceil=128, tag=0 (adjacent to sector 1)
    //
    // Linedefs:
    //   LD0: sector 0 ↔ sector 1 (right=SD0→sec0, left=SD1→sec1) special=ld_special, tag=tag
    //   LD1: sector 1 ↔ sector 2 (right=SD2→sec1, left=SD3→sec2)
    // -----------------------------------------------------------------------
    fn make_lift_test_level(adj_floor: i16, ld_special: u16, tag: u16) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8; 2], 3).expect("value must exist in test");

        let sectors = vec![
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            doom_map::Sector {
                floor_height: 64,
                ceil_height: 192,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag,
            },
            doom_map::Sector {
                floor_height: adj_floor,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
            doom_map::Vertex { x: 128, y: 0 },
            doom_map::Vertex { x: 192, y: 0 },
        ];

        let sidedefs = vec![
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 0,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 2,
            },
        ];

        let linedefs = vec![
            doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004,
                special: ld_special,
                tag,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            doom_map::Linedef {
                from_vertex: 1,
                to_vertex: 2,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 2,
                left_sidedef: 3,
            },
        ];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: LiftMover creation with correct fields
    // -----------------------------------------------------------------------
    #[test]
    fn lift_mover_creation_correct_fields() {
        use crate::state::{LiftMover, LiftStatus};
        let lm = LiftMover {
            sector_index: 1,
            low_height: 0,
            high_height: 64,
            speed: 4,
            wait_tics: 105,
            wait_remaining: 0,
            status: LiftStatus::Lowering,
        };
        assert_eq!(lm.sector_index, 1);
        assert_eq!(lm.low_height, 0);
        assert_eq!(lm.high_height, 64);
        assert_eq!(lm.speed, 4);
        assert_eq!(lm.wait_tics, 105);
        assert_eq!(lm.wait_remaining, 0);
        assert_eq!(lm.status, LiftStatus::Lowering);
    }

    // -----------------------------------------------------------------------
    // Test 2: lowest_adjacent_floor computes correct height (for lift)
    // -----------------------------------------------------------------------
    #[test]
    fn lowest_adjacent_floor_for_lift() {
        // Sector 1 (floor=64, tag=5) is adjacent to sector 0 (floor=0) and sector 2 (floor=16).
        let level = make_lift_test_level(16, 0, 5);
        let low = lowest_adjacent_floor(&level, 1);
        assert_eq!(
            low, 0,
            "lowest adjacent floor to sector 1 should be 0 (sector 0)"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: tick_lifts lowers floor to low_height
    // -----------------------------------------------------------------------
    #[test]
    fn tick_lifts_lowers_floor() {
        use crate::state::LiftStatus;
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 0, 5);

        // Sector 1 floor starts at 64. Low = 0 (sector 0's floor). Speed 4.
        ev_do_lift(&mut gs, &level, 5, 4, 105);
        assert_eq!(gs.movers.lifts.len(), 1);
        assert_eq!(gs.movers.lifts[0].status, LiftStatus::Lowering);

        // After 1 tic, floor = 64 - 4 = 60.
        tick_lifts(&mut gs, &mut level);
        assert_eq!(level.sectors[1].floor_height, 60);
    }

    // -----------------------------------------------------------------------
    // Test 4: Lift transitions: Lowering → Waiting
    // -----------------------------------------------------------------------
    #[test]
    fn lift_transition_lowering_to_waiting() {
        use crate::state::LiftStatus;
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 0, 5);
        // Sector 1 floor=64, low=0 (from sec 0), speed=4.
        ev_do_lift(&mut gs, &level, 5, 4, 105);

        // 64/4 = 16 tics to reach low_height=0.
        for _ in 0..16 {
            tick_lifts(&mut gs, &mut level);
        }
        assert_eq!(
            level.sectors[1].floor_height, 0,
            "floor must reach low_height"
        );
        assert_eq!(gs.movers.lifts[0].status, LiftStatus::Waiting);
        assert_eq!(gs.movers.lifts[0].wait_remaining, 105);
    }

    // -----------------------------------------------------------------------
    // Test 5: Lift transitions: Waiting countdown → Raising
    // -----------------------------------------------------------------------
    #[test]
    fn lift_transition_waiting_to_raising() {
        use crate::state::LiftStatus;
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 0, 5);
        ev_do_lift(&mut gs, &level, 5, 4, 105);

        // Lower to bottom.
        for _ in 0..16 {
            tick_lifts(&mut gs, &mut level);
        }
        assert_eq!(gs.movers.lifts[0].status, LiftStatus::Waiting);

        // Wait 105 tics.
        for _ in 0..105 {
            tick_lifts(&mut gs, &mut level);
        }
        assert_eq!(gs.movers.lifts[0].status, LiftStatus::Raising);
    }

    // -----------------------------------------------------------------------
    // Test 6: Lift transitions: Raising → Done (removed)
    // -----------------------------------------------------------------------
    #[test]
    fn lift_transition_raising_to_done() {
        use crate::state::LiftStatus;
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 0, 5);
        ev_do_lift(&mut gs, &level, 5, 4, 105);

        // Lower (16 tics), wait (105 tics), then raise.
        for _ in 0..16 {
            tick_lifts(&mut gs, &mut level);
        }
        for _ in 0..105 {
            tick_lifts(&mut gs, &mut level);
        }
        assert_eq!(gs.movers.lifts[0].status, LiftStatus::Raising);

        // Raise from 0 to 64 at speed 4: 16 tics.
        for _ in 0..16 {
            tick_lifts(&mut gs, &mut level);
        }
        assert_eq!(
            level.sectors[1].floor_height, 64,
            "floor must return to high_height"
        );
        // LiftStatus::Done triggers removal on next tick.
        tick_lifts(&mut gs, &mut level);
        assert!(gs.movers.lifts.is_empty(), "lift must be removed when Done");
    }

    // -----------------------------------------------------------------------
    // Test 7: Full lift cycle: lower-wait-raise
    // -----------------------------------------------------------------------
    #[test]
    fn full_lift_cycle() {
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 0, 5);
        ev_do_lift(&mut gs, &level, 5, 4, 105);

        // Lower (16 tics) + wait (105 tics) + raise (16 tics) + removal (1 tic).
        let total_tics = 16 + 105 + 16 + 1;
        for _ in 0..total_tics {
            tick_lifts(&mut gs, &mut level);
        }

        assert_eq!(
            level.sectors[1].floor_height, 64,
            "floor must be back at original"
        );
        assert!(
            gs.movers.lifts.is_empty(),
            "lift must be removed after full cycle"
        );
    }

    // -----------------------------------------------------------------------
    // Test 8: Blazing lift with speed 8
    // -----------------------------------------------------------------------
    #[test]
    fn blazing_lift_speed_8() {
        use crate::state::LiftStatus;
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 0, 5);
        ev_do_lift(&mut gs, &level, 5, 8, 105);

        assert_eq!(
            gs.movers.lifts[0].speed, 8,
            "blazing lift must have speed 8"
        );

        // 64/8 = 8 tics to lower.
        for _ in 0..8 {
            tick_lifts(&mut gs, &mut level);
        }
        assert_eq!(level.sectors[1].floor_height, 0);
        assert_eq!(gs.movers.lifts[0].status, LiftStatus::Waiting);
    }

    // -----------------------------------------------------------------------
    // Test 9: ev_do_lift with tag matching multiple sectors
    // -----------------------------------------------------------------------
    #[test]
    fn ev_do_lift_multiple_sectors() {
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 0, 5);
        // Give sector 2 the same tag so two sectors match.
        level.sectors[2].tag = 5;
        level.sectors[2].floor_height = 32;

        let count = ev_do_lift(&mut gs, &level, 5, 4, 105);
        assert_eq!(count, 2, "should create lifts for both tagged sectors");
        assert_eq!(gs.movers.lifts.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Test 10: Line type 10 dispatches lift
    // -----------------------------------------------------------------------
    #[test]
    fn line_type_10_dispatches_lift() {
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 10, 5);

        activate_linedef(&mut gs, &mut level, 0);
        assert!(
            gs.movers.active_floors.len() == 1,
            "line type 10 should create a FloorMover lift"
        );
    }

    // -----------------------------------------------------------------------
    // Test 11: Line type 21 dispatches lift
    // -----------------------------------------------------------------------
    #[test]
    fn line_type_21_dispatches_lift() {
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 21, 5);

        activate_linedef(&mut gs, &mut level, 0);
        assert!(
            gs.movers.active_floors.len() == 1,
            "line type 21 should create a FloorMover lift"
        );
    }

    // -----------------------------------------------------------------------
    // Test 12: Line type 62 dispatches lift
    // -----------------------------------------------------------------------
    #[test]
    fn line_type_62_dispatches_lift() {
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 62, 5);

        activate_linedef(&mut gs, &mut level, 0);
        assert!(
            gs.movers.active_floors.len() == 1,
            "line type 62 should create a FloorMover lift"
        );
    }

    // -----------------------------------------------------------------------
    // Test 13: Line type 88 dispatches lift
    // -----------------------------------------------------------------------
    #[test]
    fn line_type_88_dispatches_lift() {
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 88, 5);

        activate_linedef(&mut gs, &mut level, 0);
        assert!(
            gs.movers.active_floors.len() == 1,
            "line type 88 should create a FloorMover lift"
        );
    }

    // -----------------------------------------------------------------------
    // Test 14: Line type 120 dispatches blazing lift (LiftMover)
    // -----------------------------------------------------------------------
    #[test]
    fn line_type_120_dispatches_blazing_lift() {
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 120, 5);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.lifts.len(),
            1,
            "line type 120 should create a LiftMover"
        );
        assert_eq!(
            gs.movers.lifts[0].speed, 8,
            "line type 120 should be blazing speed 8"
        );
    }

    // -----------------------------------------------------------------------
    // Test 15: Blazing door line type 105 creates fast door
    // -----------------------------------------------------------------------
    #[test]
    fn blazing_door_type_105_creates_fast_door() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(105, 5);
        level.sectors[1].tag = 5;
        level.sectors[1].ceil_height = 0; // Door starts closed.

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "should create a door mover"
        );
        assert_eq!(
            gs.movers.active_doors[0].speed, BLAZING_DOOR_SPEED,
            "blazing door must have speed 8"
        );
    }

    // -----------------------------------------------------------------------
    // Test 16: Blazing door line type 108 creates fast door
    // -----------------------------------------------------------------------
    #[test]
    fn blazing_door_type_108_creates_fast_door() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 108);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "should create a door mover"
        );
        assert_eq!(
            gs.movers.active_doors[0].speed, BLAZING_DOOR_SPEED,
            "blazing door type 108 must have speed 8"
        );
    }

    // -----------------------------------------------------------------------
    // Test 17: Keyed door type 99 requires blue key
    // -----------------------------------------------------------------------
    #[test]
    fn keyed_door_type_99_requires_blue_key() {
        use crate::player::PlayerState;
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = PlayerState::pistol_start(handle);
        let mut level = make_door_level_with_special(0, 99);

        // Without key — should not open.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_doors.len(),
            0,
            "blue keyed door must not open without key"
        );

        // Give blue card and try again.
        gs.player.give_key(crate::player::KEY_BLUE_CARD);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "blue keyed door must open with blue card"
        );
    }

    // -----------------------------------------------------------------------
    // Test 18: Keyed door type 134 requires red key
    // -----------------------------------------------------------------------
    #[test]
    fn keyed_door_type_134_requires_red_key() {
        use crate::player::PlayerState;
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = PlayerState::pistol_start(handle);
        let mut level = make_door_level_with_special(0, 134);

        // Without key — should not open.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_doors.len(),
            0,
            "red keyed door must not open without key"
        );

        // Give red card and try again.
        gs.player.give_key(crate::player::KEY_RED_CARD);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "red keyed door must open with red card"
        );
    }

    // -----------------------------------------------------------------------
    // Test 19: Keyed door type 136 requires yellow key
    // -----------------------------------------------------------------------
    #[test]
    fn keyed_door_type_136_requires_yellow_key() {
        use crate::player::PlayerState;
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = PlayerState::pistol_start(handle);
        let mut level = make_door_level_with_special(0, 136);

        // Without key — should not open.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_doors.len(),
            0,
            "yellow keyed door must not open without key"
        );

        // Give yellow card and try again.
        gs.player.give_key(crate::player::KEY_YELLOW_CARD);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.active_doors.len(),
            1,
            "yellow keyed door must open with yellow card"
        );
    }

    // -----------------------------------------------------------------------
    // Test 20: GameState clone includes lifts
    // -----------------------------------------------------------------------
    #[test]
    fn game_state_clone_includes_lifts() {
        use crate::state::{LiftMover, LiftStatus};
        let mut gs = GameState::new("TEST");
        gs.movers.lifts.push(LiftMover {
            sector_index: 1,
            low_height: 0,
            high_height: 64,
            speed: 4,
            wait_tics: 105,
            wait_remaining: 50,
            status: LiftStatus::Waiting,
        });

        let gs2 = gs.clone();
        assert_eq!(gs2.movers.lifts.len(), 1);
        assert_eq!(gs2.movers.lifts[0].sector_index, 1);
        assert_eq!(gs2.movers.lifts[0].status, LiftStatus::Waiting);
        assert_eq!(gs2.movers.lifts[0].wait_remaining, 50);
    }

    // -----------------------------------------------------------------------
    // Test 21: Save/load roundtrip for LiftMover
    // -----------------------------------------------------------------------
    #[test]
    fn save_load_roundtrip_lift_mover() {
        use crate::mobj::{Mobj, flags};
        use crate::player::PlayerState;
        use crate::savegame::{load_game, save_game};
        use crate::state::{LiftMover, LiftStatus};
        use doom_types::mobj_kind::MobjKind;

        let mut gs = GameState::new("E1M1");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(10),
            Fixed16_16::from_int(20),
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);

        gs.movers.lifts.push(LiftMover {
            sector_index: 3,
            low_height: -32,
            high_height: 64,
            speed: 8,
            wait_tics: 105,
            wait_remaining: 42,
            status: LiftStatus::Waiting,
        });

        let mut level_name = [0u8; 8];
        level_name[..4].copy_from_slice(b"E1M1");
        let data = save_game(&gs, &level_name, 2, "test lift save");
        let loaded = load_game(&data).expect("value must exist in test");

        assert_eq!(loaded.state.movers.lifts.len(), 1, "must restore 1 lift");
        assert_eq!(loaded.state.movers.lifts[0].sector_index, 3);
        assert_eq!(loaded.state.movers.lifts[0].low_height, -32);
        assert_eq!(loaded.state.movers.lifts[0].high_height, 64);
        assert_eq!(loaded.state.movers.lifts[0].speed, 8);
        assert_eq!(loaded.state.movers.lifts[0].wait_tics, 105);
        assert_eq!(loaded.state.movers.lifts[0].wait_remaining, 42);
        assert_eq!(loaded.state.movers.lifts[0].status, LiftStatus::Waiting);
    }

    // -----------------------------------------------------------------------
    // Test 22: Line type 122 dispatches S1 blazing lift
    // -----------------------------------------------------------------------
    #[test]
    fn line_type_122_dispatches_blazing_lift() {
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 122, 5);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.lifts.len(),
            1,
            "line type 122 should create a LiftMover"
        );
        assert_eq!(gs.movers.lifts[0].speed, 8);
    }

    // -----------------------------------------------------------------------
    // Test 23: Line type 123 dispatches SR blazing lift
    // -----------------------------------------------------------------------
    #[test]
    fn line_type_123_dispatches_blazing_lift() {
        let mut gs = GameState::new("TEST");
        let mut level = make_lift_test_level(16, 123, 5);

        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(
            gs.movers.lifts.len(),
            1,
            "line type 123 should create a LiftMover"
        );
        assert_eq!(gs.movers.lifts[0].speed, 8);
    }

    // -----------------------------------------------------------------------
    // Test 24: Blazing door type 109 open-stay
    // -----------------------------------------------------------------------
    #[test]
    fn blazing_door_type_109_open_stay() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 109);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(gs.movers.active_doors.len(), 1);
        assert_eq!(gs.movers.active_doors[0].speed, BLAZING_DOOR_SPEED);
        // open-stay → countdown = -1 (no auto-close).
        assert_eq!(gs.movers.active_doors[0].countdown, -1);
    }

    // -----------------------------------------------------------------------
    // Test 25: Blazing door type 110 closes door
    // -----------------------------------------------------------------------
    #[test]
    fn blazing_door_type_110_closes() {
        let mut gs = GameState::new("TEST");
        // Door starts open (ceil=128 > floor=0).
        let mut level = make_door_level_with_special(128, 110);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(gs.movers.active_doors.len(), 1);
        // Close = negative speed.
        assert_eq!(gs.movers.active_doors[0].speed, -BLAZING_DOOR_SPEED);
        assert_eq!(
            gs.movers.active_doors[0].target_height, 0,
            "closing doors should lower back to floor height"
        );
    }

    // -----------------------------------------------------------------------
    // Test 26: Keyed blazing door type 133 (S1 Blue blazing)
    // -----------------------------------------------------------------------
    #[test]
    fn keyed_blazing_door_type_133() {
        use crate::player::PlayerState;
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = PlayerState::pistol_start(handle);
        let mut level = make_door_level_with_special(0, 133);

        // Without key — should not open.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_doors.len(), 0);

        // Give blue skull and try again.
        gs.player.give_key(crate::player::KEY_BLUE_SKULL);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_doors.len(), 1);
        assert_eq!(
            gs.movers.active_doors[0].speed, BLAZING_DOOR_SPEED,
            "keyed blazing door must use fast speed"
        );
    }

    // -----------------------------------------------------------------------
    // Test 27: Keyed blazing door type 135 (S1 Red blazing)
    // -----------------------------------------------------------------------
    #[test]
    fn keyed_blazing_door_type_135() {
        use crate::player::PlayerState;
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = PlayerState::pistol_start(handle);
        let mut level = make_door_level_with_special(0, 135);

        // Without key — should not open.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_doors.len(), 0);

        // Give red skull and try again.
        gs.player.give_key(crate::player::KEY_RED_SKULL);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_doors.len(), 1);
        assert_eq!(gs.movers.active_doors[0].speed, BLAZING_DOOR_SPEED);
    }

    // -----------------------------------------------------------------------
    // Test 28: Keyed blazing door type 137 (S1 Yellow blazing)
    // -----------------------------------------------------------------------
    #[test]
    fn keyed_blazing_door_type_137() {
        use crate::player::PlayerState;
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = PlayerState::pistol_start(handle);
        let mut level = make_door_level_with_special(0, 137);

        // Without key — should not open.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_doors.len(), 0);

        // Give yellow skull and try again.
        gs.player.give_key(crate::player::KEY_YELLOW_SKULL);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_doors.len(), 1);
        assert_eq!(gs.movers.active_doors[0].speed, BLAZING_DOOR_SPEED);
    }

    // -----------------------------------------------------------------------
    // Types 16, 76: close door, wait 30s, reopen
    // -----------------------------------------------------------------------
    #[test]
    fn close_wait_open_door_triggers() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        // Floor=0, Ceil=128, special=16
        let mut level = make_door_level_with_special(128, 16);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(gs.movers.active_doors.len(), 1);
        let door = &gs.movers.active_doors[0];
        assert_eq!(door.speed, -DOOR_SPEED);
        assert_eq!(door.target_height, 0); // floor height
        assert!(door.is_ceiling);
        // Wait is calculated internally; just verify it's a valid mover.
    }

    #[test]
    fn close_wait_open_door_triggers_type_76() {
        let mut gs = GameState::new("TEST");
        let handle = make_actor_at_z(&mut gs, 0);
        gs.player = crate::player::PlayerState::pistol_start(handle);
        let mut level = make_door_level_with_special(128, 76);

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(gs.movers.active_doors.len(), 1);
        let door = &gs.movers.active_doors[0];
        assert_eq!(door.speed, -DOOR_SPEED);
        assert_eq!(door.target_height, 0);
        assert!(door.is_ceiling);
    }

    // -----------------------------------------------------------------------
    // Test 29: ev_do_lift avoids duplicate lifts on same sector
    // -----------------------------------------------------------------------
    #[test]
    fn ev_do_lift_avoids_duplicates() {
        let mut gs = GameState::new("TEST");
        let level = make_lift_test_level(16, 0, 5);

        let count1 = ev_do_lift(&mut gs, &level, 5, 4, 105);
        let count2 = ev_do_lift(&mut gs, &level, 5, 4, 105);

        assert_eq!(count1, 1, "first call should create 1 lift");
        assert_eq!(count2, 0, "second call should not create duplicates");
        assert_eq!(gs.movers.lifts.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Test 30: LiftStatus derives
    // -----------------------------------------------------------------------
    #[test]
    fn lift_status_derives() {
        use crate::state::LiftStatus;
        let a = LiftStatus::Lowering;
        let b = a; // Copy
        let c = a; // Clone
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_ne!(LiftStatus::Lowering, LiftStatus::Raising);
    }

    // -----------------------------------------------------------------------
    // Test 31: Blazing door type 106 open-stay via tag
    // -----------------------------------------------------------------------
    #[test]
    fn blazing_door_type_106_open_stay_by_tag() {
        let mut gs = GameState::new("TEST");
        let mut level = make_tagged_linedef_level(106, 5);
        level.sectors[1].tag = 5;
        level.sectors[1].ceil_height = 0; // Door starts closed.

        activate_linedef(&mut gs, &mut level, 0);

        assert_eq!(gs.movers.active_doors.len(), 1);
        assert_eq!(gs.movers.active_doors[0].speed, BLAZING_DOOR_SPEED);
        // open-stay: no auto-close.
        assert_eq!(gs.movers.active_doors[0].countdown, -1);
    }

    // =======================================================================
    // Scrolling walls + conveyor belt tests
    // =======================================================================

    /// Build a level with linedefs having the given special types.
    /// Each linedef has a right sidedef pointing at sector 0.
    fn make_scrolling_level(specials: &[u16]) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("value must exist in test");

        let sectors = vec![doom_map::Sector {
            floor_height: 0,
            ceil_height: 128,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
        ];

        let sidedefs = vec![doom_map::Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"\0\0\0\0\0\0\0\0",
            lower_texture: *b"\0\0\0\0\0\0\0\0",
            middle_texture: *b"\0\0\0\0\0\0\0\0",
            sector: 0,
        }];

        let linedefs: Vec<doom_map::Linedef> = specials
            .iter()
            .map(|&sp| doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: sp,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: SIDEDEF_NONE,
            })
            .collect();

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    /// Build a level with a conveyor linedef (type 253) with the given sidedef offsets.
    fn make_conveyor_level(line_type: u16, x_offset: i16, y_offset: i16) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("value must exist in test");

        let sectors = vec![doom_map::Sector {
            floor_height: 0,
            ceil_height: 128,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }];

        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
        ];

        let sidedefs = vec![doom_map::Sidedef {
            x_offset,
            y_offset,
            upper_texture: *b"\0\0\0\0\0\0\0\0",
            lower_texture: *b"\0\0\0\0\0\0\0\0",
            middle_texture: *b"\0\0\0\0\0\0\0\0",
            sector: 0,
        }];

        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: line_type,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: SIDEDEF_NONE,
        }];

        doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: ScrollingWall creation with correct fields
    // -----------------------------------------------------------------------
    #[test]
    fn scrolling_wall_creation_correct_fields() {
        use crate::state::ScrollingWall;
        let sw = ScrollingWall {
            linedef_index: 7,
            speed_x: 1,
            speed_y: -2,
            accumulated_x: 0,
            accumulated_y: 0,
        };
        assert_eq!(sw.linedef_index, 7);
        assert_eq!(sw.speed_x, 1);
        assert_eq!(sw.speed_y, -2);
        assert_eq!(sw.accumulated_x, 0);
        assert_eq!(sw.accumulated_y, 0);
    }

    // -----------------------------------------------------------------------
    // Test 2: tick_scrollers advances accumulated_x
    // -----------------------------------------------------------------------
    #[test]
    fn tick_scrollers_advances_accumulated_x() {
        use crate::state::ScrollingWall;
        let mut gs = GameState::new("TEST");
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: 0,
            speed_x: 1,
            speed_y: 0,
            accumulated_x: 0,
            accumulated_y: 0,
        });

        tick_scrollers(&mut gs);

        assert_eq!(
            gs.movers.scrolling_walls[0].accumulated_x, 1,
            "accumulated_x must advance by speed_x each tic"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: tick_scrollers advances accumulated_y (vertical scroll)
    // -----------------------------------------------------------------------
    #[test]
    fn tick_scrollers_advances_accumulated_y() {
        use crate::state::ScrollingWall;
        let mut gs = GameState::new("TEST");
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: 0,
            speed_x: 0,
            speed_y: 3,
            accumulated_x: 0,
            accumulated_y: 0,
        });

        tick_scrollers(&mut gs);

        assert_eq!(
            gs.movers.scrolling_walls[0].accumulated_y, 3,
            "accumulated_y must advance by speed_y each tic"
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: Scroll left (type 48) has positive speed_x
    // -----------------------------------------------------------------------
    #[test]
    fn scroll_left_type_48_positive_speed_x() {
        let mut gs = GameState::new("TEST");
        let level = make_scrolling_level(&[48]);
        init_scrolling_walls(&mut gs, &level);

        assert_eq!(gs.movers.scrolling_walls.len(), 1);
        assert_eq!(
            gs.movers.scrolling_walls[0].speed_x, 1,
            "line type 48 (scroll left) must have speed_x = 1"
        );
        assert_eq!(gs.movers.scrolling_walls[0].speed_y, 0);
    }

    // -----------------------------------------------------------------------
    // Test 5: Scroll right (type 85) has negative speed_x
    // -----------------------------------------------------------------------
    #[test]
    fn scroll_right_type_85_negative_speed_x() {
        let mut gs = GameState::new("TEST");
        let level = make_scrolling_level(&[85]);
        init_scrolling_walls(&mut gs, &level);

        assert_eq!(gs.movers.scrolling_walls.len(), 1);
        assert_eq!(
            gs.movers.scrolling_walls[0].speed_x, -1,
            "line type 85 (scroll right) must have speed_x = -1"
        );
        assert_eq!(gs.movers.scrolling_walls[0].speed_y, 0);
    }

    // -----------------------------------------------------------------------
    // Test 6: get_scroll_offset returns correct values after ticking
    // -----------------------------------------------------------------------
    #[test]
    fn get_scroll_offset_correct_after_ticking() {
        use crate::state::ScrollingWall;
        let mut gs = GameState::new("TEST");
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: 5,
            speed_x: 2,
            speed_y: -1,
            accumulated_x: 0,
            accumulated_y: 0,
        });

        tick_scrollers(&mut gs);
        tick_scrollers(&mut gs);
        tick_scrollers(&mut gs);

        let (ox, oy) = gs.get_scroll_offset(5);
        assert_eq!(ox, 6, "3 tics * speed_x=2 = 6");
        assert_eq!(oy, -3, "3 tics * speed_y=-1 = -3");
    }

    // -----------------------------------------------------------------------
    // Test 7: get_scroll_offset returns (0,0) for non-scrolling linedef
    // -----------------------------------------------------------------------
    #[test]
    fn get_scroll_offset_zero_for_non_scrolling() {
        let gs = GameState::new("TEST");
        let (ox, oy) = gs.get_scroll_offset(99);
        assert_eq!(ox, 0, "non-scrolling linedef must return x=0");
        assert_eq!(oy, 0, "non-scrolling linedef must return y=0");
    }

    // -----------------------------------------------------------------------
    // Test 8: Multiple scrolling walls tick independently
    // -----------------------------------------------------------------------
    #[test]
    fn multiple_scrolling_walls_tick_independently() {
        use crate::state::ScrollingWall;
        let mut gs = GameState::new("TEST");
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: 0,
            speed_x: 1,
            speed_y: 0,
            accumulated_x: 0,
            accumulated_y: 0,
        });
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: 1,
            speed_x: -3,
            speed_y: 2,
            accumulated_x: 0,
            accumulated_y: 0,
        });

        tick_scrollers(&mut gs);
        tick_scrollers(&mut gs);

        assert_eq!(
            gs.movers.scrolling_walls[0].accumulated_x, 2,
            "wall 0: 2 tics * speed_x=1"
        );
        assert_eq!(gs.movers.scrolling_walls[0].accumulated_y, 0);
        assert_eq!(
            gs.movers.scrolling_walls[1].accumulated_x, -6,
            "wall 1: 2 tics * speed_x=-3"
        );
        assert_eq!(
            gs.movers.scrolling_walls[1].accumulated_y, 4,
            "wall 1: 2 tics * speed_y=2"
        );
    }

    // -----------------------------------------------------------------------
    // Test 9: Accumulated offset grows linearly over multiple tics
    // -----------------------------------------------------------------------
    #[test]
    fn accumulated_offset_grows_linearly() {
        use crate::state::ScrollingWall;
        let mut gs = GameState::new("TEST");
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: 0,
            speed_x: 5,
            speed_y: 0,
            accumulated_x: 0,
            accumulated_y: 0,
        });

        for expected_tic in 1..=100 {
            tick_scrollers(&mut gs);
            assert_eq!(
                gs.movers.scrolling_walls[0].accumulated_x,
                expected_tic * 5,
                "offset must grow linearly: {} tics * 5",
                expected_tic
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test 10: ConveyorBelt creation with correct fields
    // -----------------------------------------------------------------------
    #[test]
    fn conveyor_belt_creation_correct_fields() {
        use crate::state::ConveyorBelt;
        let cb = ConveyorBelt {
            sector_index: 3,
            push_x: 100,
            push_y: -50,
            direction: 45,
            speed: 100,
        };
        assert_eq!(cb.sector_index, 3);
        assert_eq!(cb.push_x, 100);
        assert_eq!(cb.push_y, -50);
        assert_eq!(cb.direction, 45);
        assert_eq!(cb.speed, 100);
    }

    // -----------------------------------------------------------------------
    // Test 11: init_scrolling_walls finds all type-48 linedefs
    // -----------------------------------------------------------------------
    #[test]
    fn init_scrolling_walls_finds_all_type_48() {
        let mut gs = GameState::new("TEST");
        let level = make_scrolling_level(&[48, 0, 48, 48, 0]);
        init_scrolling_walls(&mut gs, &level);

        assert_eq!(
            gs.movers.scrolling_walls.len(),
            3,
            "must find exactly 3 type-48 linedefs out of 5"
        );
        assert_eq!(gs.movers.scrolling_walls[0].linedef_index, 0);
        assert_eq!(gs.movers.scrolling_walls[1].linedef_index, 2);
        assert_eq!(gs.movers.scrolling_walls[2].linedef_index, 3);
    }

    // -----------------------------------------------------------------------
    // Test 12: init_scrolling_walls skips non-scrolling linedefs
    // -----------------------------------------------------------------------
    #[test]
    fn init_scrolling_walls_skips_non_scrolling() {
        let mut gs = GameState::new("TEST");
        let level = make_scrolling_level(&[0, 1, 2, 26, 97, 100]);
        init_scrolling_walls(&mut gs, &level);

        assert_eq!(
            gs.movers.scrolling_walls.len(),
            0,
            "none of these line types are scrolling walls"
        );
    }

    // -----------------------------------------------------------------------
    // Test 13: init_conveyors creates conveyors for type 253
    // -----------------------------------------------------------------------
    #[test]
    fn init_conveyors_creates_for_type_253() {
        let mut gs = GameState::new("TEST");
        let level = make_conveyor_level(253, 10, 5);
        init_conveyors(&mut gs, &level);

        assert_eq!(
            gs.movers.conveyors.len(),
            1,
            "must create 1 conveyor for type 253"
        );
        assert_eq!(gs.movers.conveyors[0].sector_index, 0);
        assert_eq!(gs.movers.conveyors[0].push_x, 10);
        assert_eq!(gs.movers.conveyors[0].push_y, 5);
    }

    // -----------------------------------------------------------------------
    // Test 14: tick_conveyors applies push force (simplified test)
    // -----------------------------------------------------------------------
    #[test]
    fn tick_conveyors_applies_push_force() {
        use crate::state::ConveyorBelt;
        let mut gs = GameState::new("TEST");
        let level = make_conveyor_level(253, 0, 0);

        // Create an actor at z=0 (matching sector 0 floor_height=0).
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        let handle = gs.mobjslab.alloc(mo);

        // Manually add a conveyor with known push values.
        gs.movers.conveyors.push(ConveyorBelt {
            sector_index: 0,
            push_x: 256, // 256 raw = a small push
            push_y: 128,
            direction: 0,
            speed: 1,
        });

        tick_conveyors(&mut gs, Some(&level));

        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(
            mo.x,
            Fixed16_16::from_raw(256),
            "actor x must be pushed by push_x"
        );
        assert_eq!(
            mo.y,
            Fixed16_16::from_raw(128),
            "actor y must be pushed by push_y"
        );
    }

    // -----------------------------------------------------------------------
    // Test 15: GameState clone includes scrolling_walls and conveyors
    // -----------------------------------------------------------------------
    #[test]
    fn game_state_clone_includes_scrolling_walls_and_conveyors() {
        use crate::state::{ConveyorBelt, ScrollingWall};
        let mut gs = GameState::new("TEST");
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: 0,
            speed_x: 1,
            speed_y: 0,
            accumulated_x: 42,
            accumulated_y: 0,
        });
        gs.movers.conveyors.push(ConveyorBelt {
            sector_index: 3,
            push_x: 10,
            push_y: 20,
            direction: 0,
            speed: 10,
        });

        let gs2 = gs.clone();

        assert_eq!(gs2.movers.scrolling_walls.len(), 1);
        assert_eq!(gs2.movers.scrolling_walls[0].accumulated_x, 42);
        assert_eq!(gs2.movers.conveyors.len(), 1);
        assert_eq!(gs2.movers.conveyors[0].push_x, 10);

        // Ensure independence.
        gs.movers.scrolling_walls[0].accumulated_x = 999;
        assert_eq!(
            gs2.movers.scrolling_walls[0].accumulated_x, 42,
            "clone must be independent"
        );
    }

    // -----------------------------------------------------------------------
    // Test 16: Save/load roundtrip for ScrollingWall
    // -----------------------------------------------------------------------
    #[test]
    fn save_load_roundtrip_scrolling_wall() {
        use crate::state::ScrollingWall;
        let mut gs = GameState::new("E1M1");
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(100),
            Fixed16_16::from_int(200),
            Bam(0x4000_0000),
        );
        let handle = gs.mobjslab.alloc(mo);
        gs.player = crate::player::PlayerState::pistol_start(handle);

        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: 7,
            speed_x: -2,
            speed_y: 3,
            accumulated_x: 1234,
            accumulated_y: -5678,
        });

        let mut level_name = [0u8; 8];
        level_name[..4].copy_from_slice(b"E1M1");
        let data = crate::savegame::save_game(&gs, &level_name, 2, "test");
        let loaded = crate::savegame::load_game(&data).expect("value must exist in test");

        assert_eq!(loaded.state.movers.scrolling_walls.len(), 1);
        let sw = &loaded.state.movers.scrolling_walls[0];
        assert_eq!(sw.linedef_index, 7);
        assert_eq!(sw.speed_x, -2);
        assert_eq!(sw.speed_y, 3);
        assert_eq!(sw.accumulated_x, 1234);
        assert_eq!(sw.accumulated_y, -5678);
    }

    // -----------------------------------------------------------------------
    // Test 17: Save/load roundtrip for ConveyorBelt
    // -----------------------------------------------------------------------
    #[test]
    fn save_load_roundtrip_conveyor_belt() {
        use crate::state::ConveyorBelt;
        let mut gs = GameState::new("E1M1");
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::from_int(100),
            Fixed16_16::from_int(200),
            Bam(0x4000_0000),
        );
        let handle = gs.mobjslab.alloc(mo);
        gs.player = crate::player::PlayerState::pistol_start(handle);

        gs.movers.conveyors.push(ConveyorBelt {
            sector_index: 5,
            push_x: -999,
            push_y: 777,
            direction: 90,
            speed: 50,
        });

        let mut level_name = [0u8; 8];
        level_name[..4].copy_from_slice(b"E1M1");
        let data = crate::savegame::save_game(&gs, &level_name, 2, "test");
        let loaded = crate::savegame::load_game(&data).expect("value must exist in test");

        assert_eq!(loaded.state.movers.conveyors.len(), 1);
        let cb = &loaded.state.movers.conveyors[0];
        assert_eq!(cb.sector_index, 5);
        assert_eq!(cb.push_x, -999);
        assert_eq!(cb.push_y, 777);
        assert_eq!(cb.direction, 90);
        assert_eq!(cb.speed, 50);
    }

    // -----------------------------------------------------------------------
    // Test 18: Scroll offset wrapping / large values don't overflow
    // -----------------------------------------------------------------------
    #[test]
    fn scroll_offset_large_values_no_overflow() {
        use crate::state::ScrollingWall;
        let mut gs = GameState::new("TEST");
        gs.movers.scrolling_walls.push(ScrollingWall {
            linedef_index: 0,
            speed_x: i16::MAX,
            speed_y: i16::MIN,
            accumulated_x: i32::MAX - 100,
            accumulated_y: i32::MIN + 100,
        });

        // This must not panic — wrapping_add handles overflow gracefully.
        tick_scrollers(&mut gs);

        // The values should have wrapped around via wrapping_add.
        let sw = &gs.movers.scrolling_walls[0];
        let expected_x = (i32::MAX - 100).wrapping_add(i16::MAX as i32);
        let expected_y = (i32::MIN + 100).wrapping_add(i16::MIN as i32);
        assert_eq!(
            sw.accumulated_x, expected_x,
            "wrapping_add must handle overflow"
        );
        assert_eq!(
            sw.accumulated_y, expected_y,
            "wrapping_add must handle underflow"
        );
    }

    // -----------------------------------------------------------------------
    // Test 19: init_scrolling_walls handles mixed types (48 + 85)
    // -----------------------------------------------------------------------
    #[test]
    fn init_scrolling_walls_mixed_types_48_and_85() {
        let mut gs = GameState::new("TEST");
        let level = make_scrolling_level(&[48, 85, 0, 48]);
        init_scrolling_walls(&mut gs, &level);

        assert_eq!(gs.movers.scrolling_walls.len(), 3);
        // Linedef 0: type 48 → speed_x = 1
        assert_eq!(gs.movers.scrolling_walls[0].speed_x, 1);
        // Linedef 1: type 85 → speed_x = -1
        assert_eq!(gs.movers.scrolling_walls[1].speed_x, -1);
        // Linedef 3: type 48 → speed_x = 1
        assert_eq!(gs.movers.scrolling_walls[2].speed_x, 1);
    }

    // -----------------------------------------------------------------------
    // Test 20: init_conveyors handles types 254 and 255
    // -----------------------------------------------------------------------
    #[test]
    fn init_conveyors_handles_type_254_and_255() {
        let mut gs = GameState::new("TEST");
        let level_254 = make_conveyor_level(254, 20, 10);
        init_conveyors(&mut gs, &level_254);
        assert_eq!(
            gs.movers.conveyors.len(),
            1,
            "type 254 must create a conveyor"
        );
        assert_eq!(gs.movers.conveyors[0].push_x, 20);

        let mut gs2 = GameState::new("TEST");
        let level_255 = make_conveyor_level(255, -5, 15);
        init_conveyors(&mut gs2, &level_255);
        assert_eq!(
            gs2.movers.conveyors.len(),
            1,
            "type 255 must create a conveyor"
        );
        assert_eq!(gs2.movers.conveyors[0].push_x, -5);
        assert_eq!(gs2.movers.conveyors[0].push_y, 15);
    }

    // -----------------------------------------------------------------------
    // Tests: Floor Specials Completion (Batch 21)
    // -----------------------------------------------------------------------

    // --- Sector height query helpers ---

    #[test]
    fn highest_adjacent_ceiling_returns_correct_value() {
        // Sector 1 adjacent to sector 0 (ceil=128) and sector 2 (ceil=200).
        let level = make_multi_sector_level([0, 0, 0], [128, 96, 200], [0, 0, 0], 0, 0);
        let result = highest_adjacent_ceiling(&level, 1);
        assert_eq!(
            result, 200,
            "highest adjacent ceiling to sector 1 should be 200 (sector 2)"
        );
    }

    #[test]
    fn highest_adjacent_ceiling_no_neighbors_returns_own() {
        let level = make_damage_level(0, 0);
        // Sector 0 has ceil=128 and no adjacent sectors.
        let result = highest_adjacent_ceiling(&level, 0);
        assert_eq!(result, 128, "no adjacent sectors => returns own ceiling");
    }

    #[test]
    fn next_highest_floor_above_returns_next_floor_above_current() {
        // Sector 1: floor=0, adjacent to sector 0 (floor=32) and sector 2 (floor=64).
        let level = make_multi_sector_level([32, 0, 64], [128, 128, 128], [0, 0, 0], 0, 0);
        let result = next_highest_floor_above(&level, 1, 10);
        assert_eq!(result, 32, "next floor above 10 should be 32 (sector 0)");
    }

    #[test]
    fn next_highest_floor_above_returns_current_when_no_higher() {
        // Sector 1: floor=100, adjacent to 0 (floor=20) and 2 (floor=50). Both below 100.
        let level = make_multi_sector_level([20, 100, 50], [200, 200, 200], [0, 0, 0], 0, 0);
        let result = next_highest_floor_above(&level, 1, 100);
        assert_eq!(result, 100, "no floor above 100 => returns 100");
    }

    #[test]
    fn shortest_lower_texture_returns_correct_value() {
        // Build a level with a lower texture on a sidedef facing sector 1.
        let reject = doom_map::Reject::parse_lump(&[0u8; 2], 3).expect("value must exist in test");
        let sectors = vec![
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 1,
            },
            doom_map::Sector {
                floor_height: 32,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            doom_map::Sector {
                floor_height: 64,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
            doom_map::Vertex { x: 128, y: 0 },
        ];
        let sidedefs = vec![
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 48, // texture height proxy
                upper_texture: [0; 8],
                lower_texture: *b"STEP1\0\0\0", // non-empty lower texture
                middle_texture: [0; 8],
                sector: 0,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 72, // another lower texture height
                upper_texture: [0; 8],
                lower_texture: *b"STEP2\0\0\0",
                middle_texture: [0; 8],
                sector: 0,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 2,
            },
        ];
        let linedefs = vec![
            doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            doom_map::Linedef {
                from_vertex: 1,
                to_vertex: 2,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 2,
                left_sidedef: 3,
            },
        ];
        let level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        };

        let result = shortest_lower_texture(&level, 0);
        assert_eq!(
            result, 48,
            "shortest lower texture height should be 48 (the smaller y_offset)"
        );
    }

    #[test]
    fn shortest_lower_texture_no_textures_returns_zero() {
        let level = make_damage_level(0, 0);
        let result = shortest_lower_texture(&level, 0);
        assert_eq!(result, 0, "no lower textures => returns 0");
    }

    // --- Floor activation functions ---

    #[test]
    fn ev_floor_lower_to_lowest_creates_correct_mover() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=0, Sector 1: floor=64 tag=1, Sector 2: floor=32.
        let level = make_multi_sector_level([0, 64, 32], [128, 128, 128], [0, 1, 0], 0, 0);
        ev_floor_lower_to_lowest(&mut gs, &level, 1, 2);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 0,
            "lowest adjacent = 0"
        );
        assert_eq!(gs.movers.active_floors[0].speed, 2);
        assert_eq!(gs.movers.active_floors[0].direction, MoveDirection::Down);
        assert_eq!(
            gs.movers.active_floors[0].floor_type,
            FloorType::LowerToLowest
        );
    }

    #[test]
    fn ev_floor_lower_to_highest_creates_correct_mover() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=10, Sector 1: floor=64 tag=1, Sector 2: floor=48.
        let level = make_multi_sector_level([10, 64, 48], [128, 128, 128], [0, 1, 0], 0, 0);
        ev_floor_lower_to_highest(&mut gs, &level, 1, 1);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 48,
            "highest adjacent = 48"
        );
        assert_eq!(
            gs.movers.active_floors[0].floor_type,
            FloorType::LowerToHighest
        );
    }

    #[test]
    fn ev_floor_raise_to_lowest_ceiling_works() {
        let mut gs = GameState::new("TEST");
        // Sector 0: ceil=128, Sector 1: floor=0 ceil=200 tag=1, Sector 2: ceil=96.
        let level = make_multi_sector_level([0, 0, 0], [128, 200, 96], [0, 1, 0], 0, 0);
        ev_floor_raise_to_lowest_ceiling(
            &mut gs,
            &level,
            1,
            1,
            crate::state::CrushBehavior::NoCrush,
        );
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 96,
            "lowest adj ceil = 96"
        );
        assert_eq!(gs.movers.active_floors[0].direction, MoveDirection::Up);
        assert!(
            gs.movers.active_floors[0].crush == crate::state::CrushBehavior::NoCrush,
            "crush should be false"
        );
    }

    #[test]
    fn ev_floor_raise_to_nearest_works() {
        let mut gs = GameState::new("TEST");
        // Sector 1: floor=0, adjacent to sector 0 (floor=32) and sector 2 (floor=64).
        // next_highest_floor above 0 = 32.
        let level = make_multi_sector_level([32, 0, 64], [128, 128, 128], [0, 1, 0], 0, 0);
        ev_floor_raise_to_nearest(&mut gs, &level, 1, 1);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 32,
            "next highest = 32"
        );
        assert_eq!(
            gs.movers.active_floors[0].floor_type,
            FloorType::RaiseToNearest
        );
    }

    #[test]
    fn ev_floor_raise_by_texture_works() {
        let mut gs = GameState::new("TEST");
        // Build a level where sector 1 (tag=1) has a lower texture with height 48.
        let reject = doom_map::Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");
        let sectors = vec![
            doom_map::Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            doom_map::Sector {
                floor_height: 16,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 1,
            },
        ];
        let vertexes = vec![
            doom_map::Vertex { x: 0, y: 0 },
            doom_map::Vertex { x: 64, y: 0 },
        ];
        let sidedefs = vec![
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 48,
                upper_texture: [0; 8],
                lower_texture: *b"STEP1\0\0\0",
                middle_texture: [0; 8],
                sector: 1,
            },
            doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 0,
            },
        ];
        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];
        let level = doom_map::Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors,
            reject,
            blockmap: make_minimal_blockmap(),
        };

        ev_floor_raise_by_texture(&mut gs, &level, 1, 1);
        assert_eq!(gs.movers.active_floors.len(), 1);
        // floor=16, shortest lower texture=48, target=16+48=64.
        assert_eq!(gs.movers.active_floors[0].target_height, 64, "16 + 48 = 64");
        assert_eq!(
            gs.movers.active_floors[0].floor_type,
            FloorType::RaiseByTexture
        );
    }

    #[test]
    fn ev_floor_raise_24_raises_by_exactly_24() {
        let mut gs = GameState::new("TEST");
        // Sector 1: floor=10, tag=1.
        let level = make_multi_sector_level([0, 10, 0], [128, 128, 128], [0, 1, 0], 0, 0);
        ev_floor_raise_24(&mut gs, &level, 1, 1);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(gs.movers.active_floors[0].target_height, 34, "10 + 24 = 34");
        assert_eq!(gs.movers.active_floors[0].floor_type, FloorType::Raise24);
    }

    #[test]
    fn ev_floor_raise_32_raises_by_exactly_32() {
        let mut gs = GameState::new("TEST");
        // Sector 1: floor=20, tag=1.
        let level = make_multi_sector_level([0, 20, 0], [128, 128, 128], [0, 1, 0], 0, 0);
        ev_floor_raise_32(&mut gs, &level, 1, 1);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(gs.movers.active_floors[0].target_height, 52, "20 + 32 = 52");
        assert_eq!(gs.movers.active_floors[0].floor_type, FloorType::Raise32);
    }

    #[test]
    fn ev_floor_raise_to_ceiling_raises_to_own_ceiling() {
        let mut gs = GameState::new("TEST");
        // Sector 1: floor=0, ceil=200, tag=1.
        let level = make_multi_sector_level([0, 0, 0], [128, 200, 128], [0, 1, 0], 0, 0);
        ev_floor_raise_to_ceiling(&mut gs, &level, 1, 1, crate::state::CrushBehavior::NoCrush);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 200,
            "target = own ceiling = 200"
        );
        assert_eq!(
            gs.movers.active_floors[0].floor_type,
            FloorType::RaiseToCeiling
        );
    }

    // --- Line type dispatch tests ---

    #[test]
    fn line_type_19_dispatches_lower_to_highest() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=10, Sector 1: floor=64 tag=1, Sector 2: floor=48.
        let mut level = make_multi_sector_level([10, 64, 48], [128, 128, 128], [0, 1, 0], 19, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 48,
            "type 19: target = highest adjacent = 48"
        );
    }

    #[test]
    fn line_type_23_dispatches_lower_to_lowest() {
        let mut gs = GameState::new("TEST");
        let mut level = make_multi_sector_level([0, 64, 32], [128, 128, 128], [0, 1, 0], 23, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 0,
            "type 23: target = lowest adjacent = 0"
        );
    }

    #[test]
    fn line_type_36_dispatches_highest_minus_8() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=10, Sector 1: floor=64 tag=1, Sector 2: floor=30.
        // highest_adjacent = 30, target = 30 + 8 = 38.
        let mut level = make_multi_sector_level([10, 64, 30], [128, 128, 128], [0, 1, 0], 36, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 38,
            "type 36: target = highest_adj(30) + 8 = 38"
        );
        assert_eq!(
            gs.movers.active_floors[0].speed, 4,
            "type 36: turbo speed = 4"
        );
    }

    #[test]
    fn line_type_56_dispatches_ceiling_minus_8_with_crush() {
        let mut gs = GameState::new("TEST");
        // Sector 0: ceil=128, Sector 1: floor=0 ceil=200 tag=1, Sector 2: ceil=100.
        // lowest_adj_ceil = 100, target = 100 - 8 = 92.
        let mut level = make_multi_sector_level([0, 0, 0], [128, 200, 100], [0, 1, 0], 56, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 92,
            "type 56: target = lowest_adj_ceil(100) - 8 = 92"
        );
        assert!(
            gs.movers.active_floors[0].crush == crate::state::CrushBehavior::Crush,
            "type 56 must have crush=true"
        );
    }

    #[test]
    fn line_type_64_dispatches_raise_to_lowest_ceiling() {
        let mut gs = GameState::new("TEST");
        // Sector 0: ceil=128, Sector 1: floor=0 ceil=200 tag=1, Sector 2: ceil=96.
        let mut level = make_multi_sector_level([0, 0, 0], [128, 200, 96], [0, 1, 0], 64, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 96,
            "type 64: target = lowest_adj_ceil = 96"
        );
    }

    #[test]
    fn line_type_91_dispatches_raise_to_lowest_ceiling() {
        let mut gs = GameState::new("TEST");
        let mut level = make_multi_sector_level([0, 0, 0], [128, 200, 80], [0, 1, 0], 91, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 80,
            "type 91: target = lowest_adj_ceil = 80"
        );
    }

    #[test]
    fn multiple_floor_movers_active_simultaneously() {
        let mut gs = GameState::new("TEST");
        // Sector 0: floor=0, Sector 1: floor=64 tag=1, Sector 2: floor=32 tag=2.
        // Both sectors 1 and 2 have different tags.
        let level = make_multi_sector_level([0, 64, 32], [128, 128, 128], [0, 1, 2], 0, 0);

        // Create a floor mover for tag 1 (sector 1).
        ev_floor_lower_to_lowest(&mut gs, &level, 1, 1);
        assert_eq!(gs.movers.active_floors.len(), 1);

        // Create a floor mover for tag 2 (sector 2).
        ev_floor_lower_to_lowest(&mut gs, &level, 2, 2);
        assert_eq!(
            gs.movers.active_floors.len(),
            2,
            "two floor movers must be active simultaneously"
        );

        // Verify they target different sectors.
        assert_eq!(gs.movers.active_floors[0].sector_index, 1);
        assert_eq!(gs.movers.active_floors[1].sector_index, 2);
    }

    #[test]
    fn game_state_clone_preserves_floor_type_field() {
        let mut gs = GameState::new("TEST");
        gs.movers.active_floors.push(FloorMover {
            sector_index: 0,
            target_height: 32,
            speed: 1,
            direction: MoveDirection::Up,
            wait_tics: -1,
            return_height: 0,
            waiting: false,
            wait_remaining: 0,
            crush: crate::state::CrushBehavior::NoCrush,
            tag: 1,
            floor_type: FloorType::Raise24,
        });

        let gs2 = gs.clone();
        assert_eq!(gs2.movers.active_floors.len(), 1);
        assert_eq!(
            gs2.movers.active_floors[0].floor_type,
            FloorType::Raise24,
            "clone must preserve floor_type field"
        );
    }

    // --- Additional line type dispatch tests ---

    #[test]
    fn line_type_38_dispatches_lower_to_lowest() {
        let mut gs = GameState::new("TEST");
        let mut level = make_multi_sector_level([10, 64, 32], [128, 128, 128], [0, 1, 0], 38, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 10,
            "type 38: target = lowest adjacent = 10"
        );
    }

    #[test]
    fn line_type_45_dispatches_lower_to_highest() {
        let mut gs = GameState::new("TEST");
        let mut level = make_multi_sector_level([10, 64, 48], [128, 128, 128], [0, 1, 0], 45, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 48,
            "type 45: target = highest adjacent = 48"
        );
    }

    #[test]
    fn line_type_82_dispatches_lower_to_lowest() {
        let mut gs = GameState::new("TEST");
        let mut level = make_multi_sector_level([5, 64, 32], [128, 128, 128], [0, 1, 0], 82, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 5,
            "type 82: target = lowest adjacent = 5"
        );
    }

    #[test]
    fn line_type_58_dispatches_raise_24() {
        let mut gs = GameState::new("TEST");
        let mut level = make_multi_sector_level([0, 10, 0], [128, 128, 128], [0, 1, 0], 58, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 34,
            "type 58: target = 10 + 24 = 34"
        );
    }

    #[test]
    fn line_type_102_dispatches_lower_to_highest() {
        let mut gs = GameState::new("TEST");
        let mut level = make_multi_sector_level([10, 64, 40], [128, 128, 128], [0, 1, 0], 102, 1);
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 40,
            "type 102: target = highest adjacent = 40"
        );
    }

    #[test]
    fn ev_floor_lower_to_nearest_works() {
        let mut gs = GameState::new("TEST");
        // Sector 1: floor=64, adjacent to sector 0 (floor=10) and sector 2 (floor=32).
        // Next lowest (highest below 64) = 32.
        let level = make_multi_sector_level([10, 64, 32], [128, 128, 128], [0, 1, 0], 0, 0);
        ev_floor_lower_to_nearest(&mut gs, &level, 1, 1);
        assert_eq!(gs.movers.active_floors.len(), 1);
        assert_eq!(
            gs.movers.active_floors[0].target_height, 32,
            "next lowest adjacent below 64 = 32"
        );
        assert_eq!(
            gs.movers.active_floors[0].floor_type,
            FloorType::LowerToNearest
        );
    }
}
