//! SVG Telemetry Heatmap generation.

use crate::telemetry::{SessionTelemetry, TelemetryKind};
use doom_map::Level;

/// Overlays a session's telemetry data onto the map's SVG representation.
pub fn export_telemetry_heatmap_svg(level: &Level, telemetry: &SessionTelemetry) -> String {
    let mut base_svg = doom_map::export_map_to_svg(level);

    // Insert telemetry data just before the closing </g> tag.
    let closing_g = "</g>\n</svg>\n";
    if let Some(pos) = base_svg.rfind(closing_g) {
        let mut overlay = String::new();

        // Draw the player's path as a connected line.
        let mut path_coords = Vec::new();
        for ev in &telemetry.events {
            if ev.kind == TelemetryKind::Position {
                path_coords.push(format!("{},{}", ev.x, ev.y));
            }
        }
        if !path_coords.is_empty() {
            let points = path_coords.join(" ");
            overlay.push_str(&format!(
                "<polyline points=\"{}\" fill=\"none\" stroke=\"#0f0\" stroke-width=\"2\" opacity=\"0.5\" />\n",
                points
            ));
        }

        // Draw events as colored circles.
        for ev in &telemetry.events {
            match &ev.kind {
                TelemetryKind::MonsterKill(m) => {
                    overlay.push_str(&format!(
                        "<circle cx=\"{}\" cy=\"{}\" r=\"8\" fill=\"#ff0\" opacity=\"0.8\"><title>MonsterKill: {}</title></circle>\n",
                        ev.x, ev.y, m
                    ));
                }
                TelemetryKind::DamageTaken(d) => {
                    overlay.push_str(&format!(
                        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#f00\" opacity=\"0.8\"><title>DamageTaken: {}</title></circle>\n",
                        ev.x, ev.y, 4 + d / 10, d
                    ));
                }
                TelemetryKind::ItemPickup(i) => {
                    overlay.push_str(&format!(
                        "<circle cx=\"{}\" cy=\"{}\" r=\"6\" fill=\"#00f\" opacity=\"0.8\"><title>ItemPickup: {}</title></circle>\n",
                        ev.x, ev.y, i
                    ));
                }
                TelemetryKind::Position => {}
            }
        }

        base_svg.insert_str(pos, &overlay);
    }

    base_svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::TelemetryKind;
    use doom_map::lumps::{Blockmap, Reject, Vertex};

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
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 100, y: 100 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![],
            reject,
            blockmap,
        }
    }

    #[test]
    fn test_heatmap_svg() {
        let level = make_test_level();
        let mut telemetry = SessionTelemetry::new();
        telemetry.record(0, 10, 10, TelemetryKind::Position);
        telemetry.record(1, 20, 20, TelemetryKind::MonsterKill("Imp".into()));

        let svg = export_telemetry_heatmap_svg(&level, &telemetry);
        assert!(svg.contains("<svg viewBox="), "Must contain base map SVG");
        assert!(svg.contains("MonsterKill"), "Must contain telemetry data");
    }
}
