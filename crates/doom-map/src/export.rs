//! SVG Exporter for exporting Doom maps to vector graphics.

use crate::level::Level;

/// Exporter for converting a `Level` into an SVG vector graphic.
pub struct SvgExporter;

impl SvgExporter {
    /// Generates an SVG representation of the provided `Level`.
    pub fn export(level: &Level) -> String {
        if level.vertexes.is_empty() {
            return String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 0 0" />"#);
        }

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

        let width = (max_x as i32 - min_x as i32).max(1);
        let height = (max_y as i32 - min_y as i32).max(1);

        let mut svg = String::new();
        svg.push_str(&format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" style="background-color: black;">"#,
            width, height
        ));
        svg.push('\n');

        // Apply a transform to invert the Y-axis (since Doom's Y is up) and shift
        // the map coordinates into the positive 0..width and 0..height SVG coordinate space.
        svg.push_str(&format!(
            r#"  <g transform="translate({}, {}) scale(1, -1) translate({}, {})">"#,
            0, height, -min_x, -min_y
        ));
        svg.push('\n');

        for ld in &level.linedefs {
            if let (Some(v1), Some(v2)) = (
                level.vertexes.get(ld.from_vertex as usize),
                level.vertexes.get(ld.to_vertex as usize),
            ) {
                let stroke = if ld.is_two_sided() { "gray" } else { "white" };
                svg.push_str(&format!(
                    r#"    <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" />"#,
                    v1.x, v1.y, v2.x, v2.y, stroke
                ));
                svg.push('\n');
            }
        }

        svg.push_str("  </g>\n</svg>\n");
        svg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Linedef, Reject, Vertex};

    #[test]
    fn test_svg_export() {
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
                    left_sidedef: 0xFFFF,
                },
                Linedef {
                    from_vertex: 1,
                    to_vertex: 2,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 0xFFFF,
                },
            ],
            sidedefs: vec![],
            vertexes: vec![
                Vertex { x: 0, y: 0 },
                Vertex { x: 64, y: 0 },
                Vertex { x: 64, y: 64 },
            ],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![],
            reject: Reject::parse_lump(&[0u8], 1)
                .unwrap_or_else(|_| Reject::parse_lump(&[0u8], 0).unwrap()),
            blockmap: Blockmap::parse_lump(&[
                0, 0, 0, 0, // origin
                1, 0, 1, 0, // x_count, y_count
                5, 0, // offset
                0, 0, // sentinel
                0xFF, 0xFF, // term
            ])
            .unwrap(),
        };

        let svg = SvgExporter::export(&level);

        // Verify the basic SVG structure and content.
        assert!(svg.starts_with("<svg "), "Must start with svg tag");
        assert!(
            svg.contains("viewBox=\"0 0 64 64\""),
            "Must have correct viewBox"
        );
        assert!(svg.contains("scale(1, -1)"), "Must flip Y axis");
        assert!(
            svg.contains("<line x1=\"0\" y1=\"0\" x2=\"64\" y2=\"0\""),
            "Must contain first linedef"
        );
        assert!(
            svg.contains("<line x1=\"64\" y1=\"0\" x2=\"64\" y2=\"64\""),
            "Must contain second linedef"
        );
    }
}
