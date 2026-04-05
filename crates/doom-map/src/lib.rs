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

pub mod bsp;
pub mod export;
pub mod graph;
pub mod level;
pub mod lumps;
pub mod udmf;

pub use bsp::{BspChild, BspError, BspTree};
pub use export::export_map_to_geojson;
pub use graph::SectorGraph;
pub use export::export_map_to_html;
pub use level::{Level, LevelError};
pub use lumps::{
    Blockmap, FLAG_TWO_SIDED, Linedef, LumpParseError, Node, NodeBBox, Reject, SIDEDEF_NONE,
    Sector, Seg, Sidedef, Ssector, Thing, Vertex,
};
pub use export::export_map_to_obj;
pub use export::export_map_to_svg;
