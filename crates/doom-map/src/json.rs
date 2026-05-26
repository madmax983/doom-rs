//! JSON export functionality for map layouts.
//!
//! This module provides the [`export_map_to_json`] function, which
//! serializes a high-level summary of a [`Level`]'s statistics
//! and composition into a JSON string.
//!
//! # Examples
//! ```
//! use doom_map::Level;
//! use doom_map::export_map_to_json;
//! use doom_map::{Blockmap, Reject};
//!
//! // Create a minimal empty level
//! let level = Level {
//!     name: "E1M1".to_string(),
//!     things: vec![],
//!     linedefs: vec![],
//!     sidedefs: vec![],
//!     vertexes: vec![],
//!     segs: vec![],
//!     ssectors: vec![],
//!     nodes: vec![],
//!     sectors: vec![],
//!     reject: Reject::parse_lump(&[], 0).unwrap(),
//!     blockmap: Blockmap::parse_lump(&[0, 0, 0, 0, 0, 0, 0, 0]).unwrap(),
//! };
//!
//! let json_output = export_map_to_json(&level);
//! assert!(json_output.contains(r#""name": "E1M1""#));
//! assert!(json_output.contains(r#""sectors": 0"#));
//! ```

use crate::Level;

/// Exports the level's basic layout and statistics to a JSON string.
///
/// This does not include the full vertex/linedef geometry, but instead
/// serves as a high-level summary of the level's stats and composition.
#[must_use]
pub fn export_map_to_json(level: &Level) -> String {
    format!(
        r#"{{
  "name": "{}",
  "stats": {{
    "things": {},
    "linedefs": {},
    "sidedefs": {},
    "vertexes": {},
    "sectors": {},
    "segs": {},
    "ssectors": {},
    "nodes": {}
  }}
}}"#,
        level.name,
        level.things.len(),
        level.linedefs.len(),
        level.sidedefs.len(),
        level.vertexes.len(),
        level.sectors.len(),
        level.segs.len(),
        level.ssectors.len(),
        level.nodes.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Reject, Sector};

    fn make_test_level() -> Level {
        let reject = Reject::parse_lump(&[0u8], 1).expect("value must exist in test");
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("value must exist in test");

        Level {
            name: "TEST".to_owned(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            }],
            reject,
            blockmap,
        }
    }

    #[test]
    fn test_export_map_to_json() {
        let level = make_test_level();
        let json = export_map_to_json(&level);

        assert!(json.contains(r#""name": "TEST""#));
        assert!(json.contains(r#""sectors": 1"#));
        assert!(json.contains(r#""things": 0"#));
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }
}
