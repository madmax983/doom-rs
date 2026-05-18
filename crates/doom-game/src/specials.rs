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
mod tests;
