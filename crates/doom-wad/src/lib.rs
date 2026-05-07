//! WAD file parsing, lump directory management, and PWAD stack.
//!
//! Supports IWAD (base) + PWAD (patch) stacking with runtime-validated
//! structural invariants: no out-of-bounds lumps.
//!
//! # Key types
//! - [`WadFile`] — a parsed, validated single WAD file
//! - [`WadStack`] — IWAD + PWADs with override-resolution semantics
//! - [`LumpDef`] — a validated lump descriptor (offset, size, name)

/// Lump metadata definitions.
pub mod lump;
/// Multi-WAD (PWAD overriding IWAD) stack logic.
pub mod stack;
/// Core WAD file parsing and querying.
pub mod wad;

pub use lump::{LumpDef, LumpName, RawLumpEntry};
pub use stack::WadStack;
pub use wad::{
    ClassicMapLumpGroup, MapLumpGroup, REQUIRED_MAP_LUMPS, UdmfMapLumpGroup, WadDir, WadError,
    WadFile, WadKind,
};
