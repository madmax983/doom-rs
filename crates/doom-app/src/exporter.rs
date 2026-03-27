//! Export map statistics and interactive dashboard to an HTML file.

use doom_game::{GameState, mobj::flags};
use doom_map::Level;

/// Exports a self-contained HTML level dashboard.
///
/// This dashboard includes:
/// - Level name.
/// - Core geometry counts (sectors, linedefs, vertexes, things).
/// - An embedded interactive minimap (SVG).
/// - Enemy and Item counts.
pub fn export_html_dashboard(level: &Level, gs: &GameState) -> String {
    let svg_content = doom_map::export_map_to_svg(level);

    let mut enemy_count = 0;
    let mut item_count = 0;
    let mut secret_count = 0; // if we want to extract it

    // Count enemies and items by walking the GameState mobj slab
    for handle in gs.mobjslab.iter_handles() {
        if let Some(mo) = gs.mobjslab.get(handle) {
            if mo.flags & flags::MF_COUNTKILL != 0 {
                enemy_count += 1;
            }
            if mo.flags & flags::MF_COUNTITEM != 0 {
                item_count += 1;
            }
        }
    }

    // Count map secrets:
    for sector in &level.sectors {
        if sector.special == 9 {
            secret_count += 1;
        }
    }

    let sector_count = level.sectors.len();
    let linedef_count = level.linedefs.len();
    let vertex_count = level.vertexes.len();
    let thing_count = level.things.len();

    let title = format!("Doom-RS Dashboard: {}", level.name);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{title}</title>
    <style>
        body {{
            background-color: #1e1e1e;
            color: #c0c0c0;
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            margin: 0;
            padding: 20px;
            display: flex;
            flex-direction: column;
            align-items: center;
        }}
        h1 {{
            color: #ff5555;
            text-shadow: 2px 2px #000;
        }}
        .dashboard {{
            display: flex;
            flex-wrap: wrap;
            gap: 20px;
            max-width: 1200px;
            width: 100%;
        }}
        .stats-panel, .map-panel {{
            background-color: #2d2d2d;
            border: 1px solid #444;
            border-radius: 8px;
            padding: 20px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.5);
        }}
        .stats-panel {{
            flex: 1;
            min-width: 300px;
        }}
        .map-panel {{
            flex: 2;
            min-width: 400px;
            display: flex;
            justify-content: center;
            align-items: center;
            overflow: hidden;
            resize: both;
        }}
        .stat-group {{
            margin-bottom: 20px;
        }}
        .stat-group h2 {{
            border-bottom: 1px solid #555;
            padding-bottom: 5px;
            margin-top: 0;
            color: #55ff55;
        }}
        .stat-row {{
            display: flex;
            justify-content: space-between;
            margin: 5px 0;
            font-size: 1.1em;
        }}
        .stat-label {{
            font-weight: bold;
        }}
        .stat-value {{
            color: #fff;
        }}
        svg {{
            max-width: 100%;
            max-height: 800px;
            border: 1px solid #000;
            border-radius: 4px;
        }}
    </style>
</head>
<body>
    <h1>{title}</h1>
    <div class="dashboard">
        <div class="stats-panel">
            <div class="stat-group">
                <h2>Level Geometry</h2>
                <div class="stat-row"><span class="stat-label">Sectors:</span><span class="stat-value">{sector_count}</span></div>
                <div class="stat-row"><span class="stat-label">Linedefs:</span><span class="stat-value">{linedef_count}</span></div>
                <div class="stat-row"><span class="stat-label">Vertexes:</span><span class="stat-value">{vertex_count}</span></div>
            </div>
            <div class="stat-group">
                <h2>Gameplay Entities</h2>
                <div class="stat-row"><span class="stat-label">Total Map Things:</span><span class="stat-value">{thing_count}</span></div>
                <div class="stat-row"><span class="stat-label">Live Enemies:</span><span class="stat-value">{enemy_count}</span></div>
                <div class="stat-row"><span class="stat-label">Live Items:</span><span class="stat-value">{item_count}</span></div>
                <div class="stat-row"><span class="stat-label">Map Secrets:</span><span class="stat-value">{secret_count}</span></div>
            </div>
        </div>
        <div class="map-panel">
            {svg_content}
        </div>
    </div>
</body>
</html>"#,
        title = title,
        sector_count = sector_count,
        linedef_count = linedef_count,
        vertex_count = vertex_count,
        thing_count = thing_count,
        enemy_count = enemy_count,
        item_count = item_count,
        secret_count = secret_count,
        svg_content = svg_content
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_game::{GameState, Mobj, MobjKind, flags};
    use doom_types::{Fixed16_16, Bam};
    use doom_map::lumps::{Sector, Blockmap, Reject};

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
            name: "E1M1".to_owned(),
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
                special: 9, // A secret sector
                tag: 0,
            }],
            reject,
            blockmap,
        }
    }

    #[test]
    fn test_export_html_dashboard() {
        let level = make_test_level();
        let mut gs = GameState::new("E1M1");

        // Add a killable monster
        let mut imp = Mobj::new(MobjKind::Imp, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        imp.flags |= flags::MF_COUNTKILL;
        gs.mobjslab.alloc(imp);

        // Add an item
        let mut medikit = Mobj::new(MobjKind::Medikit, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        medikit.flags |= flags::MF_COUNTITEM;
        gs.mobjslab.alloc(medikit);

        let html = export_html_dashboard(&level, &gs);

        // Validate basic structure
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Doom-RS Dashboard: E1M1"));
        assert!(html.contains("<svg viewBox="));

        // Validate stats
        assert!(html.contains(r#"<span class="stat-value">1</span></div>"#.trim())); // We have some counts = 1
        assert!(html.contains("Live Enemies:</span><span class=\"stat-value\">1</span>"));
        assert!(html.contains("Live Items:</span><span class=\"stat-value\">1</span>"));
        assert!(html.contains("Map Secrets:</span><span class=\"stat-value\">1</span>"));
        assert!(html.contains("Sectors:</span><span class=\"stat-value\">1</span>"));
    }
}
