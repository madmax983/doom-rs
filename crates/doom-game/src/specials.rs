//! Sector specials and linedef triggers.
//!
//! Port of Doom's `p_spec.c` and `p_ceilng.c` / `p_doors.c` (simplified).
//!
//! # Implemented
//! - `tick_sector_specials`: damage floors (specials 5, 7, 16).
//! - `p_use_lines`: player USE activation, dispatches to `activate_linedef`.
//! - `activate_linedef`: door toggle (special 1).

use doom_map::{Level, SIDEDEF_NONE};

use crate::mobj::MobjHandle;
use crate::state::GameState;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Distance ahead the player can activate a linedef.
pub const USE_RANGE: i32 = 64;

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
/// Currently handles:
/// - **1**: Door toggle — opens a closed door or closes an open one.
/// - **11**: Exit — no-op stub.
/// - Other: no-op.
pub fn activate_linedef(gs: &mut GameState, level: &mut Level, linedef_idx: usize) {
    let Some(ld) = level.linedefs.get(linedef_idx) else {
        return;
    };

    match ld.special {
        1 => {
            // Door toggle.  The back sector is referenced via the left sidedef.
            let left_sidedef = ld.left_sidedef;
            if left_sidedef == SIDEDEF_NONE {
                // One-sided linedef — nothing to toggle.
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

            // Leave special=1 so the door stays retriggerable.
            let _ = gs; // gs is threaded through for future use (e.g., sounds).
        }
        11 => {
            // Exit — no-op stub.
        }
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
    /// - Linedef 0: two-sided (FLAG_TWO_SIDED=4), special=1, right=0, left=1.
    ///
    /// The actor stands at (-32, 0) and the linedef is at x=0.
    /// With the trig-table fallback (`ahead_x = ax + USE_RANGE`), the ray from
    /// (-32, 0) → (32, 0) will cross x=0.
    fn make_door_level(door_ceil: i16) -> doom_map::Level {
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

        // FLAG_TWO_SIDED = 0x0004
        let linedefs = vec![doom_map::Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 1,
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
    // Tests: activate_linedef (door toggle)
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
}
