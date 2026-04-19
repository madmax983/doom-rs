//! Sector visibility tracking for cogmind-mode fog-of-war.
//!
//! Uses the level's REJECT table to determine which sectors the player
//! can see from the current sector.  Sectors transition through three
//! states: `Unexplored` -> `Visible` -> `Remembered` (fog of war).

// ---------------------------------------------------------------------------
// SectorVisibility
// ---------------------------------------------------------------------------

/// Visibility state for a single sector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SectorVisibility {
    /// Never seen by the player.
    Unexplored,
    /// Previously seen; rendered dimmed.  The `u8` is the last-known light level.
    Remembered(u8),
    /// Currently visible to the player.  The `u8` is the live light level.
    Visible(u8),
}

// ---------------------------------------------------------------------------
// VisibilityMap
// ---------------------------------------------------------------------------

/// Per-sector visibility state for the entire level.
pub(crate) struct VisibilityMap {
    sectors: Vec<SectorVisibility>,
}

impl VisibilityMap {
    /// Create a new map with all sectors unexplored.
    #[must_use]
    pub(crate) fn new(n: usize) -> Self {
        Self {
            sectors: vec![SectorVisibility::Unexplored; n],
        }
    }

    /// Demote all `Visible` sectors to `Remembered`, preserving their light level.
    /// `Remembered` and `Unexplored` sectors are unchanged.
    pub(crate) fn demote_all(&mut self) {
        for vis in &mut self.sectors {
            if let SectorVisibility::Visible(light) = *vis {
                *vis = SectorVisibility::Remembered(light);
            }
        }
    }

    /// Mark a sector as visible with the given light level.
    pub(crate) fn mark_visible(&mut self, sector_idx: usize, light: u8) {
        if let Some(vis) = self.sectors.get_mut(sector_idx) {
            *vis = SectorVisibility::Visible(light);
        }
    }

    /// Per-tic visibility update.
    ///
    /// 1. Demotes all visible sectors to remembered.
    /// 2. Marks the player's sector as visible.
    /// 3. For every other sector, if `reject_visible_fn(player_sector, i)` is
    ///    true, marks it visible with the given light level.
    pub(crate) fn update<F, L>(
        &mut self,
        player_sector: usize,
        num_sectors: usize,
        reject_visible_fn: F,
        light_fn: L,
    ) where
        F: Fn(usize, usize) -> bool,
        L: Fn(usize) -> u8,
    {
        self.demote_all();

        // Mark player's own sector.
        if player_sector < num_sectors {
            self.mark_visible(player_sector, light_fn(player_sector));
        }

        // Mark sectors visible via reject table.
        for i in 0..num_sectors {
            if i == player_sector {
                continue;
            }
            if reject_visible_fn(player_sector, i) {
                self.mark_visible(i, light_fn(i));
            }
        }
    }

    /// Evaluates the current visibility status of a map sector in the Cogmind renderer. Returns `Unexplored` for
    /// out-of-bounds indices.
    #[must_use]
    pub(crate) fn get(&self, sector_idx: usize) -> SectorVisibility {
        self.sectors
            .get(sector_idx)
            .copied()
            .unwrap_or(SectorVisibility::Unexplored)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_map_all_unexplored() {
        let map = VisibilityMap::new(5);
        for i in 0..5 {
            assert_eq!(map.get(i), SectorVisibility::Unexplored);
        }
    }

    #[test]
    fn mark_visible_and_demote() {
        let mut map = VisibilityMap::new(3);
        map.mark_visible(1, 200);
        assert_eq!(map.get(1), SectorVisibility::Visible(200));

        map.demote_all();
        assert_eq!(map.get(1), SectorVisibility::Remembered(200));

        // Demoting again should leave it as Remembered.
        map.demote_all();
        assert_eq!(map.get(1), SectorVisibility::Remembered(200));
    }

    #[test]
    fn update_marks_player_sector_visible() {
        let mut map = VisibilityMap::new(4);
        let lights = [100, 150, 200, 50];

        // No reject visibility (closure always returns false).
        map.update(2, lights.len(), |_, _| false, |i| lights[i]);

        assert_eq!(map.get(2), SectorVisibility::Visible(200));
        assert_eq!(map.get(0), SectorVisibility::Unexplored);
        assert_eq!(map.get(1), SectorVisibility::Unexplored);
        assert_eq!(map.get(3), SectorVisibility::Unexplored);
    }

    #[test]
    fn update_with_reject_marks_visible_sectors() {
        let mut map = VisibilityMap::new(4);
        let lights = [100, 150, 200, 50];

        // Player in sector 0; sectors 1 and 3 are visible per reject table.
        map.update(
            0,
            lights.len(),
            |_player, other| other == 1 || other == 3,
            |i| lights[i],
        );

        assert_eq!(map.get(0), SectorVisibility::Visible(100));
        assert_eq!(map.get(1), SectorVisibility::Visible(150));
        assert_eq!(map.get(2), SectorVisibility::Unexplored);
        assert_eq!(map.get(3), SectorVisibility::Visible(50));
    }

    #[test]
    fn remembered_sectors_persist_across_updates() {
        let mut map = VisibilityMap::new(3);
        let lights = [100, 150, 200];

        // First update: all visible.
        map.update(0, lights.len(), |_, _| true, |i| lights[i]);
        assert_eq!(map.get(1), SectorVisibility::Visible(150));
        assert_eq!(map.get(2), SectorVisibility::Visible(200));

        // Second update: only sector 0 visible (player sector).
        map.update(0, lights.len(), |_, _| false, |i| lights[i]);
        assert_eq!(map.get(0), SectorVisibility::Visible(100));
        assert_eq!(map.get(1), SectorVisibility::Remembered(150));
        assert_eq!(map.get(2), SectorVisibility::Remembered(200));
    }

    #[test]
    fn oob_sector_returns_unexplored() {
        let map = VisibilityMap::new(2);
        assert_eq!(map.get(99), SectorVisibility::Unexplored);
    }
}
