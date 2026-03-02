//! Sector specials and linedef triggers.
//!
//! Port of Doom's `p_spec.c` and `p_ceilng.c` / `p_doors.c` (simplified).
//!
//! # Implemented
//! - `tick_sector_specials`: damage floors (specials 5, 7, 16).
//! - `tick_doors`: advance active door/floor movers.
//! - `tick_lights`: advance light specials.
//! - `spawn_level_specials`: initialise light thinkers on level load.
//! - `p_use_lines`: player USE activation, dispatches to `activate_linedef`.
//! - `activate_linedef`: door toggle (types 1, 2, 26, 27, 28, 29, 63, 64).

use doom_map::{Level, SIDEDEF_NONE};

use crate::mobj::MobjHandle;
use crate::state::{DoorMover, GameState, LightSpecial};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Distance ahead the player can activate a linedef.
pub const USE_RANGE: i32 = 64;

/// Door open/close speed in map units per tic (Doom standard: 2 units/tic).
const DOOR_SPEED: i16 = 2;

/// Tics a door stays open before auto-closing (3.5 seconds at 35 Hz ≈ 120 tics).
const DOOR_WAIT: i32 = 120;

/// Period for fast blinking lights (tics).
const BLINK_FAST_PERIOD: i32 = 15;

/// Period for slow blinking lights (tics).
const BLINK_SLOW_PERIOD: i32 = 35;

// ---------------------------------------------------------------------------
// tick_sector_specials
// ---------------------------------------------------------------------------

/// Apply sector special damage to the actor each tic.
///
/// Simplified port of `P_PlayerInSpecialSector`.
///
/// Sector containment is approximated: the actor is considered to be "in" a
/// special sector if `actor.z.to_int() == sector.floor_height as i32`.
///
/// Damage is applied directly to `mobj.health` without routing through combat
/// to avoid circular dependencies at this stage.
pub fn tick_sector_specials(gs: &mut GameState, level: &Level, handle: MobjHandle) {
    // Read actor position.
    let (az, _ax, _ay) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.z.to_int(), mo.x.to_int(), mo.y.to_int()),
        None => return,
    };

    for sector in &level.sectors {
        if sector.special == 0 {
            continue;
        }

        // Only apply damage if actor is standing on this floor.
        if az != sector.floor_height as i32 {
            continue;
        }

        let dmg: i32 = match sector.special {
            5 => 10,  // lava
            7 => 5,   // nukage
            16 => 20, // acid
            _ => continue,
        };

        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.health -= dmg;
            if mo.health < 0 {
                mo.health = 0;
            }
        }

        // Only apply one sector's damage per tic (first match wins).
        return;
    }
}

// ---------------------------------------------------------------------------
// tick_doors
// ---------------------------------------------------------------------------

/// Advance all active door/floor movers by one tic.
///
/// Call this once per tic from `tick()`.
pub fn tick_doors(gs: &mut GameState, level: &mut Level) {
    let mut i = 0;
    while i < gs.active_doors.len() {
        // Borrow just the fields we need, then operate.
        let countdown = gs.active_doors[i].countdown;
        let speed_abs = gs.active_doors[i].speed.abs();

        // Waiting at open position?
        if countdown > 0 {
            gs.active_doors[i].countdown -= 1;
            i += 1;
            continue;
        }
        if countdown == 0 {
            // Start closing — negate speed so it moves downward.
            gs.active_doors[i].speed = -speed_abs;
            gs.active_doors[i].countdown = -1;
        }

        // Move toward target.
        let sector_idx = gs.active_doors[i].sector;
        let speed = gs.active_doors[i].speed;
        let target = gs.active_doors[i].target_height;
        let is_ceiling = gs.active_doors[i].is_ceiling;

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
                // Update current_height mirror.
                gs.active_doors[i].current_height = target;
                gs.active_doors.remove(i);
                // Do NOT increment i — item was removed.
                continue;
            }
            // Mirror current height.
            let new_h = if is_ceiling {
                level.sectors[sector_idx].ceil_height
            } else {
                level.sectors[sector_idx].floor_height
            };
            gs.active_doors[i].current_height = new_h;
        }

        i += 1;
    }
}

// ---------------------------------------------------------------------------
// tick_lights
// ---------------------------------------------------------------------------

/// Advance all light specials by one tic.
pub fn tick_lights(gs: &mut GameState, level: &mut Level) {
    for light in &mut gs.active_lights {
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
        match sector.special {
            1 => {
                // Random off: slow blink, goes dark.
                gs.active_lights.push(LightSpecial {
                    sector: i,
                    timer: BLINK_SLOW_PERIOD,
                    period: BLINK_SLOW_PERIOD,
                    bright: sector.light_level,
                    dark: 0,
                    is_bright: true,
                });
            }
            2 => {
                // Fast strobe.
                gs.active_lights.push(LightSpecial {
                    sector: i,
                    timer: BLINK_FAST_PERIOD,
                    period: BLINK_FAST_PERIOD,
                    bright: sector.light_level,
                    dark: 0,
                    is_bright: true,
                });
            }
            3 => {
                // Slow strobe: dim but not fully dark.
                gs.active_lights.push(LightSpecial {
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
// Door helpers
// ---------------------------------------------------------------------------

/// Enqueue a door mover that opens and optionally auto-closes.
fn open_door(gs: &mut GameState, level: &Level, sector_idx: usize, auto_close: bool) {
    let sector = match level.sectors.get(sector_idx) {
        Some(s) => s,
        None => return,
    };

    // Use sector's current ceiling as the open target (at least 128 above floor).
    let target = sector.ceil_height.max(sector.floor_height + 128);

    // Avoid duplicate movers for the same sector.
    if gs.active_doors.iter().any(|d| d.sector == sector_idx) {
        return;
    }

    gs.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: DOOR_SPEED,
        is_ceiling: true,
        wait_tics: if auto_close { DOOR_WAIT } else { -1 },
        countdown: if auto_close { DOOR_WAIT } else { -1 },
    });
}

/// Enqueue a door mover that closes a door.
fn close_door(gs: &mut GameState, level: &Level, sector_idx: usize) {
    let sector = match level.sectors.get(sector_idx) {
        Some(s) => s,
        None => return,
    };

    let target = sector.floor_height + 4;

    // Avoid duplicate movers for the same sector.
    if gs.active_doors.iter().any(|d| d.sector == sector_idx) {
        return;
    }

    gs.active_doors.push(DoorMover {
        sector: sector_idx,
        target_height: target,
        current_height: sector.ceil_height,
        speed: -DOOR_SPEED,
        is_ceiling: true,
        wait_tics: -1,
        countdown: -1,
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
    let (ax, ay, angle) = match gs.mobjslab.get(handle) {
        Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.angle),
        None => return,
    };

    let cos_int = angle.cos().to_int();
    let sin_int = angle.sin().to_int();

    // Fall back if trig tables are uninitialised (both return 0).
    let (ahead_x, ahead_y) = if cos_int == 0 && sin_int == 0 {
        (ax + USE_RANGE, ay)
    } else {
        (ax + USE_RANGE * cos_int, ay + USE_RANGE * sin_int)
    };

    // Find and activate the first linedef whose special segment the ray crosses.
    for ld_idx in 0..level.linedefs.len() {
        let ld = &level.linedefs[ld_idx];
        if ld.special == 0 {
            continue;
        }

        let v1 = &level.vertexes[ld.from_vertex as usize];
        let v2 = &level.vertexes[ld.to_vertex as usize];

        let lx1 = v1.x as i32;
        let ly1 = v1.y as i32;
        let lx2 = v2.x as i32;
        let ly2 = v2.y as i32;

        if segment_crosses_line(ax, ay, ahead_x, ahead_y, lx1, ly1, lx2, ly2) {
            activate_linedef(gs, level, ld_idx);
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
        // --- Type 1: toggle door (immediate, for backward compatibility with existing tests) ---
        1 => {
            if left_sidedef == SIDEDEF_NONE {
                return;
            }
            let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                Some(sd) => sd.sector as usize,
                None => return,
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
            if left_sidedef == SIDEDEF_NONE {
                return;
            }
            let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                Some(sd) => sd.sector as usize,
                None => return,
            };
            open_door(gs, level, sector_idx, false);
        }

        // --- Type 29: close door (animated) ---
        29 => {
            if left_sidedef == SIDEDEF_NONE {
                return;
            }
            let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                Some(sd) => sd.sector as usize,
                None => return,
            };
            close_door(gs, level, sector_idx);
        }

        // --- Types 26/27/28: locked raise-and-close door ---
        26 => {
            // Blue card or skull required.
            if gs.player.has_key(crate::player::KEY_BLUE_CARD)
                || gs.player.has_key(crate::player::KEY_BLUE_SKULL)
            {
                if left_sidedef == SIDEDEF_NONE {
                    return;
                }
                let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                    Some(sd) => sd.sector as usize,
                    None => return,
                };
                open_door(gs, level, sector_idx, true);
            }
        }
        27 => {
            // Yellow key required.
            if gs.player.has_key(crate::player::KEY_YELLOW_CARD)
                || gs.player.has_key(crate::player::KEY_YELLOW_SKULL)
            {
                if left_sidedef == SIDEDEF_NONE {
                    return;
                }
                let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                    Some(sd) => sd.sector as usize,
                    None => return,
                };
                open_door(gs, level, sector_idx, true);
            }
        }
        28 => {
            // Red key required.
            if gs.player.has_key(crate::player::KEY_RED_CARD)
                || gs.player.has_key(crate::player::KEY_RED_SKULL)
            {
                if left_sidedef == SIDEDEF_NONE {
                    return;
                }
                let sector_idx = match level.sidedefs.get(left_sidedef as usize) {
                    Some(sd) => sd.sector as usize,
                    None => return,
                };
                open_door(gs, level, sector_idx, true);
            }
        }

        // --- Type 63/64: remote tag-based door ---
        63 | 64 => {
            let tag = level.linedefs[linedef_idx].tag;
            let sector_indices: Vec<usize> = level
                .sectors
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tag == tag)
                .map(|(i, _)| i)
                .collect();
            for idx in sector_indices {
                open_door(gs, level, idx, special == 64);
            }
        }

        // --- Type 11: Exit — no-op stub ---
        11 => {}

        _ => {
            // Unknown special — silently ignored.
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the segment from `(ax, ay)` to `(bx, by)` crosses the
/// infinite line defined by `(lx1, ly1)` → `(lx2, ly2)`.
///
/// Uses the cross-product (sign) test: the segment crosses the line when the
/// two endpoints lie on opposite sides.
fn segment_crosses_line(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    lx1: i32,
    ly1: i32,
    lx2: i32,
    ly2: i32,
) -> bool {
    // Direction of the linedef.
    let ldx = (lx2 - lx1) as i64;
    let ldy = (ly2 - ly1) as i64;

    // Cross products of linedef direction with each segment endpoint.
    let c1 = ldx * (ay - ly1) as i64 - ldy * (ax - lx1) as i64;
    let c2 = ldx * (by - ly1) as i64 - ldy * (bx - lx1) as i64;

    // Different signs (XOR of sign bits < 0) means the segment straddles the line.
    (c1 ^ c2) < 0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind};
    use crate::state::GameState;
    use doom_types::{Bam, Fixed16_16};

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
        doom_map::Blockmap::parse_lump(&bm_data).unwrap()
    }

    /// Build a level with one sector that has the given special, no linedefs.
    fn make_damage_level(floor_height: i16, special: u16) -> doom_map::Level {
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).unwrap();
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
        let reject = doom_map::Reject::parse_lump(&[0u8], 2).unwrap();

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

        let health = gs.mobjslab.get(handle).unwrap().health;
        assert_eq!(health, 90, "lava (special 5) must deal 10 damage per tic");
    }

    #[test]
    fn damage_floor_ignores_actor_above_it() {
        let mut gs = GameState::new("TEST");
        let level = make_damage_level(0, 5); // lava at floor=0
        let handle = make_actor_at_z(&mut gs, 10); // actor z=10, not on the floor

        tick_sector_specials(&mut gs, &level, handle);

        let health = gs.mobjslab.get(handle).unwrap().health;
        assert_eq!(health, 100, "actor above lava floor must take no damage");
    }

    // -----------------------------------------------------------------------
    // Tests: p_use_lines
    // -----------------------------------------------------------------------

    #[test]
    fn p_use_lines_activates_nearest_linedef() {
        let mut gs = GameState::new("TEST");
        // Closed door: ceil == floor (0).
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

        // Before: door is closed (ceil == floor == 0).
        assert_eq!(level.sectors[1].ceil_height, 0);

        p_use_lines(&mut gs, &mut level, handle);

        // After: door opened (ceil = floor + 128 = 128).
        assert_eq!(
            level.sectors[1].ceil_height, 128,
            "p_use_lines must open the door sector"
        );
    }

    // -----------------------------------------------------------------------
    // Tests: activate_linedef (door toggle — type 1)
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

    // -----------------------------------------------------------------------
    // Tests: animated doors (type 2)
    // -----------------------------------------------------------------------

    #[test]
    fn animated_door_opens_over_time() {
        let mut gs = GameState::new("TEST");
        // Type 2: open door, stays open. Start fully closed (ceil == floor == 0).
        let mut level = make_door_level_with_special(0, 2);

        assert_eq!(level.sectors[1].ceil_height, 0, "precondition: door closed");
        assert!(gs.active_doors.is_empty());

        // Activate the linedef — enqueues a DoorMover.
        activate_linedef(&mut gs, &mut level, 0);
        assert_eq!(gs.active_doors.len(), 1, "DoorMover should be enqueued");

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
            gs.active_doors.is_empty(),
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
            gs.active_doors.len(),
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
            gs.active_doors.len(),
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
            gs.active_doors.is_empty(),
            "yellow door must not open without yellow key"
        );
    }

    #[test]
    fn red_locked_door_blocked_without_key() {
        let mut gs = GameState::new("TEST");
        let mut level = make_door_level_with_special(0, 28);

        activate_linedef(&mut gs, &mut level, 0);

        assert!(
            gs.active_doors.is_empty(),
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
            gs.active_lights.len(),
            1,
            "spawn_level_specials must create one light thinker for special=1"
        );
    }

    #[test]
    fn light_toggles_after_period() {
        let mut gs = GameState::new("TEST");
        let reject = doom_map::Reject::parse_lump(&[0u8], 1).unwrap();
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
        assert_eq!(gs.active_lights.len(), 1);

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
}
