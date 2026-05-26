//! Export map geometry and statistics to a standalone HTML report.
//!
//! This module provides the `export_map_to_html` function, which creates a rich,
//! self-contained HTML dashboard for a Doom level, embedding the SVG layout and
//! summarizing geometry and entity metrics.

use crate::{Level, export_map_to_svg};

/// Exports a `Level` to a self-contained HTML report.
///
/// This generates a single HTML file containing an interactive visual
/// representation of the map (using embedded SVG) alongside a dashboard
/// of statistics about the map's geometry, entities, and environment.
///
/// ## Examples
///
/// ```
/// use doom_map::Level;
/// use doom_map::lumps::{Blockmap, Reject, Sector, Vertex};
///
/// let mut level = Level {
///     name: "TEST".to_owned(),
///     things: vec![], linedefs: vec![], sidedefs: vec![],
///     vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 64, y: 0 }],
///     segs: vec![], ssectors: vec![], nodes: vec![],
///     sectors: vec![Sector {
///         floor_height: 0, ceil_height: 128, floor_flat: *b"FLAT1\0\0\0",
///         ceil_flat: *b"FLAT2\0\0\0", light_level: 192, special: 0, tag: 0
///     }],
///     reject: Reject::parse_lump(&[], 0).unwrap(),
///     blockmap: Blockmap::parse_lump(&[0, 0, 0, 0, 0, 0, 0, 0]).unwrap(),
/// };
///
/// let html = doom_map::export_map_to_html(&level);
/// assert!(html.contains("<!DOCTYPE html>"));
/// assert!(html.contains("Map Report: TEST"));
/// assert!(html.contains("64 x 0"));
/// ```
pub fn export_map_to_html(level: &Level) -> String {
    let svg = export_map_to_svg(level);

    let num_vertexes = level.vertexes.len();
    let num_linedefs = level.linedefs.len();
    let num_sidedefs = level.sidedefs.len();
    let num_sectors = level.sectors.len();
    let num_things = level.things.len();

    let mut min_light = 255;
    let mut max_light = 0;
    for sector in &level.sectors {
        min_light = min_light.min(sector.light_level);
        max_light = max_light.max(sector.light_level);
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

    let width = (max_x as i32).saturating_sub(min_x as i32);
    let height = (max_y as i32).saturating_sub(min_y as i32);

    let dim_str = if level.vertexes.is_empty() {
        "0 x 0".to_string()
    } else {
        format!("{width} x {height}")
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Doom Map Report: {name}</title>
    <style>
        body {{ font-family: system-ui, -apple-system, sans-serif; background: #111; color: #eee; margin: 0; padding: 20px; }}
        h1 {{ color: #f55; border-bottom: 2px solid #333; padding-bottom: 10px; }}
        .container {{ display: flex; flex-wrap: wrap; gap: 20px; }}
        .svg-container {{ flex: 1 1 600px; background: #222; padding: 10px; border-radius: 8px; border: 1px solid #333; }}
        .svg-container svg {{ width: 100%; height: auto; max-height: 80vh; }}
        .stats-container {{ flex: 1 1 300px; display: flex; flex-direction: column; gap: 15px; }}
        .card {{ background: #222; padding: 15px; border-radius: 8px; border: 1px solid #333; }}
        .card h2 {{ margin-top: 0; color: #88f; font-size: 1.2em; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ text-align: left; padding: 8px; border-bottom: 1px solid #333; }}
        th {{ color: #aaa; font-weight: normal; }}
        td {{ font-weight: bold; text-align: right; }}
    </style>
</head>
<body>
    <h1>Map Report: {name}</h1>
    <div class="container">
        <div class="svg-container">
            {svg}
        </div>
        <div class="stats-container">
            <div class="card">
                <h2>Geometry</h2>
                <table>
                    <tr><th>Dimensions</th><td>{dim}</td></tr>
                    <tr><th>Vertices</th><td>{v}</td></tr>
                    <tr><th>Linedefs</th><td>{l}</td></tr>
                    <tr><th>Sidedefs</th><td>{sd}</td></tr>
                    <tr><th>Sectors</th><td>{s}</td></tr>
                </table>
            </div>
            <div class="card">
                <h2>Entities</h2>
                <table>
                    <tr><th>Total Things</th><td>{t}</td></tr>
                </table>
            </div>
            <div class="card">
                <h2>Environment</h2>
                <table>
                    <tr><th>Min Light Level</th><td>{min_l}</td></tr>
                    <tr><th>Max Light Level</th><td>{max_l}</td></tr>
                </table>
            </div>
        </div>
    </div>
</body>
</html>"#,
        name = level.name,
        svg = svg,
        dim = dim_str,
        v = num_vertexes,
        l = num_linedefs,
        sd = num_sidedefs,
        s = num_sectors,
        t = num_things,
        min_l = min_light,
        max_l = max_light
    )
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
    fn test_export_html() {
        let level = make_test_level();
        let html = export_map_to_html(&level);

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<svg viewBox="));
        assert!(html.contains("Map Report: TEST"));
        assert!(html.contains("<th>Dimensions</th><td>64 x 0</td>"));
        assert!(html.contains("<th>Vertices</th><td>2</td>"));
        assert!(html.contains("<th>Linedefs</th><td>1</td>"));
    }
}
