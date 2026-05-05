//! The `GeometryAnalyzer` computes the exact physical properties of a sector's shape.
//! It uses the Shoelace Formula to calculate Area and Centroid (center of mass).
//!
//! # Uses
//! - **Spawn Density**: Determining if a 100-monster ambush is spawning in a 64x64 closet.
//! - **Pathfinding Costs**: Avoiding massive open courtyards where the player is exposed.
//! - **Procedural Generation**: Identifying cramped areas that need culling.

use crate::Level;
use crate::lumps::SIDEDEF_NONE;

/// The physical properties of a sector's layout in 2D space.
#[derive(Debug, Clone, PartialEq)]
pub struct SectorGeometry {
    /// The physical size of the sector in map units squared.
    pub area: f64,
    /// The X coordinate of the sector's center of mass.
    pub centroid_x: f64,
    /// The Y coordinate of the sector's center of mass.
    pub centroid_y: f64,
}

/// Analyzes map geometry for physical features (Area, Centroids).
pub struct GeometryAnalyzer<'a> {
    level: &'a Level,
}

impl<'a> GeometryAnalyzer<'a> {
    /// Creates a new `GeometryAnalyzer` bound to the given Level.
    #[must_use]
    pub fn new(level: &'a Level) -> Self {
        Self { level }
    }

    /// Computes the area and centroid for every sector in the map.
    ///
    /// This runs the Shoelace Formula over every directed linedef that forms the
    /// boundary of each sector. Inner holes (like pillars) correctly subtract
    /// from the area, and disconnected sectors are correctly summed.
    ///
    /// # Returns
    /// A vector where the index corresponds to the sector index in `Level::sectors`.
    #[must_use]
    pub fn sector_geometries(&self) -> Vec<SectorGeometry> {
        let n = self.level.sectors.len();
        let mut geometries = vec![
            SectorGeometry {
                area: 0.0,
                centroid_x: 0.0,
                centroid_y: 0.0,
            };
            n
        ];

        let mut areas = vec![0.0; n];
        let mut cx = vec![0.0; n];
        let mut cy = vec![0.0; n];

        for ld in &self.level.linedefs {
            let v1 = &self.level.vertexes[ld.from_vertex as usize];
            let v2 = &self.level.vertexes[ld.to_vertex as usize];
            let x1 = f64::from(v1.x);
            let y1 = f64::from(v1.y);
            let x2 = f64::from(v2.x);
            let y2 = f64::from(v2.y);

            let cross = x1 * y2 - x2 * y1;

            if ld.right_sidedef != SIDEDEF_NONE {
                let sd = &self.level.sidedefs[ld.right_sidedef as usize];
                let s_idx = sd.sector as usize;
                areas[s_idx] += cross;
                cx[s_idx] += (x1 + x2) * cross;
                cy[s_idx] += (y1 + y2) * cross;
            }

            if ld.left_sidedef != SIDEDEF_NONE {
                let sd = &self.level.sidedefs[ld.left_sidedef as usize];
                let s_idx = sd.sector as usize;
                // Left sidedef traces from to_vertex to from_vertex.
                let cross_left = x2 * y1 - x1 * y2;
                areas[s_idx] += cross_left;
                cx[s_idx] += (x1 + x2) * cross_left;
                cy[s_idx] += (y1 + y2) * cross_left;
            }
        }

        for i in 0..n {
            // Shoelace gives 2*Area.
            // In Doom, right-sided linedefs are clockwise, giving a negative area.
            let area2 = areas[i];
            let area = (area2 * 0.5).abs();

            let mut centroid_x = 0.0;
            let mut centroid_y = 0.0;

            if area > 0.001 {
                // Centroid formula: Cx = (1 / 6A) * sum((x_i + x_i+1) * (x_i y_i+1 - x_i+1 y_i))
                // Note: since area2 has the original sign of the shoelace cross products,
                // dividing by (3 * area2) naturally cancels the sign and preserves the correct coordinate.
                centroid_x = cx[i] / (3.0 * area2);
                centroid_y = cy[i] / (3.0 * area2);
            }

            geometries[i] = SectorGeometry {
                area,
                centroid_x,
                centroid_y,
            };
        }

        geometries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex};

    fn make_test_level() -> Level {
        let reject = Reject::parse_lump(&[0u8], 1).expect("value must exist in test");
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("value must exist in test");

        Level {
            name: "TEST".to_owned(),
            things: vec![],
            linedefs: vec![
                // 0,0 -> 10,0
                Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: SIDEDEF_NONE,
                },
                // 10,0 -> 10,10
                Linedef {
                    from_vertex: 1,
                    to_vertex: 2,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: SIDEDEF_NONE,
                },
                // 10,10 -> 0,10
                Linedef {
                    from_vertex: 2,
                    to_vertex: 3,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: SIDEDEF_NONE,
                },
                // 0,10 -> 0,0
                Linedef {
                    from_vertex: 3,
                    to_vertex: 0,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: SIDEDEF_NONE,
                },
            ],
            sidedefs: vec![Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"WALL1\0\0\0",
                lower_texture: *b"WALL2\0\0\0",
                middle_texture: *b"WALL3\0\0\0",
                sector: 0,
            }],
            vertexes: vec![
                Vertex { x: 0, y: 0 },
                Vertex { x: 10, y: 0 },
                Vertex { x: 10, y: 10 },
                Vertex { x: 0, y: 10 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![Sector {
                floor_height: 0,
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

    #[test]
    fn test_geometry_analyzer_square() {
        let level = make_test_level();
        let analyzer = GeometryAnalyzer::new(&level);
        let geom = analyzer.sector_geometries();

        assert_eq!(geom.len(), 1);

        let s0 = &geom[0];
        // 10x10 square has area 100
        assert!(
            (s0.area - 100.0).abs() < 0.001,
            "Expected area 100, got {}",
            s0.area
        );
        // Centroid should be at (5, 5)
        assert!(
            (s0.centroid_x - 5.0).abs() < 0.001,
            "Expected cx 5.0, got {}",
            s0.centroid_x
        );
        assert!(
            (s0.centroid_y - 5.0).abs() < 0.001,
            "Expected cy 5.0, got {}",
            s0.centroid_y
        );
    }
}
