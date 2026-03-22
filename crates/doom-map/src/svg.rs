use crate::level::Level;
use crate::lumps::SIDEDEF_NONE;

/// Exports a Doom level's linedefs to an SVG string.
pub struct SvgExporter;

impl SvgExporter {
    /// Generates an SVG representation of the map, rendering walls and two-sided lines.
    /// Uses `transform="scale(1, -1)"` to map Doom's Y-up coordinates to SVG's Y-down.
    #[must_use]
    pub fn export(level: &Level) -> String {
        if level.vertexes.is_empty() || level.linedefs.is_empty() {
            return String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" />");
        }

        // Calculate bounding box.
        let mut min_x = i16::MAX;
        let mut max_x = i16::MIN;
        let mut min_y = i16::MAX;
        let mut max_y = i16::MIN;

        for v in &level.vertexes {
            min_x = min_x.min(v.x);
            max_x = max_x.max(v.x);
            min_y = min_y.min(v.y);
            max_y = max_y.max(v.y);
        }

        // Add padding
        let padding = 128;
        let view_min_x = i32::from(min_x) - padding;
        let view_max_x = i32::from(max_x) + padding;
        let view_min_y = i32::from(min_y) - padding;
        let view_max_y = i32::from(max_y) + padding;

        let width = view_max_x - view_min_x;
        let height = view_max_y - view_min_y;

        // Note: viewBox uses inverted Y coordinates for the transformation.
        // If we map (x, y) to (x, -y), the new Y bounds become [-max_y, -min_y].
        // With padding, the viewBox top-left is (view_min_x, -view_max_y).
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" style=\"background-color:#111;\">\n",
            view_min_x, -view_max_y, width, height
        );

        // Group with Y-inversion transform and styling
        svg.push_str("  <g transform=\"scale(1, -1)\" stroke-linecap=\"round\">\n");

        for line in &level.linedefs {
            let Some(v1) = level.vertexes.get(usize::from(line.from_vertex)) else {
                continue;
            };
            let Some(v2) = level.vertexes.get(usize::from(line.to_vertex)) else {
                continue;
            };

            let is_two_sided =
                line.left_sidedef != SIDEDEF_NONE && line.right_sidedef != SIDEDEF_NONE;

            let color = if is_two_sided { "#555" } else { "#eee" };
            let stroke_width = if is_two_sided { 8 } else { 16 };

            svg.push_str(&format!(
                "    <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\" />\n",
                v1.x, v1.y, v2.x, v2.y, color, stroke_width
            ));
        }

        svg.push_str("  </g>\n");
        svg.push_str("</svg>");

        svg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Linedef, Reject, Vertex};

    #[test]
    fn test_svg_exporter_empty() {
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
            reject: Reject::parse_lump(&[0], 0).unwrap_or(Reject::parse_lump(&[], 0).unwrap()),
            blockmap: Blockmap::parse_lump(&[0; 14]).unwrap_or_else(|_| {
                Blockmap::parse_lump(&[0, 0, 0, 0, 1, 0, 1, 0, 5, 0, 0, 0, 255, 255]).unwrap()
            }),
        };
        let svg = SvgExporter::export(&level);
        assert_eq!(svg, "<svg xmlns=\"http://www.w3.org/2000/svg\" />");
    }

    #[test]
    fn test_svg_exporter_geometry() {
        let level = Level {
            name: "TEST".to_string(),
            things: vec![],
            linedefs: vec![
                Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: SIDEDEF_NONE,
                },
                Linedef {
                    from_vertex: 1,
                    to_vertex: 0,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
            ],
            sidedefs: vec![],
            vertexes: vec![Vertex { x: -10, y: -10 }, Vertex { x: 10, y: 10 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![],
            reject: Reject::parse_lump(&[0], 0).unwrap_or(Reject::parse_lump(&[], 0).unwrap()),
            blockmap: Blockmap::parse_lump(&[0; 14]).unwrap_or_else(|_| {
                Blockmap::parse_lump(&[0, 0, 0, 0, 1, 0, 1, 0, 5, 0, 0, 0, 255, 255]).unwrap()
            }),
        };

        let svg = SvgExporter::export(&level);

        assert!(svg.contains("<svg"), "Must contain opening svg tag");
        assert!(svg.contains("viewBox="), "Must contain viewBox attribute");
        assert!(
            svg.contains("transform=\"scale(1, -1)\""),
            "Must apply Y-up transform"
        );
        assert!(
            svg.contains("stroke=\"#eee\""),
            "Must draw single-sided line"
        );
        assert!(svg.contains("stroke=\"#555\""), "Must draw two-sided line");
        assert!(
            svg.contains("x1=\"-10\" y1=\"-10\""),
            "Must include vertex coordinates"
        );
    }
}
