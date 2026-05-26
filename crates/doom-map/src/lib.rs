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

pub(crate) mod ascii;
pub(crate) mod bsp;
pub(crate) mod geojson;
pub(crate) mod graph;
pub(crate) mod html;
pub(crate) mod json;
pub(crate) mod level;
pub(crate) mod lumps;
pub(crate) mod obj;
pub(crate) mod svg;
pub(crate) mod udmf;

pub use ascii::export_map_to_ascii;
pub use bsp::{BspChild, BspError, BspTree};
pub use geojson::export_map_to_geojson;
pub use graph::SectorGraph;
pub use html::export_map_to_html;
pub use json::export_map_to_json;
pub use level::{Level, LevelError};
pub use lumps::{
    Blockmap, FLAG_BLOCKING, FLAG_BLOCKMONSTERS, FLAG_DONTPEGBOTTOM, FLAG_DONTPEGTOP,
    FLAG_TWO_SIDED, Linedef, LumpParseError, NODE_SUBSECTOR_BIT, Node, NodeBBox, Reject,
    SIDEDEF_NONE, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
};
pub use obj::export_map_to_obj;
pub use svg::export_map_to_svg;
pub use udmf::{UdmfBlock, UdmfError, UdmfField, UdmfLevelData, UdmfMap, UdmfValue};

pub(crate) mod analyzer;
pub use analyzer::MapAnalyzer;
