//! WAD file parsing, lump directory management, and PWAD stack.
//!
//! Supports IWAD (base) + PWAD (patch) stacking with runtime-validated
//! structural invariants: no out-of-bounds lumps.
//!
//! # Key types
//! - [`WadFile`] — a parsed, validated single WAD file
//! - [`WadStack`] — IWAD + PWADs with override-resolution semantics
//! - [`LumpDef`] — a validated lump descriptor (offset, size, name)

/// Lump definitions.
pub mod lump;
/// Wad stack management.
pub mod stack;
/// Wad file parsing.
pub mod wad;

pub use lump::{LumpDef, LumpName, RawLumpEntry};
pub use stack::WadStack;
pub use wad::{
    ClassicMapLumpGroup, MapLumpGroup, REQUIRED_MAP_LUMPS, UdmfMapLumpGroup, WadDir, WadError,
    WadFile, WadKind,
};
