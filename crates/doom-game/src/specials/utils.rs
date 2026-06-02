use doom_map::{Level, SIDEDEF_NONE};

// ---------------------------------------------------------------------------
// Adjacent sector height helpers
// ---------------------------------------------------------------------------

/// Returns an iterator over all sectors adjacent to `sector_index`.
/// Yields `(adjacent_sector_index, &Sector)`.
pub fn adjacent_sectors<'a>(
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
