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

pub mod ascii;
pub mod bsp;
pub mod geojson;
pub mod geometry;
pub mod graph;
pub mod html;
pub mod json;
pub mod level;
pub mod lumps;
pub mod obj;
pub mod svg;
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

pub mod analyzer;
pub use analyzer::MapAnalyzer;
pub use geometry::{GeometryAnalyzer, SectorGeometry};
