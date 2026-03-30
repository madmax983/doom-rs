//! Thing spawning from level data.
//!
//! Iterates the level's THINGS lump and creates `Mobj` actors in the
//! `GameState`'s slab arena, applying skill filtering, multiplayer
//! filtering, MOBJINFO defaults, and tracking kill/item totals.

use doom_map::Level;
use doom_types::{Bam, Fixed16_16};

use crate::mobj::{Mobj, MobjHandle, MobjKind, flags};
use crate::mobjinfo::MOBJINFO;
use crate::pickups::doomed_type_to_kind;
use crate::player::PlayerState;
use crate::state::GameState;
use crate::states::STATES;

// ---------------------------------------------------------------------------
// Skill level
// ---------------------------------------------------------------------------

/// Skill level for thing filtering.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Skill {
    /// I'm Too Young To Die.
    Baby = 0,
    /// Hey, Not Too Rough.
    Easy = 1,
    /// Hurt Me Plenty.
    Medium = 2,
    /// Ultra-Violence.
    Hard = 3,
    /// Nightmare!
    Nightmare = 4,
}

impl Skill {
    /// Convert an integer to a `Skill`.
    ///
    /// Returns `None` for any out-of-range value.
    pub fn from_num(n: u8) -> Option<Self> {
        Self::from_repr(n)
    }
}

// ---------------------------------------------------------------------------
// Thing flag bits (from the WAD THINGS lump)
// ---------------------------------------------------------------------------

/// Thing appears on skill 1 & 2 (Baby / Easy).
const MTF_EASY: u16 = 0x0001;
/// Thing appears on skill 3 (Medium / Hurt Me Plenty).
const MTF_MEDIUM: u16 = 0x0002;
/// Thing appears on skill 4 & 5 (Hard / Nightmare).
const MTF_HARD: u16 = 0x0004;
/// Monster is deaf / ambush — won't react to sound, only sight.
const MTF_AMBUSH: u16 = 0x0008;
/// Thing only appears in multiplayer (deathmatch / coop).
const MTF_MULTIPLAYER: u16 = 0x0010;

// ---------------------------------------------------------------------------
// Angle conversion
// ---------------------------------------------------------------------------

/// BAM units per degree: 2^32 / 360.
const BAM_PER_DEGREE: u32 = (0x1_0000_0000u64 / 360) as u32;

/// Convert a degrees value (0-359) from a Thing to a BAM angle.
#[inline]
fn degrees_to_bam(degrees: u16) -> Bam {
    Bam((degrees as u32).wrapping_mul(BAM_PER_DEGREE))
}

// ---------------------------------------------------------------------------
// MOBJINFO application
// ---------------------------------------------------------------------------

/// Apply default properties from the MOBJINFO table to a freshly-created Mobj.
///
/// Sets health, radius, height, speed, flags, and initial state/tics from
/// the table entry corresponding to `mo.kind`.
pub(crate) fn apply_mobjinfo_defaults(mo: &mut Mobj) {
    let idx = mo.kind as usize;
    if idx >= MOBJINFO.len() {
        return;
    }
    let info = &MOBJINFO[idx];
    mo.health = info.spawn_health;
    mo.radius = info.radius;
    mo.height = info.height;
    mo.flags = info.flags;

    // Set initial state and tics from the spawn state.
    mo.state = info.spawn_state;
    let state_idx = info.spawn_state.0 as usize;
    if state_idx < STATES.len() {
        mo.tics = STATES[state_idx].tics;
    }
}

/// Sync a freshly spawned map thing to the floor and subsector it occupies.
fn sync_mobj_to_level(level: &Level, mo: &mut Mobj) {
    if let Some(subsector) = level.subsector_index_at(mo.x.to_int(), mo.y.to_int()) {
        mo.subsector = subsector as u32;
    }

    let sector_idx = level
        .sector_index_at(mo.x.to_int(), mo.y.to_int())
        .or_else(|| crate::sight::sector_from_subsector(level, mo.subsector as usize));
    if let Some(sector_idx) = sector_idx
        && let Some(sector) = level.sectors.get(sector_idx)
    {
        mo.z = if mo.flags & flags::MF_SPAWNCEILING != 0 {
            Fixed16_16::from_int(i32::from(sector.ceil_height)) - mo.height
        } else {
            Fixed16_16::from_int(i32::from(sector.floor_height))
        };
    }
}

// ---------------------------------------------------------------------------
// Skill filtering
// ---------------------------------------------------------------------------

/// Returns `true` if the thing should be spawned at the given skill level.
fn should_spawn_for_skill(thing_flags: u16, skill: Skill) -> bool {
    let skill_bits = thing_flags & (MTF_EASY | MTF_MEDIUM | MTF_HARD);

    // If no skill bits are set at all, spawn it anyway (some WADs do this).
    if skill_bits == 0 {
        return true;
    }

    match skill {
        Skill::Baby | Skill::Easy => skill_bits & MTF_EASY != 0,
        Skill::Medium => skill_bits & MTF_MEDIUM != 0,
        Skill::Hard | Skill::Nightmare => skill_bits & MTF_HARD != 0,
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Spawn all things from the level's THINGS lump into the game state.
///
/// Filters by skill level and multiplayer flag. Returns the player handle
/// if a player-1 start (DoomEd type 1) was found and spawned.
///
/// Player starts 2-4 (DoomEd types 2-4) are skipped in single-player
/// (`is_deathmatch == false`).
pub fn spawn_level_things(
    gs: &mut GameState,
    level: &Level,
    skill: Skill,
    is_deathmatch: bool,
) -> Option<MobjHandle> {
    let mut player_handle: Option<MobjHandle> = None;

    for thing in &level.things {
        // --- Skill filter ---
        if !should_spawn_for_skill(thing.flags, skill) {
            continue;
        }

        // --- Multiplayer filter ---
        if thing.flags & MTF_MULTIPLAYER != 0 && !is_deathmatch {
            continue;
        }

        // --- Map DoomEd type to MobjKind ---
        let kind = match doomed_type_to_kind(thing.kind) {
            Some(k) => k,
            None => continue, // Unknown thing type -- skip
        };

        // --- Angle conversion ---
        let angle = degrees_to_bam(thing.angle);
        let x = Fixed16_16::from_int(thing.x as i32);
        let y = Fixed16_16::from_int(thing.y as i32);

        // --- Player start ---
        if kind == MobjKind::Player {
            let mut mo = Mobj::new(kind, x, y, angle);
            apply_mobjinfo_defaults(&mut mo);
            sync_mobj_to_level(level, &mut mo);
            let handle = gs.mobjslab.alloc(mo);
            gs.player = PlayerState::pistol_start(handle);
            crate::weapons::setup_psprites(&mut gs.player);
            player_handle = Some(handle);
            continue;
        }

        // --- Non-player things (monsters, items, decorations) ---
        let mut mo = Mobj::new(kind, x, y, angle);
        apply_mobjinfo_defaults(&mut mo);
        if mo.tics > 0 && skill != Skill::Nightmare {
            mo.tics = 1 + i16::from(gs.p_random() % (mo.tics as u8));
        }
        sync_mobj_to_level(level, &mut mo);

        // Apply ambush flag from thing flags (deaf monsters).
        if thing.flags & MTF_AMBUSH != 0 {
            mo.flags |= flags::MF_AMBUSH;
        }

        // Save original spawn point for Nightmare respawning.
        mo.spawn_x = x;
        mo.spawn_y = y;
        mo.spawn_angle = angle;
        mo.spawn_type = thing.kind;

        let handle = gs.mobjslab.alloc(mo);

        // --- Track totals for intermission screen ---
        if let Some(spawned) = gs.mobjslab.get(handle) {
            if spawned.flags & flags::MF_COUNTKILL != 0 {
                gs.stats.total_kills += 1;
            }
            if spawned.flags & flags::MF_COUNTITEM != 0 {
                gs.stats.total_items += 1;
            }
        }
    }

    player_handle
}

// ---------------------------------------------------------------------------
// GameState convenience method
// ---------------------------------------------------------------------------

impl GameState {
    /// Spawn all things from the level and return the player handle.
    ///
    /// This is a convenience wrapper around [`spawn_level_things`] with
    /// `is_deathmatch = false`.
    pub fn spawn_things(&mut self, level: &Level, skill: Skill) -> Option<MobjHandle> {
        spawn_level_things(self, level, skill, false)
    }
}

// ---------------------------------------------------------------------------
// Nightmare respawn
// ---------------------------------------------------------------------------

/// Simulation tics per second (Doom runs at 35 Hz).
pub const TICRATE: u32 = 35;

/// Number of tics a dead monster waits before respawning on Nightmare.
///
/// 12 seconds * 35 tics/sec = 420 tics.
pub const NIGHTMARE_RESPAWN_TICS: i32 = 12 * TICRATE as i32;

/// Attempt to respawn a dead monster at its original spawn point.
///
/// On Nightmare difficulty, dead monsters (those with `MF_COUNTKILL` that
/// are in their death state) use `movecount` as a respawn timer.  Each tic,
/// `movecount` is incremented.  Once it reaches `NIGHTMARE_RESPAWN_TICS`
/// (420), the corpse is removed and a fresh monster is spawned at its
/// original map position with teleport fog effects.
///
/// Returns `true` if the monster respawned (corpse should be removed by
/// the caller), `false` if the timer is still counting or the mobj is
/// ineligible.
pub fn p_nightmare_respawn(gs: &mut GameState, level: Option<&Level>, handle: MobjHandle) -> bool {
    // Read all the data we need from the corpse before mutating.
    let (spawn_x, spawn_y, spawn_angle, spawn_type, corpse_x, corpse_y, movecount) = {
        let Some(mo) = gs.mobjslab.get(handle) else {
            return false;
        };

        // Must be a dead monster with a valid spawn type.
        if mo.spawn_type == 0 {
            return false;
        }

        (
            mo.spawn_x,
            mo.spawn_y,
            mo.spawn_angle,
            mo.spawn_type,
            mo.x,
            mo.y,
            mo.movecount,
        )
    };

    // Increment the respawn counter.
    if movecount < NIGHTMARE_RESPAWN_TICS {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.movecount += 1;
        }
        return false;
    }

    let kind = match doomed_type_to_kind(spawn_type) {
        Some(k) => k,
        None => return false,
    };

    let spawn_radius = MOBJINFO[kind as usize].radius;
    let spawn_blocked = gs.mobjslab.iter_handles().any(|other| {
        if other == handle {
            return false;
        }
        let Some(mo) = gs.mobjslab.get(other) else {
            return false;
        };
        if mo.flags & flags::MF_SOLID == 0 || mo.health <= 0 {
            return false;
        }

        let combined_radius = spawn_radius + mo.radius;
        (spawn_x - mo.x).abs() < combined_radius && (spawn_y - mo.y).abs() < combined_radius
    });
    if spawn_blocked {
        return false;
    }
    if let Some(level) = level
        && !crate::movement::p_try_move(&gs.mobjslab, handle, spawn_x, spawn_y, level)
    {
        return false;
    }

    // --- Timer expired: respawn the monster ---

    // Spawn teleport fog at the corpse location.
    let mut fog_corpse = Mobj::new(MobjKind::SpawnFire, corpse_x, corpse_y, Bam::ZERO);
    fog_corpse.state = crate::mobj::StateNum(crate::states::ids::S_TFOG1);
    if let Some(entry) = STATES.get(crate::states::ids::S_TFOG1 as usize) {
        fog_corpse.tics = entry.tics;
    }
    fog_corpse.flags = flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY;
    if let Some(level) = level {
        sync_mobj_to_level(level, &mut fog_corpse);
    }
    gs.mobjslab.alloc(fog_corpse);

    // Spawn teleport fog at the original spawn point.
    let mut fog_spawn = Mobj::new(MobjKind::SpawnFire, spawn_x, spawn_y, Bam::ZERO);
    fog_spawn.state = crate::mobj::StateNum(crate::states::ids::S_TFOG1);
    if let Some(entry) = STATES.get(crate::states::ids::S_TFOG1 as usize) {
        fog_spawn.tics = entry.tics;
    }
    fog_spawn.flags = flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY;
    if let Some(level) = level {
        sync_mobj_to_level(level, &mut fog_spawn);
    }
    gs.mobjslab.alloc(fog_spawn);

    // Spawn a fresh monster at the original position.
    let mut fresh = Mobj::new(kind, spawn_x, spawn_y, spawn_angle);
    apply_mobjinfo_defaults(&mut fresh);
    if let Some(level) = level {
        sync_mobj_to_level(level, &mut fresh);
    }

    // Carry over the spawn-point data so it can respawn again.
    fresh.spawn_x = spawn_x;
    fresh.spawn_y = spawn_y;
    fresh.spawn_angle = spawn_angle;
    fresh.spawn_type = spawn_type;

    gs.mobjslab.alloc(fresh);

    // Remove the corpse.
    gs.mobjslab.free(handle);

    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::flags;
    use doom_map::lumps::NODE_SUBSECTOR_BIT;
    use doom_map::{Node, NodeBBox, Thing};

    /// Build a minimal valid Level with the given things list and floor height.
    fn make_test_level_with_things_and_floor(things: Vec<Thing>, floor_height: i16) -> Level {
        // 1x1 blockmap at origin with one empty block.
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes()); // offset
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes()); // sentinel
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).unwrap();

        let reject = doom_map::Reject::parse_lump(&[0u8], 1).unwrap();

        Level {
            name: "TEST".to_string(),
            things,
            linedefs: vec![doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: doom_map::SIDEDEF_NONE,
            }],
            sidedefs: vec![doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"        ",
                lower_texture: *b"        ",
                middle_texture: *b"WALL1   ",
                sector: 0,
            }],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: 0 },
                doom_map::Vertex { x: 128, y: 0 },
            ],
            segs: vec![doom_map::Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            }],
            ssectors: vec![doom_map::Ssector {
                seg_count: 1,
                first_seg: 0,
            }],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height,
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

    /// Build a minimal valid Level with the given things list.
    fn make_test_level_with_things(things: Vec<Thing>) -> Level {
        make_test_level_with_things_and_floor(things, 0)
    }

    fn make_partition_test_level(things: Vec<Thing>, right_floor: i16, left_floor: i16) -> Level {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).unwrap();
        let reject = doom_map::Reject::parse_lump(&[0u8; 1], 2).unwrap();

        Level {
            name: "TEST".to_string(),
            things,
            linedefs: vec![
                doom_map::Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: doom_map::FLAG_TWO_SIDED,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
                doom_map::Linedef {
                    from_vertex: 2,
                    to_vertex: 3,
                    flags: doom_map::FLAG_TWO_SIDED,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
            ],
            sidedefs: vec![
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"        ",
                    lower_texture: *b"        ",
                    middle_texture: *b"WALL0   ",
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"        ",
                    lower_texture: *b"        ",
                    middle_texture: *b"WALL1   ",
                    sector: 1,
                },
            ],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: -128 },
                doom_map::Vertex { x: 0, y: 128 },
                doom_map::Vertex { x: 64, y: -128 },
                doom_map::Vertex { x: 64, y: 128 },
            ],
            segs: vec![
                doom_map::Seg {
                    from_vertex: 0,
                    to_vertex: 1,
                    angle: 0,
                    linedef: 0,
                    direction: 0,
                    offset: 0,
                },
                doom_map::Seg {
                    from_vertex: 3,
                    to_vertex: 2,
                    angle: 0,
                    linedef: 1,
                    direction: 1,
                    offset: 0,
                },
            ],
            ssectors: vec![
                doom_map::Ssector {
                    seg_count: 1,
                    first_seg: 0,
                },
                doom_map::Ssector {
                    seg_count: 1,
                    first_seg: 1,
                },
            ],
            nodes: vec![Node {
                x: 0,
                y: 0,
                dx: 0,
                dy: 1,
                right_bbox: NodeBBox {
                    ymax: 128,
                    ymin: -128,
                    xmin: 0,
                    xmax: 128,
                },
                left_bbox: NodeBBox {
                    ymax: 128,
                    ymin: -128,
                    xmin: -128,
                    xmax: 0,
                },
                right_child: NODE_SUBSECTOR_BIT,
                left_child: NODE_SUBSECTOR_BIT | 1,
            }],
            sectors: vec![
                doom_map::Sector {
                    floor_height: right_floor,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: left_floor,
                    ceil_height: 192,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap,
        }
    }

    // ===================================================================
    // Player spawning
    // ===================================================================

    #[test]
    fn spawn_level_things_creates_player() {
        let level = make_test_level_with_things(vec![Thing {
            x: 100,
            y: 200,
            angle: 90,
            kind: 1,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, false);
        assert!(handle.is_some());
        let mo = gs.mobjslab.get(handle.unwrap()).unwrap();
        assert_eq!(mo.kind, MobjKind::Player);
        assert_eq!(mo.x, Fixed16_16::from_int(100));
        assert_eq!(mo.y, Fixed16_16::from_int(200));
    }

    #[test]
    fn spawn_player_sets_player_state() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, false);
        assert!(handle.is_some());
        assert_eq!(gs.player.handle, handle.unwrap());
        assert_eq!(gs.player.health(), 100);
    }

    #[test]
    fn spawn_player_applies_mobjinfo() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, false).unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();
        // Player MOBJINFO: health=100, radius=16, height=56
        assert_eq!(mo.health, 100);
        assert_eq!(mo.radius, Fixed16_16::from_int(16));
        assert_eq!(mo.height, Fixed16_16::from_int(56));
    }

    #[test]
    fn spawn_player_snaps_to_sector_floor_height() {
        let level = make_test_level_with_things_and_floor(
            vec![Thing {
                x: 32,
                y: 0,
                angle: 0,
                kind: 1,
                flags: 7,
            }],
            24,
        );
        let mut gs = GameState::new("E1M1");

        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, false).unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();

        assert_eq!(mo.z, Fixed16_16::from_int(24));
    }

    #[test]
    fn spawn_nonplayer_snaps_to_sector_floor_height() {
        let level = make_test_level_with_things_and_floor(
            vec![Thing {
                x: 64,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 7,
            }],
            -32,
        );
        let mut gs = GameState::new("E1M1");

        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let trooper = gs
            .mobjslab
            .iter_handles()
            .find_map(|h| gs.mobjslab.get(h).filter(|mo| mo.kind == MobjKind::Trooper))
            .expect("trooper should spawn");

        assert_eq!(trooper.z, Fixed16_16::from_int(-32));
    }

    #[test]
    fn spawn_nonplayer_on_partition_line_uses_doom_subsector_tiebreak_for_floor() {
        let level = make_partition_test_level(
            vec![Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 7,
            }],
            0,
            64,
        );
        let mut gs = GameState::new("E1M1");

        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let trooper = gs
            .mobjslab
            .iter_handles()
            .find_map(|h| gs.mobjslab.get(h).filter(|mo| mo.kind == MobjKind::Trooper))
            .expect("trooper should spawn");

        assert_eq!(
            trooper.z,
            Fixed16_16::from_int(64),
            "thing on a partition line should inherit the floor from Doom's chosen subsector"
        );
        assert_eq!(
            trooper.subsector, 1,
            "partition-line thing should resolve to the left subsector"
        );
    }

    #[test]
    fn sync_mobj_to_level_honors_spawnceiling() {
        let mut level = make_test_level_with_things(vec![]);
        level.sectors[0].ceil_height = 128;

        let mut mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(32),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        apply_mobjinfo_defaults(&mut mo);
        mo.flags |= flags::MF_SPAWNCEILING;

        sync_mobj_to_level(&level, &mut mo);

        assert_eq!(
            mo.z,
            Fixed16_16::from_int(72),
            "spawnceiling thing should hang from ceiling minus actor height"
        );
    }

    #[test]
    fn spawn_nonplayer_randomizes_positive_spawn_tics_outside_nightmare() {
        let level = make_test_level_with_things(vec![Thing {
            x: 64,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        gs.rng.set_index(3); // 220 % 10 = 0, so Doom-style randomized tics should become 1.

        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let trooper = gs
            .mobjslab
            .iter_handles()
            .find_map(|h| gs.mobjslab.get(h).filter(|mo| mo.kind == MobjKind::Trooper))
            .expect("trooper should spawn");

        assert_eq!(
            trooper.tics, 1,
            "non-Nightmare map thing spawn should randomize initial positive tics"
        );
    }

    // ===================================================================
    // Skill filtering
    // ===================================================================

    #[test]
    fn spawn_filters_by_skill_easy_only() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 1,
            }, // easy only (Trooper)
            Thing {
                x: 100,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 4,
            }, // hard only
        ]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Easy, false);
        // Only the easy-skill trooper should exist.
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn spawn_filters_by_skill_hard_only() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 1,
            }, // easy only
            Thing {
                x: 100,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 4,
            }, // hard only
        ]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Hard, false);
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn spawn_all_skills_when_bits_are_7() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        for skill in [
            Skill::Baby,
            Skill::Easy,
            Skill::Medium,
            Skill::Hard,
            Skill::Nightmare,
        ] {
            let mut gs = GameState::new("TEST");
            spawn_level_things(&mut gs, &level, skill, false);
            let count = gs
                .mobjslab
                .iter_handles()
                .filter(|&h| {
                    gs.mobjslab
                        .get(h)
                        .map(|m| m.kind == MobjKind::Trooper)
                        .unwrap_or(false)
                })
                .count();
            assert_eq!(
                count, 1,
                "Skill {:?} should spawn thing with flags=7",
                skill
            );
        }
    }

    #[test]
    fn spawn_when_no_skill_bits_set() {
        // Some WADs have things with flags & 7 == 0; they should spawn anyway.
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 0,
        }]);
        let mut gs = GameState::new("TEST");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 1, "Things with no skill bits should spawn anyway");
    }

    // ===================================================================
    // Multiplayer filter
    // ===================================================================

    #[test]
    fn spawn_filters_multiplayer_only() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7 | 16,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false); // singleplayer
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 0, "Multiplayer-only thing should be filtered in SP");
    }

    #[test]
    fn spawn_multiplayer_in_deathmatch() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7 | 16,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, true); // deathmatch
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 1, "Multiplayer-only thing should spawn in DM");
    }

    // ===================================================================
    // Ambush flag
    // ===================================================================

    #[test]
    fn spawn_sets_ambush_flag() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7 | 8,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();
        assert!(
            mo.flags & flags::MF_AMBUSH != 0,
            "MF_AMBUSH should be set for deaf monsters"
        );
    }

    #[test]
    fn spawn_no_ambush_when_not_flagged() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();
        assert!(
            mo.flags & flags::MF_AMBUSH == 0,
            "MF_AMBUSH should NOT be set without thing flag bit 3"
        );
    }

    // ===================================================================
    // Kill/item counters
    // ===================================================================

    #[test]
    fn spawn_counts_kills_and_items() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 7,
            }, // Trooper (MF_COUNTKILL)
            Thing {
                x: 100,
                y: 0,
                angle: 0,
                kind: 2014,
                flags: 7,
            }, // HealthBonus (MF_COUNTITEM)
        ]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        assert_eq!(gs.stats.total_kills, 1, "Trooper should count as a kill");
        assert_eq!(
            gs.stats.total_items, 1,
            "HealthBonus should count as an item"
        );
    }

    #[test]
    fn spawn_counts_multiple_kills() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 7,
            }, // Trooper
            Thing {
                x: 100,
                y: 0,
                angle: 0,
                kind: 9,
                flags: 7,
            }, // Sergeant
            Thing {
                x: 200,
                y: 0,
                angle: 0,
                kind: 3001,
                flags: 7,
            }, // Imp
        ]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        assert_eq!(
            gs.stats.total_kills, 3,
            "All three monsters should be counted"
        );
    }

    // ===================================================================
    // MOBJINFO defaults
    // ===================================================================

    #[test]
    fn spawn_applies_mobjinfo_defaults() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();
        // Trooper MOBJINFO: health=20, radius=20, height=56, MF_SOLID|MF_SHOOTABLE|MF_COUNTKILL
        assert_eq!(mo.health, 20);
        assert_eq!(mo.radius, Fixed16_16::from_int(20));
        assert_eq!(mo.height, Fixed16_16::from_int(56));
        assert!(mo.flags & flags::MF_SOLID != 0);
        assert!(mo.flags & flags::MF_SHOOTABLE != 0);
        assert!(mo.flags & flags::MF_COUNTKILL != 0);
    }

    #[test]
    fn spawn_applies_barrel_defaults() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 2035,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Barrel)
                    .unwrap_or(false)
            })
            .unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.health, 20, "Barrel health should be 20");
        assert!(mo.flags & flags::MF_SOLID != 0, "Barrel should be solid");
        assert!(
            mo.flags & flags::MF_SHOOTABLE != 0,
            "Barrel should be shootable"
        );
    }

    // ===================================================================
    // Angle conversion
    // ===================================================================

    #[test]
    fn spawn_converts_angle_to_bam() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 180,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();
        // 180 degrees should be approximately ANG180 (0x8000_0000)
        let expected = Bam(0x8000_0000);
        let diff = mo.angle.0.wrapping_sub(expected.0);
        assert!(
            !(0x0100_0000..=0xFF00_0000).contains(&diff),
            "180 degrees should convert to approximately ANG180, got {:08X}",
            mo.angle.0
        );
    }

    #[test]
    fn spawn_converts_angle_0_degrees() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();
        assert_eq!(mo.angle.0, 0, "0 degrees should be BAM 0");
    }

    #[test]
    fn spawn_converts_angle_90_degrees() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 90,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();
        // 90 degrees = ANG90 = 0x4000_0000
        let expected = Bam(0x4000_0000);
        let diff = mo.angle.0.wrapping_sub(expected.0);
        assert!(
            !(0x0100_0000..=0xFF00_0000).contains(&diff),
            "90 degrees should convert to approximately ANG90, got {:08X}",
            mo.angle.0
        );
    }

    // ===================================================================
    // GameState convenience method
    // ===================================================================

    #[test]
    fn gs_spawn_things_convenience() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 50,
                y: 50,
                angle: 0,
                kind: 1,
                flags: 7,
            },
            Thing {
                x: 100,
                y: 100,
                angle: 45,
                kind: 3004,
                flags: 7,
            },
        ]);
        let mut gs = GameState::new("E1M1");
        let player = gs.spawn_things(&level, Skill::Medium);
        assert!(player.is_some(), "Player should be spawned");
        // Should have player + trooper = 2 actors
        assert_eq!(gs.mobjslab.len(), 2);
    }

    // ===================================================================
    // Unknown thing types
    // ===================================================================

    #[test]
    fn spawn_skips_unknown_thing_types() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 9999,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, false);
        assert!(handle.is_none(), "No player should be spawned");
        assert!(gs.mobjslab.is_empty(), "Unknown type should be skipped");
    }

    // ===================================================================
    // Empty level
    // ===================================================================

    #[test]
    fn spawn_empty_level() {
        let level = make_test_level_with_things(vec![]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, false);
        assert!(handle.is_none());
        assert!(gs.mobjslab.is_empty());
        assert_eq!(gs.stats.total_kills, 0);
        assert_eq!(gs.stats.total_items, 0);
    }

    // ===================================================================
    // Spawn state from MOBJINFO
    // ===================================================================

    #[test]
    fn spawn_sets_spawn_state_from_mobjinfo() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();
        let expected_state = MOBJINFO[MobjKind::Trooper as usize].spawn_state;
        assert_eq!(mo.state, expected_state);
    }

    // ===================================================================
    // Skill filter unit tests
    // ===================================================================

    #[test]
    fn should_spawn_for_skill_easy_bit() {
        assert!(should_spawn_for_skill(1, Skill::Baby));
        assert!(should_spawn_for_skill(1, Skill::Easy));
        assert!(!should_spawn_for_skill(1, Skill::Medium));
        assert!(!should_spawn_for_skill(1, Skill::Hard));
        assert!(!should_spawn_for_skill(1, Skill::Nightmare));
    }

    #[test]
    fn should_spawn_for_skill_medium_bit() {
        assert!(!should_spawn_for_skill(2, Skill::Baby));
        assert!(!should_spawn_for_skill(2, Skill::Easy));
        assert!(should_spawn_for_skill(2, Skill::Medium));
        assert!(!should_spawn_for_skill(2, Skill::Hard));
        assert!(!should_spawn_for_skill(2, Skill::Nightmare));
    }

    #[test]
    fn should_spawn_for_skill_hard_bit() {
        assert!(!should_spawn_for_skill(4, Skill::Baby));
        assert!(!should_spawn_for_skill(4, Skill::Easy));
        assert!(!should_spawn_for_skill(4, Skill::Medium));
        assert!(should_spawn_for_skill(4, Skill::Hard));
        assert!(should_spawn_for_skill(4, Skill::Nightmare));
    }

    #[test]
    fn should_spawn_for_skill_all_bits() {
        for skill in [
            Skill::Baby,
            Skill::Easy,
            Skill::Medium,
            Skill::Hard,
            Skill::Nightmare,
        ] {
            assert!(
                should_spawn_for_skill(7, skill),
                "flags=7 should spawn at all skills"
            );
        }
    }

    #[test]
    fn should_spawn_for_skill_zero_bits() {
        for skill in [
            Skill::Baby,
            Skill::Easy,
            Skill::Medium,
            Skill::Hard,
            Skill::Nightmare,
        ] {
            assert!(
                should_spawn_for_skill(0, skill),
                "flags=0 should spawn at all skills (WAD compat)"
            );
        }
    }

    // ===================================================================
    // Degrees to BAM conversion
    // ===================================================================

    #[test]
    fn degrees_to_bam_conversion() {
        assert_eq!(degrees_to_bam(0).0, 0);
        // 90 degrees ~ 0x4000_0000
        let bam90 = degrees_to_bam(90);
        let diff90 = bam90.0.wrapping_sub(0x4000_0000);
        assert!(!(0x0100_0000..=0xFF00_0000).contains(&diff90));
        // 180 degrees ~ 0x8000_0000
        let bam180 = degrees_to_bam(180);
        let diff180 = bam180.0.wrapping_sub(0x8000_0000);
        assert!(!(0x0100_0000..=0xFF00_0000).contains(&diff180));
        // 270 degrees ~ 0xC000_0000
        let bam270 = degrees_to_bam(270);
        let diff270 = bam270.0.wrapping_sub(0xC000_0000);
        assert!(!(0x0100_0000..=0xFF00_0000).contains(&diff270));
    }

    // ===================================================================
    // Nightmare respawn
    // ===================================================================

    /// Create a dead Trooper corpse with spawn-point data set.
    fn make_dead_trooper_corpse(gs: &mut GameState) -> crate::mobj::MobjHandle {
        let spawn_x = Fixed16_16::from_int(200);
        let spawn_y = Fixed16_16::from_int(300);
        let spawn_angle = degrees_to_bam(90);

        let mut mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(500),
            Fixed16_16::from_int(600),
            Bam::ZERO,
        );
        crate::spawn::apply_mobjinfo_defaults(&mut mo);

        // Simulate death: zero health, set MF_COUNTKILL (already set by mobjinfo).
        mo.health = 0;
        mo.movecount = 0;

        // Set spawn point data.
        mo.spawn_x = spawn_x;
        mo.spawn_y = spawn_y;
        mo.spawn_angle = spawn_angle;
        mo.spawn_type = 3004; // DoomEd type for Trooper

        gs.mobjslab.alloc(mo)
    }

    #[test]
    fn nightmare_respawn_constants() {
        assert_eq!(TICRATE, 35);
        assert_eq!(NIGHTMARE_RESPAWN_TICS, 420);
    }

    #[test]
    fn nightmare_respawn_timer_increments() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);

        // First call: movecount goes from 0 to 1, returns false.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
        assert_eq!(gs.mobjslab.get(handle).unwrap().movecount, 1);

        // Second call: movecount goes to 2.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
        assert_eq!(gs.mobjslab.get(handle).unwrap().movecount, 2);
    }

    #[test]
    fn nightmare_respawn_triggers_at_420_tics() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);

        // Set movecount just below threshold.
        gs.mobjslab.get_mut(handle).unwrap().movecount = NIGHTMARE_RESPAWN_TICS - 1;

        // One more increment, still not at threshold.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
        assert_eq!(
            gs.mobjslab.get(handle).unwrap().movecount,
            NIGHTMARE_RESPAWN_TICS
        );

        // Now at threshold — respawn should happen.
        assert!(p_nightmare_respawn(&mut gs, None, handle));

        // Original corpse handle should be freed.
        assert!(gs.mobjslab.get(handle).is_none());
    }

    #[test]
    fn nightmare_respawn_creates_fresh_monster_at_spawn_point() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);

        // Set movecount to threshold so respawn fires immediately.
        gs.mobjslab.get_mut(handle).unwrap().movecount = NIGHTMARE_RESPAWN_TICS;

        let initial_count = gs.mobjslab.len();
        assert!(p_nightmare_respawn(&mut gs, None, handle));

        // Corpse removed, but fresh monster + 2 fog effects added.
        // Net: -1 corpse + 1 monster + 2 fog = +2
        assert_eq!(gs.mobjslab.len(), initial_count + 2);

        // Find the freshly-spawned Trooper (not SpawnFire).
        let fresh_handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("Fresh trooper should exist after respawn");

        let fresh = gs.mobjslab.get(fresh_handle).unwrap();

        // Verify spawn point position.
        assert_eq!(fresh.x, Fixed16_16::from_int(200));
        assert_eq!(fresh.y, Fixed16_16::from_int(300));

        // Verify full health restored.
        assert_eq!(fresh.health, 20); // Trooper spawn_health = 20

        // Verify spawn data carried over for re-respawning.
        assert_eq!(fresh.spawn_x, Fixed16_16::from_int(200));
        assert_eq!(fresh.spawn_y, Fixed16_16::from_int(300));
        assert_eq!(fresh.spawn_type, 3004);
    }

    #[test]
    fn nightmare_respawn_snaps_fresh_monster_and_fog_to_sector_floor() {
        let level = make_test_level_with_things_and_floor(vec![], 40);
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab.get_mut(handle).unwrap().movecount = NIGHTMARE_RESPAWN_TICS;

        assert!(p_nightmare_respawn(&mut gs, Some(&level), handle));

        let fresh = gs
            .mobjslab
            .iter_handles()
            .find_map(|h| gs.mobjslab.get(h).filter(|mo| mo.kind == MobjKind::Trooper))
            .expect("fresh trooper should exist after respawn");
        assert_eq!(fresh.z, Fixed16_16::from_int(40));

        let fog_zs: Vec<Fixed16_16> = gs
            .mobjslab
            .iter_handles()
            .filter_map(|h| {
                gs.mobjslab
                    .get(h)
                    .filter(|mo| mo.kind == MobjKind::SpawnFire)
                    .map(|mo| mo.z)
            })
            .collect();
        assert_eq!(fog_zs.len(), 2);
        assert!(fog_zs.iter().all(|&z| z == Fixed16_16::from_int(40)));
    }

    #[test]
    fn nightmare_respawn_spawns_teleport_fog() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab.get_mut(handle).unwrap().movecount = NIGHTMARE_RESPAWN_TICS;

        assert!(p_nightmare_respawn(&mut gs, None, handle));

        // Should have exactly 2 SpawnFire fog effects.
        let fog_count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::SpawnFire)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(fog_count, 2, "Should spawn fog at corpse and spawn point");
    }

    #[test]
    fn nightmare_respawn_fails_when_spawn_spot_is_blocked_by_solid_actor() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab.get_mut(handle).unwrap().movecount = NIGHTMARE_RESPAWN_TICS;

        let mut blocker = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(200),
            Fixed16_16::from_int(300),
            Bam::ZERO,
        );
        apply_mobjinfo_defaults(&mut blocker);
        blocker.health = 20;
        blocker.flags |= flags::MF_SOLID;
        gs.mobjslab.alloc(blocker);

        let initial_count = gs.mobjslab.len();
        assert!(
            !p_nightmare_respawn(&mut gs, None, handle),
            "blocked spawn spot should prevent Nightmare respawn"
        );
        assert!(
            gs.mobjslab.get(handle).is_some(),
            "corpse should remain when respawn is blocked"
        );
        assert_eq!(
            gs.mobjslab.len(),
            initial_count,
            "blocked respawn must not spawn fog or a fresh monster"
        );
    }

    #[test]
    fn nightmare_respawn_no_action_without_spawn_type() {
        let mut gs = GameState::new("TEST");
        let mut mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 0;
        mo.flags = flags::MF_COUNTKILL;
        mo.spawn_type = 0; // No spawn type — cannot respawn.
        let handle = gs.mobjslab.alloc(mo);

        // Should return false because spawn_type == 0.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
    }

    #[test]
    fn nightmare_respawn_does_not_happen_below_threshold() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);

        // Run 419 tics — should not respawn.
        for _ in 0..419 {
            assert!(!p_nightmare_respawn(&mut gs, None, handle));
        }

        // Monster should still be alive in the slab (corpse).
        assert!(gs.mobjslab.get(handle).is_some());
        assert_eq!(gs.mobjslab.get(handle).unwrap().movecount, 419);

        // 420th call pushes to threshold.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
        assert_eq!(
            gs.mobjslab.get(handle).unwrap().movecount,
            NIGHTMARE_RESPAWN_TICS
        );

        // 421st call (at threshold) triggers respawn.
        assert!(p_nightmare_respawn(&mut gs, None, handle));
        assert!(gs.mobjslab.get(handle).is_none());
    }

    #[test]
    fn tick_all_mobjs_nightmare_triggers_respawn() {
        let mut gs = GameState::new("TEST");
        gs.skill = Skill::Nightmare;

        // Spawn a player so tick_all_mobjs can skip it.
        let player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let player_handle = gs.mobjslab.alloc(player_mo);
        gs.player = crate::player::PlayerState::pistol_start(player_handle);

        // Spawn a dead trooper with spawn data.
        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab.get_mut(handle).unwrap().movecount = NIGHTMARE_RESPAWN_TICS;

        // Run tick_all_mobjs — should trigger respawn on Nightmare.
        crate::tic::tick_all_mobjs(&mut gs, None);

        // Corpse should be gone.
        assert!(gs.mobjslab.get(handle).is_none());

        // Fresh trooper should exist.
        let has_trooper = gs.mobjslab.iter_handles().any(|h| {
            gs.mobjslab
                .get(h)
                .map(|m| m.kind == MobjKind::Trooper && m.health > 0)
                .unwrap_or(false)
        });
        assert!(
            has_trooper,
            "Fresh trooper should exist after Nightmare respawn"
        );
    }

    #[test]
    fn tick_all_mobjs_no_respawn_on_lower_skill() {
        let mut gs = GameState::new("TEST");
        gs.skill = Skill::Hard; // Not Nightmare

        let player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let player_handle = gs.mobjslab.alloc(player_mo);
        gs.player = crate::player::PlayerState::pistol_start(player_handle);

        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab.get_mut(handle).unwrap().movecount = NIGHTMARE_RESPAWN_TICS;

        // Run tick_all_mobjs on Hard — should NOT trigger respawn.
        crate::tic::tick_all_mobjs(&mut gs, None);

        // Corpse should still be there (not respawned).
        assert!(gs.mobjslab.get(handle).is_some());
    }

    #[test]
    fn spawn_level_things_saves_spawn_point() {
        let level = make_test_level_with_things(vec![Thing {
            x: 150,
            y: 250,
            angle: 45,
            kind: 3004, // Trooper
            flags: 7,
        }]);
        let mut gs = GameState::new("TEST");
        spawn_level_things(&mut gs, &level, Skill::Medium, false);

        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .unwrap();
        let mo = gs.mobjslab.get(handle).unwrap();

        assert_eq!(mo.spawn_x, Fixed16_16::from_int(150));
        assert_eq!(mo.spawn_y, Fixed16_16::from_int(250));
        assert_eq!(mo.spawn_type, 3004);
    }
}
