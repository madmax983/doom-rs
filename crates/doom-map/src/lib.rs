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

pub(crate) mod bsp;
pub(crate) mod geojson;
pub(crate) mod html;
pub(crate) mod level;
pub(crate) mod lumps;
pub(crate) mod obj;
pub(crate) mod svg;
pub(crate) mod udmf;

pub use bsp::{BspChild, BspError, BspTree};
pub use geojson::export_map_to_geojson;
pub use html::export_map_to_html;
pub use level::{Level, LevelError};
pub use lumps::{
    FLAG_DONTPEGBOTTOM,
    FLAG_DONTPEGTOP,
    FLAG_BLOCKING,
    FLAG_BLOCKMONSTERS,
    NODE_SUBSECTOR_BIT,
    Blockmap, FLAG_TWO_SIDED, Linedef, LumpParseError, Node, NodeBBox, Reject, SIDEDEF_NONE,
    Sector, Seg, Sidedef, Ssector, Thing, Vertex,
};
pub use obj::export_map_to_obj;
pub use svg::export_map_to_svg;

pub use udmf::UdmfMap;
