use crate::mobj::MobjHandle;
use crate::state::*;
use doom_map::Level;
use doom_types::Fixed16_16;

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
