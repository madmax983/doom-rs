//! The `Level` struct: a fully parsed and validated Doom map.
//!
//! Loaded from a `WadFile` by name (e.g. "E1M1" or "MAP01") using either the
//! classic 10-lump binary format or Doom-namespace UDMF `TEXTMAP` plus
//! auxiliary BSP/collision lumps.
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
use crate::udmf::{UdmfError, UdmfMap};
use doom_wad::{MapLumpGroup, WadFile, WadStack};
use thiserror::Error;

/// Errors from level loading.
///
/// ## Examples
/// ```
/// use doom_map::LevelError;
///
/// let err = LevelError::NotFound("E1M1".to_string());
/// ```
#[derive(Debug, Error)]
pub enum LevelError {
    /// The map marker lump was not found.
    #[error("map '{0}' not found in WAD")]
    NotFound(String),

    /// Required lump is missing or in the wrong position.
    #[error("map '{map}': required lump {lump} missing")]
    MissingLump {
        /// The name of the map.
        map: String,
        /// The name of the missing lump.
        lump: &'static str,
    },

    /// A lump failed to parse.
    #[error("map '{map}': lump parse error: {source}")]
    ParseError {
        /// The name of the map.
        map: String,
        /// The underlying parse error.
        source: LumpParseError,
    },

    /// BSP structural validation failed.
    #[error("map '{map}': BSP validation failed: {source}")]
    BspInvalid {
        /// The name of the map.
        map: String,
        /// The underlying BSP validation error.
        source: BspError,
    },

    /// UDMF parsing or conversion failed.
    #[error("map '{map}': UDMF error: {source}")]
    Udmf {
        /// The name of the map.
        map: String,
        /// The underlying UDMF error.
        source: UdmfError,
    },

    /// The UDMF namespace requires features this engine does not support yet.
    #[error("map '{map}': unsupported UDMF namespace '{namespace}': {detail}")]
    UnsupportedUdmfNamespace {
        /// The name of the map.
        map: String,
        /// The unsupported namespace.
        namespace: String,
        /// Further details about the error.
        detail: String,
    },

    /// A linedef references a vertex index that's out of range.
    #[error("map '{map}': linedef {idx} references vertex {v} >= N_VERTEXES ({n})")]
    LindefVertexOutOfBounds {
        /// The name of the map.
        map: String,
        /// The index of the linedef.
        idx: usize,
        /// The invalid vertex index.
        v: usize,
        /// The total number of vertexes.
        n: usize,
    },

    /// A linedef has the two-sided flag but is missing a left sidedef.
    #[error("map '{map}': linedef {idx} is two-sided but left_sidedef is 0xFFFF")]
    TwoSidedMissingLeft {
        /// The name of the map.
        map: String,
        /// The index of the linedef.
        idx: usize,
    },

    /// A sidedef references a sector that's out of range.
    #[error("map '{map}': sidedef {idx} references sector {s} >= N_SECTORS ({n})")]
    SidedefSectorOutOfBounds {
        /// The name of the map.
        map: String,
        /// The index of the sidedef.
        idx: usize,
        /// The invalid sector index.
        s: usize,
        /// The total number of sectors.
        n: usize,
    },
}

/// A fully parsed and validated Doom map.
///
/// This structure holds the loaded geometry data from the binary map lumps,
/// including `THINGS`, `VERTEXES`, `LINEDEFS`, `SIDEDEFS`, `SECTORS`, `SEGS`,
/// `SSECTORS`, `NODES`, `REJECT`, and `BLOCKMAP`.
/// ## Examples
/// ```no_run
/// use doom_map::Level;
/// use doom_wad::WadFile;
///
/// let wad = WadFile::parse(std::fs::read("doom.wad").unwrap()).unwrap();
/// let level = Level::from_wad(&wad, "E1M1").unwrap();
/// assert_eq!(level.name, "E1M1");
/// ```
pub struct Level {
    /// Map name (e.g. "E1M1").
    pub name: String,
    /// The `THINGS` lump: all the monsters, weapons, keys, and decorations waiting to be spawned.
    pub things: Vec<Thing>,
    /// The `LINEDEFS` lump: the 2D line segments that construct walls and trigger actions.
    pub linedefs: Vec<Linedef>,
    /// The `SIDEDEFS` lump: textures and offsets for the front and back of each linedef.
    pub sidedefs: Vec<Sidedef>,
    /// The `VERTEXES` lump: the (x, y) points that connect all geometry.
    pub vertexes: Vec<Vertex>,
    /// The `SEGS` lump: rendered line segments that make up the walls of subsectors.
    pub segs: Vec<Seg>,
    /// The `SSECTORS` lump: convex polygons that form the leaves of the BSP tree, drawn back-to-front.
    pub ssectors: Vec<Ssector>,
    /// The `NODES` lump: the internal nodes of the BSP tree used to quickly determine drawing order.
    pub nodes: Vec<Node>,
    /// The `SECTORS` lump: distinct areas defined by floor/ceiling heights, flats, and lighting.
    pub sectors: Vec<Sector>,
    /// The `REJECT` lump: an optimized lookup table to skip line-of-sight checks between sectors.
    pub reject: Reject,
    /// The `BLOCKMAP` lump: a spatial grid for fast collision detection between actors and walls.
    pub blockmap: Blockmap,
}

impl Level {
    fn seg_front_sector_index(&self, seg_idx: usize) -> Option<usize> {
        let seg = self.segs.get(seg_idx)?;
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

    /// Resolve the sector that owns a subsector.
    ///
    /// Vanilla Doom treats subsectors as belonging to a single sector and
    /// resolves that sector from the first seg in the leaf.
    ///
    /// Returns `None` if the `subsector_idx` is out of bounds or if the
    /// internal references (seg -> linedef -> sidedef) are missing.
    #[must_use]
    pub fn subsector_sector_index(&self, subsector_idx: usize) -> Option<usize> {
        let ss = self.ssectors.get(subsector_idx)?;
        self.seg_front_sector_index(ss.first_seg as usize)
    }

    /// Load and validate a level from a WAD file.
    ///
    /// # Errors
    /// Returns `LevelError` for any structural or bounds violation.
    ///
    /// ## Examples
    /// ```no_run
    /// use doom_map::Level;
    /// use doom_wad::WadFile;
    ///
    /// let bytes = std::fs::read("doom1.wad").unwrap();
    /// let wad = WadFile::parse(bytes).unwrap();
    /// let level = Level::from_wad(&wad, "E1M1").unwrap();
    ///
    /// assert_eq!(level.name, "E1M1");
    /// ```
    pub fn from_wad(wad: &WadFile, map_name: &str) -> Result<Self, LevelError> {
        let group = wad
            .map_lump_group(map_name)
            .ok_or_else(|| LevelError::NotFound(map_name.to_owned()))?;

        Self::from_group(wad, group, map_name)
    }

    /// Load and validate a level from a stacked IWAD/PWAD view.
    ///
    /// # Errors
    /// Returns `LevelError` for any structural or bounds violation.
    ///
    /// ## Examples
    /// ```no_run
    /// use doom_map::Level;
    /// use doom_wad::{WadFile, WadStack};
    ///
    /// let iwad_bytes = std::fs::read("doom1.wad").unwrap();
    /// let pwad_bytes = std::fs::read("mymap.wad").unwrap();
    /// let mut stack = WadStack::new();
    /// stack.push_iwad(iwad_bytes).unwrap();
    /// stack.push_pwad(pwad_bytes).unwrap();
    ///
    /// let level = Level::from_wad_stack(&stack, "E1M1").unwrap();
    /// ```
    pub fn from_wad_stack(wad_stack: &WadStack, map_name: &str) -> Result<Self, LevelError> {
        let (wad, group) = wad_stack
            .find_map_lump_group(map_name)
            .ok_or_else(|| LevelError::NotFound(map_name.to_owned()))?;

        Self::from_group(wad, group, map_name)
    }

    fn from_group(
        wad: &WadFile,
        group: MapLumpGroup<'_>,
        map_name: &str,
    ) -> Result<Self, LevelError> {
        let name = map_name.to_uppercase();

        match group {
            MapLumpGroup::Classic(group) => {
                let things = Thing::parse_lump(wad.lump_data(group.lumps[0])).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let linedefs = Linedef::parse_lump(wad.lump_data(group.lumps[1])).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let sidedefs = Sidedef::parse_lump(wad.lump_data(group.lumps[2])).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let vertexes = Vertex::parse_lump(wad.lump_data(group.lumps[3])).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let segs = Seg::parse_lump(wad.lump_data(group.lumps[4])).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let ssectors = Ssector::parse_lump(wad.lump_data(group.lumps[5])).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let nodes = Node::parse_lump(wad.lump_data(group.lumps[6])).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let sectors = Sector::parse_lump(wad.lump_data(group.lumps[7])).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let reject = Reject::parse_lump(wad.lump_data(group.lumps[8]), sectors.len())
                    .map_err(|e| LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    })?;
                let blockmap =
                    Blockmap::parse_lump(wad.lump_data(group.lumps[9])).map_err(|e| {
                        LevelError::ParseError {
                            map: name.clone(),
                            source: e,
                        }
                    })?;

                Self::build_validated(
                    map_name, name, things, linedefs, sidedefs, vertexes, segs, ssectors, nodes,
                    sectors, reject, blockmap,
                )
            }
            MapLumpGroup::Udmf(group) => {
                let udmf =
                    UdmfMap::parse(wad.lump_data(group.textmap)).map_err(|e| LevelError::Udmf {
                        map: name.clone(),
                        source: e,
                    })?;
                if udmf.namespace != "doom" {
                    return Err(LevelError::UnsupportedUdmfNamespace {
                        map: name.clone(),
                        namespace: udmf.namespace.clone(),
                        detail: unsupported_udmf_namespace_detail(&group),
                    });
                }
                let geometry = udmf.into_level_data().map_err(|e| LevelError::Udmf {
                    map: name.clone(),
                    source: e,
                })?;

                let seg_lump = group.find_lump("SEGS").ok_or(LevelError::MissingLump {
                    map: name.clone(),
                    lump: "SEGS",
                })?;
                let ssector_lump = group.find_lump("SSECTORS").ok_or(LevelError::MissingLump {
                    map: name.clone(),
                    lump: "SSECTORS",
                })?;
                let node_lump = group.find_lump("NODES").ok_or(LevelError::MissingLump {
                    map: name.clone(),
                    lump: "NODES",
                })?;
                let reject_lump = group.find_lump("REJECT").ok_or(LevelError::MissingLump {
                    map: name.clone(),
                    lump: "REJECT",
                })?;
                let blockmap_lump = group.find_lump("BLOCKMAP").ok_or(LevelError::MissingLump {
                    map: name.clone(),
                    lump: "BLOCKMAP",
                })?;

                let segs = Seg::parse_lump(wad.lump_data(seg_lump)).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let ssectors = Ssector::parse_lump(wad.lump_data(ssector_lump)).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let nodes = Node::parse_lump(wad.lump_data(node_lump)).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;
                let reject = Reject::parse_lump(wad.lump_data(reject_lump), geometry.sectors.len())
                    .map_err(|e| LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    })?;
                let blockmap = Blockmap::parse_lump(wad.lump_data(blockmap_lump)).map_err(|e| {
                    LevelError::ParseError {
                        map: name.clone(),
                        source: e,
                    }
                })?;

                Self::build_validated(
                    map_name,
                    name,
                    geometry.things,
                    geometry.linedefs,
                    geometry.sidedefs,
                    geometry.vertexes,
                    segs,
                    ssectors,
                    nodes,
                    geometry.sectors,
                    reject,
                    blockmap,
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_validated(
        map_name: &str,
        name: String,
        things: Vec<Thing>,
        linedefs: Vec<Linedef>,
        sidedefs: Vec<Sidedef>,
        vertexes: Vec<Vertex>,
        segs: Vec<Seg>,
        ssectors: Vec<Ssector>,
        nodes: Vec<Node>,
        sectors: Vec<Sector>,
        reject: Reject,
        blockmap: Blockmap,
    ) -> Result<Self, LevelError> {
        BspTree::validate(&nodes, &ssectors, segs.len()).map_err(|e| LevelError::BspInvalid {
            map: name.clone(),
            source: e,
        })?;

        let n_verts = vertexes.len();
        for (i, ld) in linedefs.iter().enumerate() {
            for v in [ld.from_vertex as usize, ld.to_vertex as usize] {
                if v >= n_verts {
                    return Err(LevelError::LindefVertexOutOfBounds {
                        map: name.clone(),
                        idx: i,
                        v,
                        n: n_verts,
                    });
                }
            }
        }

        for (i, ld) in linedefs.iter().enumerate() {
            if ld.is_two_sided() && ld.left_sidedef == SIDEDEF_NONE {
                return Err(LevelError::TwoSidedMissingLeft {
                    map: name.clone(),
                    idx: i,
                });
            }
        }

        let n_sectors = sectors.len();
        for (i, sd) in sidedefs.iter().enumerate() {
            if sd.sector as usize >= n_sectors {
                return Err(LevelError::SidedefSectorOutOfBounds {
                    map: name.clone(),
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
    ///
    /// # Panics
    /// Panics if the BSP invariant is violated post-load, which indicates a
    /// memory corruption or bug because it was successfully validated
    /// upon map load.
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
        let subsector_idx = self.subsector_index_at(x, y)?;
        self.subsector_sector_index(subsector_idx)
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

    /// Return the subsector index containing world point `(x, y)`.
    #[must_use]
    pub fn subsector_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let bsp = BspTree::validate(&self.nodes, &self.ssectors, self.segs.len()).ok()?;
        let ssector = bsp.point_in_subsector(x, y)?;
        self.ssectors
            .iter()
            .position(|candidate| core::ptr::eq(candidate, ssector))
    }

    /// Print a one-line geometry summary (used by the Phase 3 CLI gate).
    ///
    /// ## Examples
    /// ```no_run
    /// use doom_map::Level;
    /// use doom_wad::WadFile;
    ///
    /// let wad = WadFile::parse(std::fs::read("doom1.wad").unwrap()).unwrap();
    /// let level = Level::from_wad(&wad, "E1M1").unwrap();
    /// level.print_stats();
    /// ```
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

fn unsupported_udmf_namespace_detail(group: &doom_wad::UdmfMapLumpGroup<'_>) -> String {
    let mut markers = Vec::new();
    for lump_name in ["ZNODES", "BEHAVIOR", "SCRIPTS"] {
        if group.find_lump(lump_name).is_some() {
            markers.push(lump_name);
        }
    }

    if markers.is_empty() {
        "only Doom-namespace UDMF with classic SEGS/SSECTORS/NODES/REJECT/BLOCKMAP is currently supported"
            .to_string()
    } else {
        format!(
            "found {}; this map targets GZDoom/ZDoom features beyond the currently supported Doom-namespace UDMF + classic SEGS/SSECTORS/NODES/REJECT/BLOCKMAP subset",
            markers.join(", ")
        )
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

    fn build_minimal_udmf_wad_bytes() -> Vec<u8> {
        let marker_name = b"MAP01\0\0\0";
        let textmap = br#"
namespace = "doom";

vertex { x = 0; y = 0; }
vertex { x = 64; y = 0; }
vertex { x = 64; y = 64; }
vertex { x = 0; y = 64; }

sector {
    heightfloor = 0;
    heightceiling = 128;
    texturefloor = "FLAT1";
    textureceiling = "FLAT2";
    lightlevel = 192;
    special = 0;
    id = 0;
}

sidedef { sector = 0; texturemiddle = "WALL1"; offsetx = 0; offsety = 0; }
sidedef { sector = 0; texturemiddle = "WALL1"; offsetx = 0; offsety = 0; }
sidedef { sector = 0; texturemiddle = "WALL1"; offsetx = 0; offsety = 0; }
sidedef { sector = 0; texturemiddle = "WALL1"; offsetx = 0; offsety = 0; }

linedef { v1 = 0; v2 = 1; sidefront = 0; special = 0; arg0 = 0; }
linedef { v1 = 1; v2 = 2; sidefront = 1; special = 0; arg0 = 0; }
linedef { v1 = 2; v2 = 3; sidefront = 2; special = 0; arg0 = 0; }
linedef { v1 = 3; v2 = 0; sidefront = 3; special = 0; arg0 = 0; }

thing {
    x = 0;
    y = 0;
    angle = 0;
    type = 1;
    skill1 = true;
    skill2 = true;
    skill3 = true;
    skill4 = true;
    skill5 = true;
    ambush = false;
    single = true;
    coop = true;
    dm = true;
}
"#;

        let mut sector_data = vec![0u8; 26];
        sector_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        sector_data[2..4].copy_from_slice(&128i16.to_le_bytes());
        sector_data[4..12].copy_from_slice(b"FLAT1\0\0\0");
        sector_data[12..20].copy_from_slice(b"FLAT2\0\0\0");
        sector_data[20..22].copy_from_slice(&192i16.to_le_bytes());
        sector_data[22..24].copy_from_slice(&0u16.to_le_bytes());
        sector_data[24..26].copy_from_slice(&0u16.to_le_bytes());

        let mut seg_data = vec![0u8; 12];
        seg_data[0..2].copy_from_slice(&0u16.to_le_bytes());
        seg_data[2..4].copy_from_slice(&1u16.to_le_bytes());
        seg_data[4..6].copy_from_slice(&0u16.to_le_bytes());
        seg_data[6..8].copy_from_slice(&0u16.to_le_bytes());
        seg_data[8..10].copy_from_slice(&0u16.to_le_bytes());
        seg_data[10..12].copy_from_slice(&0u16.to_le_bytes());

        let mut ss_data = vec![0u8; 4];
        ss_data[0..2].copy_from_slice(&1u16.to_le_bytes());
        ss_data[2..4].copy_from_slice(&0u16.to_le_bytes());

        let node_data: Vec<u8> = vec![];
        let reject_data = vec![0u8; 1];

        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        bm_data[2..4].copy_from_slice(&0i16.to_le_bytes());
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());

        let lump_payloads: &[(&[u8; 8], &[u8])] = &[
            (marker_name, &[]),
            (b"TEXTMAP\0", textmap),
            (b"SEGS\0\0\0\0", &seg_data),
            (b"SSECTORS", &ss_data),
            (b"NODES\0\0\0", &node_data),
            (b"SECTORS\0", &sector_data),
            (b"REJECT\0\0", &reject_data),
            (b"BLOCKMAP", &bm_data),
            (b"ENDMAP\0\0", &[]),
        ];

        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lump_payloads.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());

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

        data
    }

    fn build_udmf_wad_missing_blockmap_bytes() -> Vec<u8> {
        let marker_name = b"MAP01\0\0\0";
        let textmap = br#"
namespace = "doom";
vertex { x = 0; y = 0; }
sector {
    heightfloor = 0;
    heightceiling = 128;
    texturefloor = "FLAT1";
    textureceiling = "FLAT2";
    lightlevel = 192;
}
sidedef { sector = 0; texturemiddle = "WALL1"; }
linedef { v1 = 0; v2 = 0; sidefront = 0; }
"#;

        let mut seg_data = vec![0u8; 12];
        seg_data[0..2].copy_from_slice(&0u16.to_le_bytes());
        seg_data[2..4].copy_from_slice(&0u16.to_le_bytes());

        let mut ss_data = vec![0u8; 4];
        ss_data[0..2].copy_from_slice(&1u16.to_le_bytes());
        ss_data[2..4].copy_from_slice(&0u16.to_le_bytes());

        let node_data: Vec<u8> = vec![];
        let reject_data = vec![0u8; 1];

        let lump_payloads: &[(&[u8; 8], &[u8])] = &[
            (marker_name, &[]),
            (b"TEXTMAP\0", textmap),
            (b"SEGS\0\0\0\0", &seg_data),
            (b"SSECTORS", &ss_data),
            (b"NODES\0\0\0", &node_data),
            (b"REJECT\0\0", &reject_data),
            (b"ENDMAP\0\0", &[]),
        ];

        let mut data = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lump_payloads.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());

        let mut offsets = Vec::new();
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

        data
    }

    fn build_zdoom_targeted_udmf_wad_bytes() -> Vec<u8> {
        let marker_name = b"MAP01\0\0\0";
        let textmap = br#"
namespace = "zdoom";

vertex { x = 0; y = 0; }
vertex { x = 64; y = 0; }
vertex { x = 64; y = 64; }
vertex { x = 0; y = 64; }

sector {
    heightfloor = 0;
    heightceiling = 128;
    texturefloor = "FLAT1";
    textureceiling = "FLAT2";
    lightlevel = 192;
}

sidedef { sector = 0; texturemiddle = "WALL1"; }
linedef { v1 = 0; v2 = 1; sidefront = 0; special = 80; arg0 = 1; }
thing { x = 0; y = 0; angle = 0; type = 1; special = 80; arg0str = "lift_down"; }
"#;

        let lump_payloads: &[(&[u8; 8], &[u8])] = &[
            (marker_name, &[]),
            (b"TEXTMAP\0", textmap),
            (b"BEHAVIOR", b"acs bytecode"),
            (b"ZNODES\0\0", b"XGL3placeholder"),
            (b"SCRIPTS\0", b"script 1 (void) {}"),
            (b"ENDMAP\0\0", &[]),
        ];

        let mut data = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lump_payloads.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());

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
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("value must exist in test");
        assert!(matches!(
            Level::from_wad(&wad, "E2M1"),
            Err(LevelError::NotFound(_))
        ));
    }

    #[test]
    fn level_stats_smoke() {
        let wad_bytes = build_minimal_wad_bytes();
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("value must exist in test");
        let level = Level::from_wad(&wad, "E1M1").expect("value must exist in test");
        // Just ensure print_stats doesn't panic.
        level.print_stats();
    }

    #[test]
    fn subsector_sector_index_respects_seg_direction() {
        let reject = Reject::parse_lump(&[0u8], 2).expect("value must exist in test");
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("value must exist in test");

        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![
                Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0x0004,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
                Linedef {
                    from_vertex: 2,
                    to_vertex: 3,
                    flags: 0x0004,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
            ],
            sidedefs: vec![
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"        ",
                    lower_texture: *b"        ",
                    middle_texture: *b"WALL0   ",
                    sector: 0,
                },
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"        ",
                    lower_texture: *b"        ",
                    middle_texture: *b"WALL1   ",
                    sector: 1,
                },
            ],
            vertexes: vec![
                Vertex { x: 0, y: 0 },
                Vertex { x: 0, y: 64 },
                Vertex { x: 64, y: 0 },
                Vertex { x: 64, y: 64 },
            ],
            segs: vec![
                Seg {
                    from_vertex: 0,
                    to_vertex: 1,
                    angle: 0,
                    linedef: 0,
                    direction: 0,
                    offset: 0,
                },
                Seg {
                    from_vertex: 3,
                    to_vertex: 2,
                    angle: 0,
                    linedef: 1,
                    direction: 1,
                    offset: 0,
                },
            ],
            ssectors: vec![
                Ssector {
                    seg_count: 1,
                    first_seg: 0,
                },
                Ssector {
                    seg_count: 1,
                    first_seg: 1,
                },
            ],
            nodes: vec![],
            sectors: vec![
                Sector {
                    floor_height: 0,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                Sector {
                    floor_height: 64,
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
        };

        assert_eq!(level.subsector_sector_index(0), Some(0));
        assert_eq!(level.subsector_sector_index(1), Some(1));
    }

    #[test]
    fn subsector_index_at_returns_leaf_index() {
        let wad_bytes = build_minimal_wad_bytes();
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("value must exist in test");
        let level = Level::from_wad(&wad, "E1M1").expect("value must exist in test");

        assert_eq!(level.subsector_index_at(10, 10), Some(0));
    }

    #[test]
    fn sector_index_at_uses_subsector_sector_in_valid_level() {
        let wad_bytes = build_minimal_wad_bytes();
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("value must exist in test");
        let level = Level::from_wad(&wad, "E1M1").expect("value must exist in test");

        assert_eq!(level.sector_index_at(10, 10), Some(0));
        assert_eq!(level.floor_at(10, 10), Some(0));
    }

    #[test]
    fn load_minimal_udmf_level() {
        let wad_bytes = build_minimal_udmf_wad_bytes();
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("WAD parse failed");
        let level = Level::from_wad(&wad, "MAP01").expect("UDMF level load failed");

        assert_eq!(level.name, "MAP01");
        assert_eq!(level.sectors.len(), 1);
        assert_eq!(level.vertexes.len(), 4);
        assert_eq!(level.linedefs.len(), 4);
        assert_eq!(level.sidedefs.len(), 4);
        assert_eq!(level.things.len(), 1);
        assert_eq!(level.segs.len(), 1);
        assert_eq!(level.ssectors.len(), 1);
        assert_eq!(level.nodes.len(), 0);
    }

    #[test]
    fn udmf_missing_blockmap_lump_errors() {
        let wad_bytes = build_udmf_wad_missing_blockmap_bytes();
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("WAD parse failed");

        assert!(matches!(
            Level::from_wad(&wad, "MAP01"),
            Err(LevelError::MissingLump {
                lump: "BLOCKMAP",
                ..
            })
        ));
    }

    #[test]
    fn zdoom_targeted_udmf_reports_gzdoom_specific_requirements() {
        let wad_bytes = build_zdoom_targeted_udmf_wad_bytes();
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("WAD parse failed");

        assert!(matches!(
            Level::from_wad(&wad, "MAP01"),
            Err(LevelError::UnsupportedUdmfNamespace {
                namespace,
                detail,
                ..
            }) if namespace == "zdoom"
                && detail.contains("ZNODES")
                && detail.contains("BEHAVIOR")
                && detail.contains("SCRIPTS")
        ));
    }
}
