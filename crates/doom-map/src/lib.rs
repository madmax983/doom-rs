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

/// Map to ASCII export.
pub mod ascii;
/// BSP tree traversal.
pub mod bsp;
/// Map to GeoJSON export.
pub mod geojson;
/// Map geometry analysis graph.
pub mod graph;
/// Map to HTML export.
pub mod html;
/// Map to JSON export.
pub mod json;
/// Core Level struct.
pub mod level;
/// Map lump definitions.
pub mod lumps;
/// Map to OBJ export.
pub mod obj;
/// Map to SVG export.
pub mod svg;
/// UDMF text map parsing.
///
/// 1. **Parsing** ([`crate::udmf::UdmfMap::parse`]): We read the raw UTF-8 bytes of a `TEXTMAP` and parse it into an Abstract Syntax Tree (AST). This is represented by [`crate::udmf::UdmfMap`], containing raw [`crate::udmf::UdmfBlock`]s and [`crate::udmf::UdmfField`]s.
/// 2. **Conversion** ([`crate::udmf::UdmfMap::into_level_data`]): The engine doesn't want an AST; it wants flat, fast arrays of vertices, sectors, and sidedefs to render at 60 FPS. We convert the raw UDMF AST into [`crate::udmf::UdmfLevelData`], which matches the classic binary shape the engine expects.
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

/// Map graph analysis.
pub mod analyzer;
pub use analyzer::MapAnalyzer;
