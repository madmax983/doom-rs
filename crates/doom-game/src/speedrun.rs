//! Auto-generated speedrun split tracking using topological chokepoints.
//!
//! This module combines the map's structural chokepoints (from `MapAnalyzer`)
//! with the player's timeline to automatically generate speedrun splits
//! as the player progresses through critical bottlenecks of the map.

use doom_map::Level;
use doom_map::analyzer::MapAnalyzer;
use doom_map::graph::SectorGraph;
use std::collections::HashSet;

/// A recorded speedrun split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// The sector index that triggered the split.
    pub sector: usize,
    /// The game tic when the split occurred.
    pub tic: u32,
    /// A human-readable name for the split.
    pub name: String,
}

/// Tracks speedrun splits based on map chokepoints.
#[derive(Debug, Clone, Default)]
pub struct SpeedrunTracker {
    /// Sectors that act as map chokepoints.
    pub chokepoints: HashSet<usize>,
    /// Chokepoints the player has already visited.
    pub visited_chokepoints: HashSet<usize>,
    /// Recorded splits in chronological order.
    pub splits: Vec<Split>,
    /// Whether the tracker has been initialized for the current map.
    pub initialized: bool,
}

impl SpeedrunTracker {
    /// Create a new, uninitialized tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize the tracker by analyzing the map's topology if not already done.
    pub fn init_if_needed(&mut self, level: &Level) {
        if !self.initialized {
            let graph = SectorGraph::build(level);
            let analyzer = MapAnalyzer::new(&graph);
            let chokes = analyzer.chokepoints();
            self.chokepoints = chokes.into_iter().collect();
            self.initialized = true;
        }
    }

    /// Update the tracker with the player's current sector.
    ///
    /// If the sector is a known chokepoint and hasn't been visited yet,
    /// a new split is recorded and returned.
    pub fn update(&mut self, current_sector: usize, current_tic: u32) -> Option<&Split> {
        if self.chokepoints.contains(&current_sector)
            && !self.visited_chokepoints.contains(&current_sector)
        {
            self.visited_chokepoints.insert(current_sector);
            let split = Split {
                sector: current_sector,
                tic: current_tic,
                name: format!("Chokepoint Sector {}", current_sector),
            };
            self.splits.push(split);
            return self.splits.last();
        }
        None
    }

    /// Export the splits to a formatted string.
    #[must_use]
    pub fn export_splits(&self) -> String {
        let mut out = String::from("Speedrun Splits:\n");
        for (i, split) in self.splits.iter().enumerate() {
            let seconds = split.tic as f32 / 35.0;
            out.push_str(&format!("{}. {}: {:.2}s\n", i + 1, split.name, seconds));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_map::lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex};

    fn make_test_level() -> Level {
        let reject = Reject::parse_lump(&[0u8], 1).unwrap();
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).unwrap();

        Level {
            name: "TEST".to_owned(),
            things: vec![],
            linedefs: vec![
                // 0 <-> 1
                Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0x0004,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
                // 1 <-> 2
                Linedef {
                    from_vertex: 1,
                    to_vertex: 2,
                    flags: 0x0004,
                    special: 0,
                    tag: 0,
                    right_sidedef: 1,
                    left_sidedef: 2,
                },
            ],
            sidedefs: vec![
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"W\0\0\0\0\0\0\0",
                    lower_texture: *b"W\0\0\0\0\0\0\0",
                    middle_texture: *b"W\0\0\0\0\0\0\0",
                    sector: 0,
                },
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"W\0\0\0\0\0\0\0",
                    lower_texture: *b"W\0\0\0\0\0\0\0",
                    middle_texture: *b"W\0\0\0\0\0\0\0",
                    sector: 1,
                },
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"W\0\0\0\0\0\0\0",
                    lower_texture: *b"W\0\0\0\0\0\0\0",
                    middle_texture: *b"W\0\0\0\0\0\0\0",
                    sector: 2,
                },
            ],
            vertexes: vec![
                Vertex { x: 0, y: 0 },
                Vertex { x: 64, y: 0 },
                Vertex { x: 64, y: 64 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![
                Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"F\0\0\0\0\0\0\0",
                    ceil_flat: *b"F\0\0\0\0\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"F\0\0\0\0\0\0\0",
                    ceil_flat: *b"F\0\0\0\0\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"F\0\0\0\0\0\0\0",
                    ceil_flat: *b"F\0\0\0\0\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap,
        }
    }

    #[test]
    fn test_speedrun_tracker() {
        let level = make_test_level();
        let mut tracker = SpeedrunTracker::new();
        tracker.init_if_needed(&level);

        // Sector 1 is a chokepoint between 0 and 2
        assert!(tracker.chokepoints.contains(&1));

        assert_eq!(tracker.update(0, 10), None);

        let split = tracker.update(1, 35).unwrap().clone();
        assert_eq!(split.sector, 1);
        assert_eq!(split.tic, 35);

        assert_eq!(tracker.update(1, 40), None); // Already visited

        let out = tracker.export_splits();
        assert!(out.contains("Chokepoint Sector 1: 1.00s"));
    }
}
