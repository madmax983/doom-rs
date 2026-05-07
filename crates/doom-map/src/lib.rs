//! Map data parsing: THINGS, LINEDEFS, SIDEDEFS, SECTORS, SEGS, SSECTORS,
//! NODES, BLOCKMAP, REJECT, and UDMF TextMap.
//!
//! # Key invariants verified at load time
//! - `N_SSECTORS == N_NODES + 1` (BSP full-binary-tree theorem)
//! - All BSP child pointers in bounds
//! - All linedef vertex refs in bounds
//! - Two-sided flag ↔ both sidedefs present
//! - All sidedef sector refs in bounds
//! - Reject size == `ceil(N_SECTORS² / 8)`

/// ASCII rendering and exporting.
pub mod ascii;
/// Binary Space Partitioning tree structures.
pub mod bsp;
/// Export maps to GeoJSON format.
pub mod geojson;
/// Topological graph representing sector connectivity.
pub mod graph;
/// Export maps to HTML format.
pub mod html;
/// Export maps to JSON format.
pub mod json;
/// Core level definition and loading mechanics.
pub mod level;
/// Raw WAD lump parsing and structures.
pub mod lumps;
/// Export maps to OBJ 3D format.
pub mod obj;
/// Export maps to SVG format.
pub mod svg;
/// Universal Doom Map Format (UDMF) parsing.
pub mod udmf;

pub use ascii::export_map_to_ascii;
pub use bsp::{BspChild, BspError, BspTree};
pub use geojson::export_map_to_geojson;
pub use graph::SectorGraph;
pub use html::export_map_to_html;
pub use json::export_map_to_json;
pub use level::{Level, LevelError};
pub use lumps::{
    Blockmap, FLAG_TWO_SIDED, Linedef, LumpParseError, Node, NodeBBox, Reject, SIDEDEF_NONE,
    Sector, Seg, Sidedef, Ssector, Thing, Vertex,
};
pub use obj::export_map_to_obj;
pub use svg::export_map_to_svg;

/// Structural analysis for Doom maps.
pub mod analyzer;
pub use analyzer::MapAnalyzer;
