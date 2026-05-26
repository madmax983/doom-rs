//! Export map geometry to a GeoJSON representation.
//!
//! This module provides the `export_map_to_geojson` function, which translates
//! the 2D map layout and things into a GeoJSON FeatureCollection format.

use crate::Level;

/// Exports a `Level` to a GeoJSON string containing LineStrings and Points.
///
/// # Panics
///
/// Panics if a linedef references a vertex index (`from_vertex` or `to_vertex`) that
/// is out of bounds for the level's `vertexes` array.
///
/// # Examples
///
/// ```
/// use doom_map::{Level, lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Thing, Vertex}};
///
/// let reject = Reject::parse_lump(&[0u8], 1).unwrap();
/// let mut bm_data = vec![0u8; 14];
/// bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
/// bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
/// bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
/// bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
/// bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
/// let blockmap = Blockmap::parse_lump(&bm_data).unwrap();
///
/// let level = Level {
///     name: "TEST".to_owned(),
///     things: vec![Thing {
///         x: 32,
///         y: 32,
///         angle: 0,
///         kind: 1,
///         flags: 0,
///     }],
///     vertexes: vec![
///         Vertex { x: 0, y: 0 },
///         Vertex { x: 64, y: 0 },
///     ],
///     linedefs: vec![
///         Linedef {
///             from_vertex: 0,
///             to_vertex: 1,
///             flags: 0,
///             special: 0,
///             tag: 0,
///             right_sidedef: 0,
///             left_sidedef: 0xFFFF,
///         },
///     ],
///     sidedefs: vec![
///         Sidedef {
///             x_offset: 0,
///             y_offset: 0,
///             upper_texture: *b"WALL1\0\0\0",
///             lower_texture: *b"WALL2\0\0\0",
///             middle_texture: *b"WALL3\0\0\0",
///             sector: 0,
///         },
///     ],
///     sectors: vec![Sector {
///         floor_height: 0,
///         ceil_height: 128,
///         floor_flat: *b"FLAT1\0\0\0",
///         ceil_flat: *b"FLAT2\0\0\0",
///         light_level: 192,
///         special: 0,
///         tag: 0,
///     }],
///     segs: vec![],
///     ssectors: vec![],
///     nodes: vec![],
///     reject,
///     blockmap,
/// };
///
/// let geojson = doom_map::export_map_to_geojson(&level);
/// assert!(geojson.contains(r#""type": "FeatureCollection""#));
/// assert!(geojson.contains(r#""type": "LineString""#));
/// assert!(geojson.contains(r#"[[0, 0], [64, 0]]"#));
/// assert!(geojson.contains(r#""type": "Point""#));
/// assert!(geojson.contains(r#"[32, 32]"#));
/// ```
pub fn export_map_to_geojson(level: &Level) -> String {
    use std::fmt::Write;

    // Estimate capacity: 250 bytes per feature
    let capacity = 100 + (level.linedefs.len() + level.things.len()) * 250;
    let mut features_str = String::with_capacity(capacity);
    let mut first = true;

    // Map linedefs to LineString features
    for ld in &level.linedefs {
        let v1 = &level.vertexes[ld.from_vertex as usize];
        let v2 = &level.vertexes[ld.to_vertex as usize];

        if !first {
            features_str.push_str(",\n");
        }
        first = false;

        // We include some basic properties like flags and special
        let _ = write!(
            features_str,
            r#"    {{
      "type": "Feature",
      "geometry": {{
        "type": "LineString",
        "coordinates": [[{}, {}], [{}, {}]]
      }},
      "properties": {{
        "flags": {},
        "special": {},
        "tag": {}
      }}
    }}"#,
            v1.x, v1.y, v2.x, v2.y, ld.flags, ld.special, ld.tag
        );
    }

    // Map things to Point features
    for thing in &level.things {
        if !first {
            features_str.push_str(",\n");
        }
        first = false;

        let _ = write!(
            features_str,
            r#"    {{
      "type": "Feature",
      "geometry": {{
        "type": "Point",
        "coordinates": [{}, {}]
      }},
      "properties": {{
        "angle": {},
        "kind": {},
        "flags": {}
      }}
    }}"#,
            thing.x, thing.y, thing.angle, thing.kind, thing.flags
        );
    }

    let mut result = String::with_capacity(features_str.len() + 100);
    let _ = write!(
        result,
        r#"{{
  "type": "FeatureCollection",
  "features": [
{}
  ]
}}"#,
        features_str
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Thing, Vertex};

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
            things: vec![Thing {
                x: 32,
                y: 32,
                angle: 0,
                kind: 1,
                flags: 0,
            }],
            linedefs: vec![Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            }],
            sidedefs: vec![Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"WALL1\0\0\0",
                lower_texture: *b"WALL2\0\0\0",
                middle_texture: *b"WALL3\0\0\0",
                sector: 0,
            }],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 64, y: 0 }],
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
    fn test_export_geojson() {
        let level = make_test_level();
        let geojson = export_map_to_geojson(&level);

        assert!(geojson.contains(r#""type": "FeatureCollection""#));
        assert!(geojson.contains(r#""type": "LineString""#));
        assert!(geojson.contains(r#""coordinates": [[0, 0], [64, 0]]"#));
        assert!(geojson.contains(r#""type": "Point""#));
        assert!(geojson.contains(r#""coordinates": [32, 32]"#));
        assert!(geojson.contains(r#""type": "Feature""#));
    }
}
