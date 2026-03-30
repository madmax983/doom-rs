//! Exporter module for Doom maps.
//!
//! Provides functionality to export a parsed `Level` to various external formats
//! such as SVG, allowing for easy external visualization of map geometry.

use crate::Level;

/// Exports the geometry of a `Level` as a standalone SVG string.
///
/// This is an implementation of "The Exporter" pattern. It extracts all
/// linedefs and vertices to produce a top-down view of the map.
/// The output coordinates are scaled and translated so the map fits
/// neatly inside the SVG viewport with a small padding.
pub fn export_svg(level: &Level) -> String {
    if level.vertexes.is_empty() {
        return r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><text x="10" y="50">Empty Map</text></svg>"#.to_string();
    }

    let mut min_x = i16::MAX as i32;
    let mut min_y = i16::MAX as i32;
    let mut max_x = i16::MIN as i32;
    let mut max_y = i16::MIN as i32;

    for v in &level.vertexes {
        let x = v.x as i32;
        let y = v.y as i32;
        if x < min_x { min_x = x; }
        if y < min_y { min_y = y; }
        if x > max_x { max_x = x; }
        if y > max_y { max_y = y; }
    }

    let padding = 128;
    let width = (max_x - min_x) + padding * 2;
    let height = (max_y - min_y) + padding * 2;

    // We flip the Y-axis so that Doom's positive-Y (up) matches SVG's positive-Y (down).
    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" style="background-color: #1e1e2e;">"#,
        width, height
    ));
    svg.push('\n');

    // Draw all linedefs
    for line in &level.linedefs {
        let v1 = &level.vertexes[line.from_vertex as usize];
        let v2 = &level.vertexes[line.to_vertex as usize];

        let x1 = (v1.x as i32 - min_x) + padding;
        let y1 = height - ((v1.y as i32 - min_y) + padding);
        let x2 = (v2.x as i32 - min_x) + padding;
        let y2 = height - ((v2.y as i32 - min_y) + padding);

        // One-sided linedefs are drawn in red (walls), two-sided in gray (steps/windows).
        let color = if line.is_two_sided() { "#585b70" } else { "#f38ba8" };
        let thickness = if line.is_two_sided() { 2 } else { 4 };

        svg.push_str(&format!(
            r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="round" />"#,
            x1, y1, x2, y2, color, thickness
        ));
        svg.push('\n');
    }

    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Linedef, Vertex};
    use crate::Level;

    #[test]
    fn test_export_svg_empty() {
        let level = Level {
            name: "EMPTY".to_string(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![],
            reject: crate::lumps::Reject::parse_lump(&[], 0).unwrap(),
            blockmap: crate::lumps::Blockmap::parse_lump(&[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]).unwrap(),
        };

        let svg = export_svg(&level);
        assert!(svg.contains("Empty Map"));
    }

    #[test]
    fn test_export_svg_basic() {
        let level = Level {
            name: "E1M1".to_string(),
            things: vec![],
            linedefs: vec![
                Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 0xFFFF,
                }
            ],
            sidedefs: vec![],
            vertexes: vec![
                Vertex { x: 0, y: 0 },
                Vertex { x: 100, y: 100 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![],
            reject: crate::lumps::Reject::parse_lump(&[], 0).unwrap(),
            blockmap: crate::lumps::Blockmap::parse_lump(&[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]).unwrap(),
        };

        let svg = export_svg(&level);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("<line"));
        assert!(svg.contains("stroke=\"#f38ba8\""));
    }
}
