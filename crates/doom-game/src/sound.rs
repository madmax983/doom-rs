//! Sound propagation through sectors (P_NoiseAlert).
//!
//! When the player fires a weapon, sound flood-fills through connected sectors
//! via two-sided linedefs.  Monsters in reached sectors may wake up depending
//! on whether they have the `MF_AMBUSH` flag.
//!
//! # Vanilla Doom behavior
//! - Sound starts at the emitter's sector and spreads through two-sided linedefs.
//! - A linedef with the `ML_SOUNDBLOCK` flag counts as one "sound block".
//! - Sound can pass through one sound block linedef total, but not two.
//! - Each reached sector records the target (typically the player) as its
//!   sound target.  Monster AI checks this during `A_Look`.
//! - The flood fill uses a generation counter to avoid revisiting sectors
//!   within the same alert, and to avoid clearing the traversal array each time.

use doom_map::Level;

use crate::mobj::MobjHandle;
use crate::mobj::flags::MF_AMBUSH;
use crate::sight::{p_check_sight, sector_from_position_or_subsector};
use crate::state::GameState;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Linedef flag: blocks sound propagation.
///
/// In vanilla Doom, this is bit 6 (0x0040) of the linedef flags field.
/// Sound can pass through one ML_SOUNDBLOCK line but not two consecutive ones.
pub const ML_SOUNDBLOCK: u16 = 0x0040;

// ---------------------------------------------------------------------------
// Sound state initialization
// ---------------------------------------------------------------------------

/// Initialize (or resize) the sound propagation state on `GameState`.
///
/// Must be called when a level is loaded, passing the number of sectors in
/// the level.  Resets all sound targets to `None` and all traversal counters
/// to 0.
pub fn init_sound_state(gs: &mut GameState, num_sectors: usize) {
    gs.sound.sound_targets.clear();
    gs.sound.sound_targets.resize(num_sectors, None);
    gs.sound.sound_traversed.clear();
    gs.sound.sound_traversed.resize(num_sectors, 0);
    gs.sound.sound_gen = 0;
}

// ---------------------------------------------------------------------------
// Sound target accessors
// ---------------------------------------------------------------------------

/// Return the current sound target for a sector.
///
/// Returns `None` if no noise has reached this sector, or if the sector index
/// is out of range.
pub fn get_sound_target(gs: &GameState, sector_index: usize) -> Option<MobjHandle> {
    gs.sound.sound_targets.get(sector_index).copied().flatten()
}

/// Reset all sound targets to `None`.
///
/// Called on level transitions to clear stale sound state.
pub fn clear_sound_targets(gs: &mut GameState) {
    gs.sound.sound_targets.fill(None);
}

// ---------------------------------------------------------------------------
// Sector adjacency
// ---------------------------------------------------------------------------

/// Return all sector indices connected to `sector_index` via two-sided linedefs.
///
/// Walks every linedef in the level and checks if either sidedef faces into
/// `sector_index`.  For two-sided linedefs, the sector on the *other* side is
/// included in the result.
///
/// Returned indices are deduplicated but not sorted.
pub fn adjacent_sectors(level: &Level, sector_index: usize) -> Vec<usize> {
    let mut result = Vec::new();

    for ld in &level.linedefs {
        // Only two-sided linedefs connect sectors.
        if !ld.is_two_sided() {
            continue;
        }

        let right_sd = match level.sidedefs.get(ld.right_sidedef as usize) {
            Some(sd) => sd,
            None => continue,
        };
        let left_sd = match level.sidedefs.get(ld.left_sidedef as usize) {
            Some(sd) => sd,
            None => continue,
        };

        let right_sector = right_sd.sector as usize;
        let left_sector = left_sd.sector as usize;

        if right_sector == sector_index && left_sector != sector_index {
            result.push(left_sector);
        } else if left_sector == sector_index && right_sector != sector_index {
            result.push(right_sector);
        }
    }

    // Deduplicate the result once at the end instead of O(N^2) `.contains()` calls
    result.sort_unstable();
    result.dedup();

    result
}

// ---------------------------------------------------------------------------
// P_NoiseAlert — sound propagation flood fill
// ---------------------------------------------------------------------------

/// Propagate sound from an emitter (typically the player's weapon fire) through
/// connected sectors.
///
/// - `target` is the actor that monsters should chase (usually the player).
/// - `emitter` is the actor that made the noise (also usually the player).
///
/// Sound flood-fills from the emitter's sector through two-sided linedefs.
/// Linedefs with `ML_SOUNDBLOCK` count against the propagation budget:
/// sound can cross one such line total (matching vanilla Doom).
///
/// After this call, `gs.sound.sound_targets[sector]` will be `Some(target)` for
/// every reached sector.
pub fn p_noise_alert(gs: &mut GameState, level: &Level, target: MobjHandle, emitter: MobjHandle) {
    // Determine the emitter's sector from its current position.
    let emitter_sector = {
        let mo = match gs.mobjslab.get(emitter) {
            Some(mo) => mo,
            None => return, // stale handle, bail gracefully
        };
        match sector_from_position_or_subsector(level, mo.x, mo.y, mo.subsector as usize) {
            Some(s) => s,
            None => return, // can't resolve sector, bail
        }
    };

    // Increment generation counter to mark a new flood fill pass.
    gs.sound.sound_gen = gs.sound.sound_gen.wrapping_add(1);
    // If generation wrapped to 0, reset all traversed counters so the
    // comparison `traversed[s] >= gen` works correctly.
    if gs.sound.sound_gen == 0 {
        gs.sound.sound_traversed.fill(0);
        gs.sound.sound_gen = 1;
    }

    let new_gen = gs.sound.sound_gen;

    // Start flood fill from the emitter's sector with one soundblock crossing
    // available. Crossing a second `ML_SOUNDBLOCK` stops propagation.
    recursive_sound(gs, level, emitter_sector, 1, target, new_gen);
}

/// Recursive flood fill: propagate sound into `sector_idx` and its neighbors.
///
/// `sound_blocks_remaining` tracks how many `ML_SOUNDBLOCK` linedefs sound
/// can still pass through (starts at 1, decremented by each soundblock line).
fn recursive_sound(
    gs: &mut GameState,
    level: &Level,
    sector_idx: usize,
    sound_blocks_remaining: i32,
    target: MobjHandle,
    new_gen: u32,
) {
    // Bounds check.
    if sector_idx >= gs.sound.sound_traversed.len() {
        return;
    }

    // Already visited this generation with at least as much budget?
    // The generation check prevents revisiting in the same pass.
    if gs.sound.sound_traversed[sector_idx] >= new_gen {
        return;
    }

    // Mark as visited and set the sound target.
    gs.sound.sound_traversed[sector_idx] = new_gen;
    gs.sound.sound_targets[sector_idx] = Some(target);

    // Propagate through two-sided linedefs bounding this sector.
    for ld in &level.linedefs {
        if !ld.is_two_sided() {
            continue;
        }

        let right_sd = match level.sidedefs.get(ld.right_sidedef as usize) {
            Some(sd) => sd,
            None => continue,
        };
        let left_sd = match level.sidedefs.get(ld.left_sidedef as usize) {
            Some(sd) => sd,
            None => continue,
        };

        let right_sector = right_sd.sector as usize;
        let left_sector = left_sd.sector as usize;

        // Check if this linedef bounds our current sector.
        let other_sector = if right_sector == sector_idx {
            left_sector
        } else if left_sector == sector_idx {
            right_sector
        } else {
            continue;
        };

        // Calculate sound block cost for this linedef.
        let blocks = if ld.flags & ML_SOUNDBLOCK != 0 { 1 } else { 0 };
        let remaining = sound_blocks_remaining - blocks;

        if remaining >= 0 {
            recursive_sound(gs, level, other_sector, remaining, target, new_gen);
        }
    }
}

// ---------------------------------------------------------------------------
// Monster wake-up helper
// ---------------------------------------------------------------------------

/// Check whether a monster should wake up based on sound propagation.
///
/// Returns `true` if:
/// - The actor's sector has a sound target, AND
/// - Either the actor does NOT have `MF_AMBUSH` (deaf) flag, OR
/// - The actor has `MF_AMBUSH` but also has line-of-sight to the sound target.
///
/// This helper is intended to be called from `A_Look` or similar monster AI
/// functions.
pub fn monster_should_wake(gs: &GameState, level: &Level, actor_handle: MobjHandle) -> bool {
    let mo = match gs.mobjslab.get(actor_handle) {
        Some(mo) => mo,
        None => return false,
    };

    // Resolve the actor's sector from its current position.
    let actor_sector =
        match sector_from_position_or_subsector(level, mo.x, mo.y, mo.subsector as usize) {
            Some(s) => s,
            None => return false,
        };

    // Check if there's a sound target in this sector.
    let sound_target = match get_sound_target(gs, actor_sector) {
        Some(t) => t,
        None => return false,
    };

    // If the monster has MF_AMBUSH, it only wakes from sound if it also has
    // line-of-sight to the target.
    let is_ambush = mo.flags & MF_AMBUSH != 0;
    if is_ambush {
        // Need LOS to wake up.
        p_check_sight(gs, level, actor_handle, sound_target)
    } else {
        // Non-ambush monsters wake from sound alone.
        true
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjHandle, MobjKind, flags};
    use doom_map::Level;
    use doom_map::lumps::*;
    use doom_types::{Bam, Fixed16_16};

    // -- Test helpers -------------------------------------------------------

    /// Build a minimal Level with `n` sectors, configurable linedefs,
    /// and matching sidedefs.
    ///
    /// `connections` describes two-sided linedefs: `(sector_a, sector_b, flags)`.
    ///
    /// A one-sided "anchor" linedef for sector 0 is always created so that
    /// subsector 0 -> seg 0 -> linedef 0 -> right sidedef -> sector 0 resolves
    /// correctly via `sector_from_subsector`.
    fn make_test_level(n_sectors: usize, connections: &[(usize, usize, u16)]) -> Level {
        let sectors: Vec<Sector> = (0..n_sectors)
            .map(|_| Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            })
            .collect();

        let mut sidedefs = Vec::new();
        let mut linedefs = Vec::new();

        // Anchor linedef 0: one-sided wall for sector 0.
        // This ensures subsector 0 → seg 0 → linedef 0 → sidedef 0 → sector 0.
        let anchor_sd = sidedefs.len() as u16;
        sidedefs.push(Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: [0; 8],
            lower_texture: [0; 8],
            middle_texture: *b"WALL1\0\0\0",
            sector: 0,
        });
        linedefs.push(Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0, // one-sided
            special: 0,
            tag: 0,
            right_sidedef: anchor_sd,
            left_sidedef: SIDEDEF_NONE,
        });

        for &(a, b, ld_flags) in connections {
            let right_sd = sidedefs.len() as u16;
            sidedefs.push(Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: a as u16,
            });
            let left_sd = sidedefs.len() as u16;
            sidedefs.push(Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: b as u16,
            });
            linedefs.push(Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: FLAG_TWO_SIDED | ld_flags,
                special: 0,
                tag: 0,
                right_sidedef: right_sd,
                left_sidedef: left_sd,
            });
        }

        // Minimal geometry: 2 vertices, 1 seg, 1 ssector, 0 nodes.
        let vertexes = vec![Vertex { x: 0, y: 0 }, Vertex { x: 64, y: 0 }];

        // One seg referencing linedef 0 (the anchor).
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];

        let ssectors = vec![Ssector {
            seg_count: 1,
            first_seg: 0,
        }];

        let reject_size = (n_sectors * n_sectors).div_ceil(8);
        let reject_data = vec![0u8; reject_size]; // all visible
        let reject = Reject::parse_lump(&reject_data, n_sectors).unwrap();

        // Minimal blockmap.
        let mut bm_data = vec![0u8; 14];
        bm_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        bm_data[2..4].copy_from_slice(&0i16.to_le_bytes());
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).unwrap();

        Level {
            name: "TEST".to_string(),
            things: Vec::new(),
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: Vec::new(),
            sectors,
            reject,
            blockmap,
        }
    }

    /// Create a GameState with sound state initialized for `n_sectors` and
    /// a player mobj at `(0, 0)` in subsector 0.
    fn make_game_state_with_sound(n_sectors: usize) -> GameState {
        let mut gs = GameState::new("TEST");
        init_sound_state(&mut gs, n_sectors);

        // Spawn player at (0, 0), subsector 0.
        let mut player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        player_mo.health = 100;
        player_mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        player_mo.subsector = 0;
        let player_handle = gs.mobjslab.alloc(player_mo);
        gs.player.handle = player_handle;

        gs
    }

    /// Spawn a monster at `(x, y)` in the given subsector, returning its handle.
    fn spawn_monster(
        gs: &mut GameState,
        kind: MobjKind,
        x: i32,
        y: i32,
        subsector: u32,
        extra_flags: u32,
    ) -> MobjHandle {
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE | extra_flags;
        mo.subsector = subsector;
        gs.mobjslab.alloc(mo)
    }

    // -- Tests --------------------------------------------------------------

    #[test]
    fn init_sound_state_creates_correct_sized_vectors() {
        let mut gs = GameState::new("TEST");
        init_sound_state(&mut gs, 5);
        assert_eq!(gs.sound.sound_targets.len(), 5);
        assert_eq!(gs.sound.sound_traversed.len(), 5);
        assert_eq!(gs.sound.sound_gen, 0);
        for i in 0..5 {
            assert!(gs.sound.sound_targets[i].is_none());
            assert_eq!(gs.sound.sound_traversed[i], 0);
        }
    }

    #[test]
    fn p_noise_alert_sets_sound_target_in_emitter_sector() {
        // 2 sectors connected by a normal two-sided linedef.
        let level = make_test_level(2, &[(0, 1, 0)]);
        let mut gs = make_game_state_with_sound(2);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);

        // Sector 0 (emitter's sector) should have the sound target.
        assert_eq!(get_sound_target(&gs, 0), Some(player));
    }

    #[test]
    fn sound_propagates_through_two_sided_linedef() {
        // 2 sectors connected by a normal two-sided linedef.
        let level = make_test_level(2, &[(0, 1, 0)]);
        let mut gs = make_game_state_with_sound(2);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);

        // Both sectors should have the sound target.
        assert_eq!(get_sound_target(&gs, 0), Some(player));
        assert_eq!(get_sound_target(&gs, 1), Some(player));
    }

    #[test]
    fn sound_does_not_propagate_through_one_sided_linedef() {
        // 2 sectors with NO connecting linedefs (isolated sectors).
        let level = make_test_level(2, &[]);
        let mut gs = make_game_state_with_sound(2);
        let player = gs.player.handle;

        // Add a one-sided linedef (not two-sided) to the level -- but
        // our test level has no linedefs at all, so sector 1 is isolated.
        p_noise_alert(&mut gs, &level, player, player);

        assert_eq!(get_sound_target(&gs, 0), Some(player));
        assert_eq!(
            get_sound_target(&gs, 1),
            None,
            "isolated sector must not receive sound"
        );
    }

    #[test]
    fn sound_passes_through_one_soundblock_linedef() {
        // 2 sectors connected by a ML_SOUNDBLOCK linedef.
        let level = make_test_level(2, &[(0, 1, ML_SOUNDBLOCK)]);
        let mut gs = make_game_state_with_sound(2);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);

        // Sound should still reach sector 1 (one soundblock is fine).
        assert_eq!(get_sound_target(&gs, 0), Some(player));
        assert_eq!(get_sound_target(&gs, 1), Some(player));
    }

    #[test]
    fn sound_blocked_by_two_consecutive_soundblock_linedefs() {
        // 3 sectors: 0 --[SOUNDBLOCK]--> 1 --[SOUNDBLOCK]--> 2
        let level = make_test_level(3, &[(0, 1, ML_SOUNDBLOCK), (1, 2, ML_SOUNDBLOCK)]);
        let mut gs = make_game_state_with_sound(3);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);

        assert_eq!(get_sound_target(&gs, 0), Some(player));
        assert_eq!(get_sound_target(&gs, 1), Some(player));
        assert_eq!(
            get_sound_target(&gs, 2),
            None,
            "sector behind two consecutive soundblock lines must not receive sound"
        );
    }

    #[test]
    fn sound_blocked_by_three_consecutive_soundblock_linedefs() {
        // 4 sectors: 0 --[SB]--> 1 --[SB]--> 2 --[SB]--> 3
        let level = make_test_level(
            4,
            &[
                (0, 1, ML_SOUNDBLOCK),
                (1, 2, ML_SOUNDBLOCK),
                (2, 3, ML_SOUNDBLOCK),
            ],
        );
        let mut gs = make_game_state_with_sound(4);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);

        assert_eq!(get_sound_target(&gs, 0), Some(player));
        assert_eq!(get_sound_target(&gs, 1), Some(player));
        assert_eq!(
            get_sound_target(&gs, 2),
            None,
            "second consecutive soundblock must already stop the flood fill"
        );
        assert_eq!(
            get_sound_target(&gs, 3),
            None,
            "sector behind 3 soundblock lines must not receive sound"
        );
    }

    #[test]
    fn get_sound_target_returns_none_initially() {
        let mut gs = GameState::new("TEST");
        init_sound_state(&mut gs, 3);

        assert_eq!(get_sound_target(&gs, 0), None);
        assert_eq!(get_sound_target(&gs, 1), None);
        assert_eq!(get_sound_target(&gs, 2), None);
    }

    #[test]
    fn get_sound_target_returns_some_after_noise() {
        let level = make_test_level(1, &[]);
        let mut gs = make_game_state_with_sound(1);
        let player = gs.player.handle;

        assert_eq!(get_sound_target(&gs, 0), None);
        p_noise_alert(&mut gs, &level, player, player);
        assert_eq!(get_sound_target(&gs, 0), Some(player));
    }

    #[test]
    fn clear_sound_targets_resets_all_to_none() {
        let level = make_test_level(3, &[(0, 1, 0), (1, 2, 0)]);
        let mut gs = make_game_state_with_sound(3);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);
        // All sectors should have targets.
        assert!(get_sound_target(&gs, 0).is_some());
        assert!(get_sound_target(&gs, 1).is_some());
        assert!(get_sound_target(&gs, 2).is_some());

        clear_sound_targets(&mut gs);

        assert_eq!(get_sound_target(&gs, 0), None);
        assert_eq!(get_sound_target(&gs, 1), None);
        assert_eq!(get_sound_target(&gs, 2), None);
    }

    #[test]
    fn sound_generation_counter_increments() {
        let level = make_test_level(1, &[]);
        let mut gs = make_game_state_with_sound(1);
        let player = gs.player.handle;

        assert_eq!(gs.sound.sound_gen, 0);
        p_noise_alert(&mut gs, &level, player, player);
        assert_eq!(gs.sound.sound_gen, 1);
        p_noise_alert(&mut gs, &level, player, player);
        assert_eq!(gs.sound.sound_gen, 2);
        p_noise_alert(&mut gs, &level, player, player);
        assert_eq!(gs.sound.sound_gen, 3);
    }

    #[test]
    fn flood_fill_does_not_revisit_sectors() {
        // Create a diamond topology: 0 <-> 1, 0 <-> 2, 1 <-> 3, 2 <-> 3
        // Sound should reach sector 3 exactly once despite two paths.
        let level = make_test_level(4, &[(0, 1, 0), (0, 2, 0), (1, 3, 0), (2, 3, 0)]);
        let mut gs = make_game_state_with_sound(4);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);

        // All sectors should be reached exactly once.
        for i in 0..4 {
            assert_eq!(
                get_sound_target(&gs, i),
                Some(player),
                "sector {} must have sound target",
                i
            );
        }
        // Generation counter should show only one alert.
        assert_eq!(gs.sound.sound_gen, 1);
    }

    #[test]
    fn adjacent_sectors_returns_correct_neighbors() {
        // 3 sectors: 0 <-> 1, 1 <-> 2
        let level = make_test_level(3, &[(0, 1, 0), (1, 2, 0)]);

        let adj_0 = adjacent_sectors(&level, 0);
        assert_eq!(adj_0, vec![1]);

        let adj_1 = adjacent_sectors(&level, 1);
        assert!(adj_1.contains(&0));
        assert!(adj_1.contains(&2));
        assert_eq!(adj_1.len(), 2);

        let adj_2 = adjacent_sectors(&level, 2);
        assert_eq!(adj_2, vec![1]);
    }

    #[test]
    fn adjacent_sectors_returns_empty_for_isolated_sector() {
        // 3 sectors, only 0 <-> 1 connected. Sector 2 is isolated.
        let level = make_test_level(3, &[(0, 1, 0)]);

        let adj_2 = adjacent_sectors(&level, 2);
        assert!(adj_2.is_empty(), "isolated sector must have no adjacencies");
    }

    #[test]
    fn monster_should_wake_true_for_non_ambush_with_sound_target() {
        let level = make_test_level(1, &[]);
        let mut gs = make_game_state_with_sound(1);
        let player = gs.player.handle;

        // Spawn a non-ambush monster in sector 0 (subsector 0).
        let monster = spawn_monster(&mut gs, MobjKind::Trooper, 10, 0, 0, 0);

        // No sound yet.
        assert!(
            !monster_should_wake(&gs, &level, monster),
            "monster should not wake without sound"
        );

        // Fire noise alert.
        p_noise_alert(&mut gs, &level, player, player);

        assert!(
            monster_should_wake(&gs, &level, monster),
            "non-ambush monster should wake from sound"
        );
    }

    #[test]
    fn monster_should_wake_false_when_no_sound_target() {
        let level = make_test_level(1, &[]);
        let mut gs = make_game_state_with_sound(1);

        let monster = spawn_monster(&mut gs, MobjKind::Trooper, 10, 0, 0, 0);

        assert!(
            !monster_should_wake(&gs, &level, monster),
            "monster should not wake without any sound target"
        );
    }

    #[test]
    fn monster_should_wake_ambush_requires_los() {
        let level = make_test_level(1, &[]);
        let mut gs = make_game_state_with_sound(1);
        let player = gs.player.handle;

        // Spawn an AMBUSH (deaf) monster close enough for LOS (within 4096 range).
        let monster = spawn_monster(&mut gs, MobjKind::Trooper, 100, 0, 0, flags::MF_AMBUSH);

        // Fire noise alert.
        p_noise_alert(&mut gs, &level, player, player);

        // Monster has MF_AMBUSH but player is close (within Manhattan 4096).
        // p_check_sight should return true (same sector, close distance).
        assert!(
            monster_should_wake(&gs, &level, monster),
            "ambush monster with LOS should wake from sound"
        );
    }

    #[test]
    fn monster_should_wake_ambush_no_los_stays_asleep() {
        // Create a level where sector 0 and sector 1 are connected but the
        // reject table blocks visibility between them.
        let n_sectors = 2;
        let sectors: Vec<Sector> = (0..n_sectors)
            .map(|_| Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            })
            .collect();

        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: [0; 8],
                lower_texture: [0; 8],
                middle_texture: [0; 8],
                sector: 1,
            },
        ];

        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: FLAG_TWO_SIDED,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];

        let vertexes = vec![Vertex { x: 0, y: 0 }, Vertex { x: 64, y: 0 }];

        // Two segs in two subsectors, each in a different sector.
        // Seg 0 → linedef 0, direction 0 → right sidedef → sector 0.
        // Seg 1 → linedef 0, direction 1 → left sidedef → sector 1.
        let segs = vec![
            Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            },
            Seg {
                from_vertex: 1,
                to_vertex: 0,
                angle: 0,
                linedef: 0,
                direction: 1,
                offset: 0,
            },
        ];

        let ssectors = vec![
            Ssector {
                seg_count: 1,
                first_seg: 0,
            },
            Ssector {
                seg_count: 1,
                first_seg: 1,
            },
        ];

        // Reject table: block visibility between sector 0 and 1.
        // Bits: s0-s0=0(visible), s0-s1=1(blocked), s1-s0=1(blocked), s1-s1=0(visible)
        // bit 0: (0,0) = 0
        // bit 1: (0,1) = 1  (blocked)
        // bit 2: (1,0) = 1  (blocked)
        // bit 3: (1,1) = 0
        // = 0b0000_0110 = 0x06
        let reject_data = vec![0x06u8];
        let reject = Reject::parse_lump(&reject_data, 2).unwrap();

        let mut bm_data = vec![0u8; 14];
        bm_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        bm_data[2..4].copy_from_slice(&0i16.to_le_bytes());
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).unwrap();

        // Need one node to split into two subsectors.
        let nodes = vec![doom_map::Node {
            x: 32,
            y: 0,
            dx: 0,
            dy: 64,
            right_bbox: doom_map::NodeBBox {
                ymax: 64,
                ymin: 0,
                xmin: 0,
                xmax: 32,
            },
            left_bbox: doom_map::NodeBBox {
                ymax: 64,
                ymin: 0,
                xmin: 32,
                xmax: 64,
            },
            right_child: 0x8000, // subsector 0
            left_child: 0x8001,  // subsector 1
        }];

        let level = Level {
            name: "TEST".to_string(),
            things: Vec::new(),
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes,
            sectors,
            reject,
            blockmap,
        };

        let mut gs = GameState::new("TEST");
        init_sound_state(&mut gs, 2);

        // Player in subsector 0 (sector 0).
        let mut player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        player_mo.health = 100;
        player_mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        player_mo.subsector = 0;
        let player_handle = gs.mobjslab.alloc(player_mo);
        gs.player.handle = player_handle;

        // Monster in subsector 1 (sector 1), with MF_AMBUSH.
        // Place it far away (> 4096 Manhattan) so p_check_sight fails.
        let monster = spawn_monster(&mut gs, MobjKind::Trooper, 5000, 5000, 1, flags::MF_AMBUSH);

        // Fire noise alert — sound propagates to sector 1.
        p_noise_alert(&mut gs, &level, player_handle, player_handle);

        assert_eq!(
            get_sound_target(&gs, 1),
            Some(player_handle),
            "sound should reach sector 1"
        );

        // Monster has MF_AMBUSH and reject table blocks LOS + far distance.
        assert!(
            !monster_should_wake(&gs, &level, monster),
            "ambush monster without LOS should NOT wake"
        );
    }

    #[test]
    fn multiple_noise_alerts_update_targets() {
        let level = make_test_level(2, &[(0, 1, 0)]);
        let mut gs = make_game_state_with_sound(2);
        let player = gs.player.handle;

        // First alert from player.
        p_noise_alert(&mut gs, &level, player, player);
        assert_eq!(get_sound_target(&gs, 0), Some(player));
        assert_eq!(get_sound_target(&gs, 1), Some(player));

        // Spawn a second actor and alert with it as the target.
        let other = spawn_monster(&mut gs, MobjKind::Trooper, 10, 0, 0, 0);
        p_noise_alert(&mut gs, &level, other, player);

        // Now the sound target should be the second actor.
        assert_eq!(get_sound_target(&gs, 0), Some(other));
        assert_eq!(get_sound_target(&gs, 1), Some(other));
    }

    #[test]
    fn sound_propagates_across_multiple_sectors() {
        // Chain: 0 <-> 1 <-> 2 <-> 3 <-> 4
        let level = make_test_level(5, &[(0, 1, 0), (1, 2, 0), (2, 3, 0), (3, 4, 0)]);
        let mut gs = make_game_state_with_sound(5);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);

        for i in 0..5 {
            assert_eq!(
                get_sound_target(&gs, i),
                Some(player),
                "sector {} must receive sound through chain",
                i
            );
        }
    }

    #[test]
    fn ml_soundblock_flag_detection() {
        // Verify the ML_SOUNDBLOCK constant works with linedef flags.
        let ld = Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: FLAG_TWO_SIDED | ML_SOUNDBLOCK,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        };
        assert!(ld.flags & ML_SOUNDBLOCK != 0);

        let ld_no_sb = Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: FLAG_TWO_SIDED,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        };
        assert!(ld_no_sb.flags & ML_SOUNDBLOCK == 0);
    }

    #[test]
    fn game_state_clone_includes_sound_state() {
        let level = make_test_level(3, &[(0, 1, 0), (1, 2, 0)]);
        let mut gs = make_game_state_with_sound(3);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);

        let gs2 = gs.clone();

        // Cloned state should have identical sound data.
        assert_eq!(gs2.sound.sound_targets.len(), 3);
        assert_eq!(gs2.sound.sound_traversed.len(), 3);
        assert_eq!(gs2.sound.sound_gen, gs.sound.sound_gen);
        for i in 0..3 {
            assert_eq!(
                get_sound_target(&gs2, i),
                Some(player),
                "cloned state sector {} must have sound target",
                i
            );
        }

        // Modifications to original don't affect clone.
        clear_sound_targets(&mut gs);
        assert_eq!(get_sound_target(&gs, 0), None);
        assert_eq!(
            get_sound_target(&gs2, 0),
            Some(player),
            "clone must be independent"
        );
    }

    #[test]
    fn sound_target_cleared_after_clear() {
        let level = make_test_level(2, &[(0, 1, 0)]);
        let mut gs = make_game_state_with_sound(2);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);
        assert!(get_sound_target(&gs, 0).is_some());
        assert!(get_sound_target(&gs, 1).is_some());

        clear_sound_targets(&mut gs);

        assert_eq!(get_sound_target(&gs, 0), None);
        assert_eq!(get_sound_target(&gs, 1), None);
    }

    #[test]
    fn p_noise_alert_with_stale_emitter_handle_is_graceful() {
        let level = make_test_level(1, &[]);
        let mut gs = make_game_state_with_sound(1);
        let player = gs.player.handle;

        // Create and immediately free an actor to get a stale handle.
        let mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let stale = gs.mobjslab.alloc(mo);
        gs.mobjslab.free(stale);

        // Should not panic, should be a no-op.
        p_noise_alert(&mut gs, &level, player, stale);

        // Sound gen should still increment (we enter the function).
        // Actually, with the stale handle, we bail before incrementing.
        // That's fine -- the function gracefully returns early.
        assert_eq!(
            get_sound_target(&gs, 0),
            None,
            "stale emitter should not produce sound"
        );
    }

    #[test]
    fn get_sound_target_out_of_range_returns_none() {
        let mut gs = GameState::new("TEST");
        init_sound_state(&mut gs, 2);
        assert_eq!(get_sound_target(&gs, 999), None);
    }

    #[test]
    fn adjacent_sectors_with_duplicate_connections() {
        // Two linedefs both connecting sector 0 to sector 1.
        let level = make_test_level(2, &[(0, 1, 0), (0, 1, 0)]);

        let adj = adjacent_sectors(&level, 0);
        // Should be deduplicated: only one entry for sector 1.
        assert_eq!(adj, vec![1]);
    }

    #[test]
    fn sound_with_mixed_soundblock_and_normal_linedefs() {
        // 0 --[normal]--> 1 --[SOUNDBLOCK]--> 2 --[normal]--> 3 --[SOUNDBLOCK]--> 4
        let level = make_test_level(
            5,
            &[
                (0, 1, 0),
                (1, 2, ML_SOUNDBLOCK),
                (2, 3, 0),
                (3, 4, ML_SOUNDBLOCK),
            ],
        );
        let mut gs = make_game_state_with_sound(5);
        let player = gs.player.handle;

        p_noise_alert(&mut gs, &level, player, player);

        // Doom-style sound can cross one soundblock, even if normal sectors sit
        // between blockers. The second soundblock still stops propagation.
        for i in 0..4 {
            assert_eq!(
                get_sound_target(&gs, i),
                Some(player),
                "sector {} should be reached with mixed topology",
                i
            );
        }
        assert_eq!(
            get_sound_target(&gs, 4),
            None,
            "second soundblock in a mixed path must still stop propagation"
        );
    }
}
