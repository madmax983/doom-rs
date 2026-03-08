//! The `Level` struct: a fully parsed and validated Doom map.
//!
//! Loaded from a `WadFile` by name (e.g. "E1M1") using the 10-lump
//! standard format.  UDMF support lives in `udmf.rs`.
//!
//! # Validation performed at load time
//! All Verus-targeted invariants are runtime-checked here:
//! - BSP leaf count (`N_SSECTORS == N_NODES + 1`)
//! - All child pointers in bounds
//! - All node bboxes non-degenerate
//! - All ssector seg ranges in bounds
//! - All linedef vertex refs in bounds
//! - Two-sided flag ↔ both sidedefs valid
//! - All sidedef sector refs in bounds
//! - Reject size matches N_SECTORS²
//!
//! The right_sidedef-always-valid invariant is logged but not hard-errored
//! because a handful of commercial PWADs violate it (vanilla does allow it
//! for zero-length linedefs used as level markers).

use crate::bsp::{BspError, BspTree};
use crate::lumps::{
    Blockmap, Linedef, LumpParseError, Node, Reject, SIDEDEF_NONE, Sector, Seg, Sidedef, Ssector,
    Thing, Vertex,
};
use doom_wad::WadFile;
use thiserror::Error;

/// Errors from level loading.
#[derive(Debug, Error)]
pub enum LevelError {
    /// The map marker lump was not found.
    #[error("map '{0}' not found in WAD")]
    NotFound(String),

    /// Required lump is missing or in the wrong position.
    #[error("map '{map}': required lump {lump} missing")]
    MissingLump { map: String, lump: &'static str },

    /// A lump failed to parse.
    #[error("map '{map}': lump parse error: {source}")]
    ParseError { map: String, source: LumpParseError },

    /// BSP structural validation failed.
    #[error("map '{map}': BSP validation failed: {source}")]
    BspInvalid { map: String, source: BspError },

    /// A linedef references a vertex index that's out of range.
    #[error("map '{map}': linedef {idx} references vertex {v} >= N_VERTEXES ({n})")]
    LindefVertexOutOfBounds {
        map: String,
        idx: usize,
        v: usize,
        n: usize,
    },

    /// A linedef has the two-sided flag but is missing a left sidedef.
    #[error("map '{map}': linedef {idx} is two-sided but left_sidedef is 0xFFFF")]
    TwoSidedMissingLeft { map: String, idx: usize },

    /// A sidedef references a sector that's out of range.
    #[error("map '{map}': sidedef {idx} references sector {s} >= N_SECTORS ({n})")]
    SidedefSectorOutOfBounds {
        map: String,
        idx: usize,
        s: usize,
        n: usize,
    },
}

/// A fully parsed and validated Doom map.
pub struct Level {
    /// Map name (e.g. "E1M1").
    pub name: String,
    pub things: Vec<Thing>,
    pub linedefs: Vec<Linedef>,
    pub sidedefs: Vec<Sidedef>,
    pub vertexes: Vec<Vertex>,
    pub segs: Vec<Seg>,
    pub ssectors: Vec<Ssector>,
    pub nodes: Vec<Node>,
    pub sectors: Vec<Sector>,
    pub reject: Reject,
    pub blockmap: Blockmap,
}

impl Level {
    /// Load and validate a level from a WAD file.
    ///
    /// # Errors
    /// Returns `LevelError` for any structural or bounds violation.
    pub fn from_wad(wad: &WadFile, map_name: &str) -> Result<Self, LevelError> {
        let group = wad
            .map_lump_group(map_name)
            .ok_or_else(|| LevelError::NotFound(map_name.to_owned()))?;

        let name = map_name.to_uppercase();

        macro_rules! lump_data {
            ($i:expr, $lump_name:literal) => {
                wad.lump_data(group.lumps[$i])
            };
        }

        macro_rules! parse {
            ($i:expr, $lump_name:literal, $parse_fn:expr) => {
                $parse_fn(lump_data!($i, $lump_name)).map_err(|e| LevelError::ParseError {
                    map: name.clone(),
                    source: e,
                })?
            };
        }

        // Parse all 10 lumps in spec order.
        // REQUIRED_MAP_LUMPS: THINGS LINEDEFS SIDEDEFS VERTEXES SEGS SSECTORS NODES SECTORS REJECT BLOCKMAP
        let things = parse!(0, "THINGS", Thing::parse_lump);
        let linedefs = parse!(1, "LINEDEFS", Linedef::parse_lump);
        let sidedefs = parse!(2, "SIDEDEFS", Sidedef::parse_lump);
        let vertexes = parse!(3, "VERTEXES", Vertex::parse_lump);
        let segs = parse!(4, "SEGS", Seg::parse_lump);
        let ssectors = parse!(5, "SSECTORS", Ssector::parse_lump);
        let nodes = parse!(6, "NODES", Node::parse_lump);
        let sectors = parse!(7, "SECTORS", Sector::parse_lump);

        let reject = Reject::parse_lump(lump_data!(8, "REJECT"), sectors.len()).map_err(|e| {
            LevelError::ParseError {
                map: name.clone(),
                source: e,
            }
        })?;

        let blockmap = Blockmap::parse_lump(lump_data!(9, "BLOCKMAP")).map_err(|e| {
            LevelError::ParseError {
                map: name.clone(),
                source: e,
            }
        })?;

        // -- Structural validation ------------------------------------------

        // BSP invariants (crown jewel: N_SSECTORS == N_NODES + 1)
        BspTree::validate(&nodes, &ssectors, segs.len()).map_err(|e| LevelError::BspInvalid {
            map: name.clone(),
            source: e,
        })?;

        // Linedef vertex refs in bounds.
        let n_verts = vertexes.len();
        for (i, ld) in linedefs.iter().enumerate() {
            for v in [ld.from_vertex as usize, ld.to_vertex as usize] {
                if v >= n_verts {
                    return Err(LevelError::LindefVertexOutOfBounds {
                        map: name,
                        idx: i,
                        v,
                        n: n_verts,
                    });
                }
            }
        }

        // Two-sided ↔ both sidedefs valid.
        for (i, ld) in linedefs.iter().enumerate() {
            if ld.is_two_sided() && ld.left_sidedef == SIDEDEF_NONE {
                return Err(LevelError::TwoSidedMissingLeft { map: name, idx: i });
            }
        }

        // Sidedef sector refs in bounds.
        let n_sectors = sectors.len();
        for (i, sd) in sidedefs.iter().enumerate() {
            if sd.sector as usize >= n_sectors {
                return Err(LevelError::SidedefSectorOutOfBounds {
                    map: name,
                    idx: i,
                    s: sd.sector as usize,
                    n: n_sectors,
                });
            }
        }

        Ok(Self {
            name: map_name.to_uppercase(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes,
            sectors,
            reject,
            blockmap,
        })
    }

    /// Convenience: access the validated BSP tree.
    pub fn bsp(&self) -> BspTree<'_> {
        // Already validated at load time, so this cannot fail.
        BspTree::validate(&self.nodes, &self.ssectors, self.segs.len())
            .expect("BSP invariant violated post-load — this is a bug")
    }

    /// Find the sector index containing world point `(x, y)` via BSP traversal.
    ///
    /// Returns `None` if the level has no BSP nodes, or if any index is
    /// out of bounds.
    #[must_use]
    pub fn sector_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let bsp = BspTree::validate(
            &self.nodes,
            &self.ssectors,
            self.segs.len(),
        )
        .ok()?;
        let ssector = bsp.point_in_subsector(x, y)?;
        let seg = self.segs.get(ssector.first_seg as usize)?;
        let linedef = self.linedefs.get(seg.linedef as usize)?;
        let sidedef_idx = if seg.direction == 0 {
            linedef.right_sidedef
        } else {
            linedef.left_sidedef
        };
        if sidedef_idx == 0xFFFF {
            return None;
        }
        let sidedef = self.sidedefs.get(sidedef_idx as usize)?;
        Some(sidedef.sector as usize)
    }

    /// Return the floor height (in map units) at world point `(x, y)`.
    ///
    /// Uses BSP traversal to find the subsector.  Returns `None` if the
    /// level geometry is incomplete.
    #[must_use]
    pub fn floor_at(&self, x: i32, y: i32) -> Option<i16> {
        let si = self.sector_index_at(x, y)?;
        self.sectors.get(si).map(|s| s.floor_height)
    }

    /// Print a one-line geometry summary (used by the Phase 3 CLI gate).
    pub fn print_stats(&self) {
        let bsp = self.bsp();
        println!(
            "Level {}: {} things, {} linedefs, {} sidedefs, {} vertexes, \
             {} sectors, {} segs, {} ssectors, {} nodes (BSP depth {})",
            self.name,
            self.things.len(),
            self.linedefs.len(),
            self.sidedefs.len(),
            self.vertexes.len(),
            self.sectors.len(),
            self.segs.len(),
            self.ssectors.len(),
            self.nodes.len(),
            bsp.max_depth(),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal in-memory WAD with one map.
    ///
    /// The map has:
    /// - 1 sector (floor=0, ceil=128, light=192)
    /// - 4 vertices forming a square 64×64 units
    /// - 4 linedefs (one-sided square)
    /// - 4 sidedefs (all facing into the sector)
    /// - 1 thing (player 1 start)
    /// - 1 ssector covering 1 seg  ← ensures N_SSECTORS(1) == N_NODES(0) + 1
    /// - 0 nodes (trivial BSP: single subsector)
    /// - 1 seg
    fn build_minimal_wad_bytes() -> Vec<u8> {
        use doom_wad::{REQUIRED_MAP_LUMPS, WadKind};

        // We'll build the WAD manually.
        let marker_name = b"E1M1\0\0\0\0";

        // ---- sector ----
        let mut sector_data = vec![0u8; 26];
        sector_data[0..2].copy_from_slice(&0i16.to_le_bytes()); // floor_height
        sector_data[2..4].copy_from_slice(&128i16.to_le_bytes()); // ceil_height
        sector_data[4..12].copy_from_slice(b"FLAT1\0\0\0"); // floor_flat
        sector_data[12..20].copy_from_slice(b"FLAT2\0\0\0"); // ceil_flat
        sector_data[20..22].copy_from_slice(&192i16.to_le_bytes()); // light
        sector_data[22..24].copy_from_slice(&0u16.to_le_bytes()); // special
        sector_data[24..26].copy_from_slice(&0u16.to_le_bytes()); // tag

        // ---- vertices: (0,0), (64,0), (64,64), (0,64) ----
        let mut vert_data = vec![0u8; 4 * 4];
        let verts = [(0i16, 0i16), (64, 0), (64, 64), (0, 64)];
        for (i, (x, y)) in verts.iter().enumerate() {
            vert_data[i * 4..i * 4 + 2].copy_from_slice(&x.to_le_bytes());
            vert_data[i * 4 + 2..i * 4 + 4].copy_from_slice(&y.to_le_bytes());
        }

        // ---- sidedefs: all pointing to sector 0 ----
        let mut sd_data = vec![0u8; 4 * 30];
        for i in 0..4 {
            sd_data[i * 30 + 20..i * 30 + 28].copy_from_slice(b"WALL1\0\0\0"); // middle
            sd_data[i * 30 + 28..i * 30 + 30].copy_from_slice(&0u16.to_le_bytes()); // sector=0
        }

        // ---- linedefs: 4 one-sided walls forming a square ----
        // (0→1), (1→2), (2→3), (3→0), each with right_sidedef=0..3, left=0xFFFF
        let mut ld_data = vec![0u8; 4 * 14];
        let edges = [(0u16, 1u16), (1, 2), (2, 3), (3, 0)];
        for (i, (from, to)) in edges.iter().enumerate() {
            let b = &mut ld_data[i * 14..i * 14 + 14];
            b[0..2].copy_from_slice(&from.to_le_bytes());
            b[2..4].copy_from_slice(&to.to_le_bytes());
            b[4..6].copy_from_slice(&0u16.to_le_bytes()); // flags
            b[6..8].copy_from_slice(&0u16.to_le_bytes()); // special
            b[8..10].copy_from_slice(&0u16.to_le_bytes()); // tag
            b[10..12].copy_from_slice(&(i as u16).to_le_bytes()); // right_sidedef
            b[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // left=none
        }

        // ---- seg: vertex 0 → 1, linedef 0 ----
        let mut seg_data = vec![0u8; 12];
        seg_data[0..2].copy_from_slice(&0u16.to_le_bytes()); // from_vertex
        seg_data[2..4].copy_from_slice(&1u16.to_le_bytes()); // to_vertex
        seg_data[4..6].copy_from_slice(&0u16.to_le_bytes()); // angle
        seg_data[6..8].copy_from_slice(&0u16.to_le_bytes()); // linedef
        seg_data[8..10].copy_from_slice(&0u16.to_le_bytes()); // direction
        seg_data[10..12].copy_from_slice(&0u16.to_le_bytes()); // offset

        // ---- ssector: 1 seg starting at seg 0 ----
        let mut ss_data = vec![0u8; 4];
        ss_data[0..2].copy_from_slice(&1u16.to_le_bytes()); // seg_count
        ss_data[2..4].copy_from_slice(&0u16.to_le_bytes()); // first_seg

        // ---- nodes: empty (trivial BSP) ----
        let node_data: Vec<u8> = vec![];

        // ---- things: player 1 start (kind=1, angle=0, flags=7) ----
        let mut thing_data = vec![0u8; 10];
        thing_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // kind=1 player start
        thing_data[8..10].copy_from_slice(&7u16.to_le_bytes()); // flags (all skills)

        // ---- reject: ceil(1*1 / 8) = 1 byte, all zeros ----
        let reject_data = vec![0u8; 1];

        // ---- blockmap: minimal 1×1 grid ----
        let mut bm_data = vec![0u8; 8 + 2 + 4]; // header + 1 offset + one empty block
        bm_data[0..2].copy_from_slice(&0i16.to_le_bytes()); // x_origin
        bm_data[2..4].copy_from_slice(&0i16.to_le_bytes()); // y_origin
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes()); // offset = 5 words from start
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes()); // 0x0000 sentinel
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // 0xFFFF terminator

        // ---- assemble the WAD bytes ----
        let lump_payloads: &[(&[u8; 8], &[u8])] = &[
            (marker_name, &[]), // E1M1 marker
            (b"THINGS\0\0", &thing_data),
            (b"LINEDEFS", &ld_data),
            (b"SIDEDEFS", &sd_data),
            (b"VERTEXES", &vert_data),
            (b"SEGS\0\0\0\0", &seg_data),
            (b"SSECTORS", &ss_data),
            (b"NODES\0\0\0", &node_data),
            (b"SECTORS\0", &sector_data),
            (b"REJECT\0\0", &reject_data),
            (b"BLOCKMAP", &bm_data),
        ];

        // Build WAD: header + lump data + directory
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lump_payloads.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes()); // placeholder

        let mut offsets: Vec<(usize, usize)> = Vec::new();
        for (_, payload) in lump_payloads {
            let pos = data.len();
            data.extend_from_slice(payload);
            offsets.push((pos, payload.len()));
        }

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());
        for (i, (name_bytes, _)) in lump_payloads.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());
            data.extend_from_slice(*name_bytes);
        }

        let _ = REQUIRED_MAP_LUMPS; // ensure import used
        let _ = WadKind::Iwad;
        data
    }

    #[test]
    fn load_minimal_level() {
        let wad_bytes = build_minimal_wad_bytes();
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("WAD parse failed");
        let level = Level::from_wad(&wad, "E1M1").expect("Level load failed");

        assert_eq!(level.name, "E1M1");
        assert_eq!(level.sectors.len(), 1);
        assert_eq!(level.vertexes.len(), 4);
        assert_eq!(level.linedefs.len(), 4);
        assert_eq!(level.sidedefs.len(), 4);
        assert_eq!(level.things.len(), 1);
        assert_eq!(level.segs.len(), 1);
        assert_eq!(level.ssectors.len(), 1);
        assert_eq!(level.nodes.len(), 0);
        // N_SSECTORS(1) == N_NODES(0) + 1  ✓
    }

    #[test]
    fn missing_map_errors() {
        let wad_bytes = build_minimal_wad_bytes();
        let wad = doom_wad::WadFile::parse(wad_bytes).unwrap();
        assert!(matches!(
            Level::from_wad(&wad, "E2M1"),
            Err(LevelError::NotFound(_))
        ));
    }

    #[test]
    fn level_stats_smoke() {
        let wad_bytes = build_minimal_wad_bytes();
        let wad = doom_wad::WadFile::parse(wad_bytes).unwrap();
        let level = Level::from_wad(&wad, "E1M1").unwrap();
        // Just ensure print_stats doesn't panic.
        level.print_stats();
    }
}
