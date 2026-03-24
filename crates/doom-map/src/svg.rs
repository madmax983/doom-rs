//! Export map geometry to an SVG vector graphic.
//!
//! This module provides the `export_map_to_svg` function, which is useful for debugging
//! spatial structures (like BSP generation bugs, vertex winding orders, or sector
//! bounds) visually without needing to boot up the entire 3D renderer. By producing
//! a 2D overhead view of a [`Level`], developers and mappers can quickly verify that
//! their parsed map matches their intentions.

use crate::Level;

/// Exports a `Level` to an SVG XML string.
///
/// This function generates a top-down, 2D vector graphic representation of the entire
/// map structure. It is specifically designed to give a fast, visual sanity-check
/// of map parsers and procedural level generators. One-sided linedefs (solid walls)
/// are drawn as thick, bright lines, while two-sided linedefs (portals, windows, doors)
/// are drawn as thinner, dimmer lines to differentiate solid boundaries from open space.
///
/// Map things (monsters, items, spawn points) are rendered as red circles.
///
/// # Examples
///
/// Building a minimal square room and exporting it to SVG:
///
/// ```
/// use doom_map::{Level, lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex}};
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
///     things: vec![],
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
/// let svg_xml = doom_map::export_map_to_svg(&level);
/// assert!(svg_xml.contains("<svg viewBox="));
/// assert!(svg_xml.contains("</svg>"));
/// ```
pub fn export_map_to_svg(level: &Level) -> String {
    export_map_with_path_to_svg(level, &[])
}

/// Exports a `Level` to an SVG XML string, overlaying a given spatial path.
///
/// Functions identically to [`export_map_to_svg`] but additionally draws a red
/// polyline along the coordinates provided in `path`. This is useful for
/// visualizing player movement, demo playback, or AI pathfinding over a map.
pub fn export_map_with_path_to_svg(level: &Level, path: &[(i32, i32)]) -> String {
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;

    for v in &level.vertexes {
        min_x = min_x.min(v.x as i32);
        max_x = max_x.max(v.x as i32);
        min_y = min_y.min(v.y as i32);
        max_y = max_y.max(v.y as i32);
    }

    for &(px, py) in path {
        min_x = min_x.min(px);
        max_x = max_x.max(px);
        min_y = min_y.min(py);
        max_y = max_y.max(py);
    }

    if level.vertexes.is_empty() && path.is_empty() {
        min_x = 0;
        max_x = 100;
        min_y = 0;
        max_y = 100;
    }

    // Add some padding
    let pad = 128;
    let width = (max_x - min_x) + pad * 2;
    let height = (max_y - min_y) + pad * 2;
    let v_min_x = min_x - pad;
    let v_min_y = min_y - pad;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg viewBox="0 0 {width} {height}" xmlns="http://www.w3.org/2000/svg" style="background-color: #333;">
"#
    ));
    // Doom's Y axis points UP, SVG's Y axis points DOWN.
    // We scale by (1, -1) and translate to keep things in the view box.
    svg.push_str(&format!(
        r#"<g transform="translate(0, {height}) scale(1, -1) translate({trans_x}, {trans_y})">
"#,
        trans_x = -v_min_x,
        trans_y = -v_min_y
    ));

    // Draw two-sided linedefs first (so they are under one-sided)
    for ld in &level.linedefs {
        if ld.is_two_sided() {
            let v1 = &level.vertexes[ld.from_vertex as usize];
            let v2 = &level.vertexes[ld.to_vertex as usize];
            svg.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#888\" stroke-width=\"2\" />\n",
                v1.x, v1.y, v2.x, v2.y
            ));
        }
    }

    // Draw one-sided linedefs
    for ld in &level.linedefs {
        if !ld.is_two_sided() {
            let v1 = &level.vertexes[ld.from_vertex as usize];
            let v2 = &level.vertexes[ld.to_vertex as usize];
            svg.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#fff\" stroke-width=\"4\" />\n",
                v1.x, v1.y, v2.x, v2.y
            ));
        }
    }

    // Draw things
    for thing in &level.things {
        svg.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"16\" fill=\"#f55\" />\n",
            thing.x, thing.y
        ));
    }

    // Draw the path as a single polyline.
    if !path.is_empty() {
        svg.push_str("<polyline points=\"");
        for (i, &(px, py)) in path.iter().enumerate() {
            if i > 0 {
                svg.push(' ');
            }
            svg.push_str(&format!("{},{}", px, py));
        }
        svg.push_str("\" fill=\"none\" stroke=\"#f55\" stroke-width=\"4\" opacity=\"0.75\" />\n");
    }

    svg.push_str("</g>\n</svg>\n");

    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Thing, Vertex};

    fn make_test_level() -> Level {
        let reject = Reject::parse_lump(&[0u8], 1).unwrap();
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).unwrap();

        Level {
            name: "TEST".to_owned(),
            things: vec![Thing {
                x: 32,
                y: 32,
                angle: 0,
                kind: 1,
                flags: 0,
            }],
            linedefs: vec![
                Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 0xFFFF,
                },
                Linedef {
                    from_vertex: 1,
                    to_vertex: 2,
                    flags: 0x0004, // Two-sided
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
            ],
            sidedefs: vec![
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"WALL1\0\0\0",
                    lower_texture: *b"WALL2\0\0\0",
                    middle_texture: *b"WALL3\0\0\0",
                    sector: 0,
                },
                Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"WALL1\0\0\0",
                    lower_texture: *b"WALL2\0\0\0",
                    middle_texture: *b"WALL3\0\0\0",
                    sector: 0,
                },
            ],
            vertexes: vec![
                Vertex { x: 0, y: 0 },
                Vertex { x: 64, y: 0 },
                Vertex { x: 64, y: 64 },
            ],
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
    fn test_export_svg() {
        let level = make_test_level();
        let svg = export_map_to_svg(&level);

        // Check for basic SVG structure
        assert!(svg.contains("<svg viewBox="));
        assert!(svg.contains("</svg>"));

        // Check for linedef elements
        assert!(svg.contains(
            "<line x1=\"0\" y1=\"0\" x2=\"64\" y2=\"0\" stroke=\"#fff\" stroke-width=\"4\" />"
        ));
        assert!(svg.contains(
            "<line x1=\"64\" y1=\"0\" x2=\"64\" y2=\"64\" stroke=\"#888\" stroke-width=\"2\" />"
        ));

        // Check for thing element
        assert!(svg.contains("<circle cx=\"32\" cy=\"32\" r=\"16\" fill=\"#f55\" />"));
    }

    #[test]
    fn test_export_svg_with_path() {
        let level = make_test_level();
        let path = vec![(0, 0), (32, 32), (64, 64)];
        let svg = export_map_with_path_to_svg(&level, &path);

        assert!(svg.contains("<svg viewBox="));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("<polyline points=\"0,0 32,32 64,64\" fill=\"none\" stroke=\"#f55\" stroke-width=\"4\" opacity=\"0.75\" />"));
    }
}
