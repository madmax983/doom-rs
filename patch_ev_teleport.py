with open("crates/doom-game/src/specials.rs", "r") as f:
    content = f.read()

old_teleport = """pub fn ev_teleport(gs: &mut GameState, level: &Level, tag: u16, mobj_handle: MobjHandle) -> bool {
    // Collect sector indices matching the tag.
    let tagged_sectors: Vec<usize> = level
        .sectors
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tag == tag)
        .map(|(i, _)| i)
        .collect();

    if tagged_sectors.is_empty() {
        return false;
    }

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
        let dest_floor = level.sectors[tagged_sectors[0]].floor_height;"""

new_teleport = """pub fn ev_teleport(gs: &mut GameState, level: &Level, tag: u16, mobj_handle: MobjHandle) -> bool {
    // Collect sector indices matching the tag.
    let first_tagged_sector = level
        .sectors
        .iter()
        .position(|s| s.tag == tag);

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
        let dest_floor = level.sectors[first_tagged_idx].floor_height;"""

content = content.replace(old_teleport, new_teleport)

with open("crates/doom-game/src/specials.rs", "w") as f:
    f.write(content)
