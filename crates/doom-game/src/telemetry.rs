//! Player spatial telemetry tracking and GeoJSON export.
//!
//! Records player movements and major events (kills, item pickups, damage)
//! to allow generating spatial heatmaps and session analysis.

use std::fmt;

/// Types of trackable telemetry events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryKind {
    /// The player moved to a new position.
    Position,
    /// The player picked up an item.
    ItemPickup(String),
    /// The player killed a monster.
    MonsterKill(String),
    /// The player took damage.
    DamageTaken(u32),
}

impl fmt::Display for TelemetryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TelemetryKind::Position => write!(f, "Position"),
            TelemetryKind::ItemPickup(s) => write!(f, "ItemPickup: {}", s),
            TelemetryKind::MonsterKill(s) => write!(f, "MonsterKill: {}", s),
            TelemetryKind::DamageTaken(a) => write!(f, "DamageTaken: {}", a),
        }
    }
}

/// A single tracked event at a specific time and space.
#[derive(Debug, Clone)]
pub struct TelemetryEvent {
    /// The game tic when the event occurred.
    pub tic: u32,
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
    /// The type of event.
    pub kind: TelemetryKind,
}

/// Tracks an entire play session.
#[derive(Debug, Clone, Default)]
pub struct SessionTelemetry {
    /// Ordered list of events.
    pub events: Vec<TelemetryEvent>,
}

impl SessionTelemetry {
    /// Create a new empty telemetry tracker.
    #[must_use]
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Register a new event.
    pub fn record(&mut self, tic: u32, x: i32, y: i32, kind: TelemetryKind) {
        self.events.push(TelemetryEvent { tic, x, y, kind });
    }

    /// Exports the telemetry data as a GeoJSON FeatureCollection string.
    /// Player path is exported as a LineString, and significant events as Points.
    #[must_use]
    pub fn export_to_geojson(&self) -> String {
        let mut features = Vec::new();
        let mut path_coords = Vec::new();
        let mut event_features = Vec::new();

        for ev in &self.events {
            if ev.kind == TelemetryKind::Position {
                path_coords.push(format!("[{}, {}]", ev.x, ev.y));
            } else {
                let kind_str = match &ev.kind {
                    TelemetryKind::ItemPickup(_) => "ItemPickup",
                    TelemetryKind::MonsterKill(_) => "MonsterKill",
                    TelemetryKind::DamageTaken(_) => "DamageTaken",
                    TelemetryKind::Position => unreachable!(),
                };

                let desc = format!("{}", ev.kind);

                let feat = format!(
                    r#"    {{
      "type": "Feature",
      "geometry": {{
        "type": "Point",
        "coordinates": [{}, {}]
      }},
      "properties": {{
        "tic": {},
        "kind": "{}",
        "description": "{}"
      }}
    }}"#,
                    ev.x, ev.y, ev.tic, kind_str, desc
                );
                event_features.push(feat);
            }
        }

        if !path_coords.is_empty() {
            let path_feat = format!(
                r#"    {{
      "type": "Feature",
      "geometry": {{
        "type": "LineString",
        "coordinates": [{}]
      }},
      "properties": {{
        "name": "Player Path"
      }}
    }}"#,
                path_coords.join(", ")
            );
            features.push(path_feat);
        }

        features.extend(event_features);
        let features_str = features.join(",\n");

        format!(
            r#"{{
  "type": "FeatureCollection",
  "features": [
{}
  ]
}}"#,
            features_str
        )
    }

    /// Exports the telemetry data as an HTML dashboard overlaying the SVG map.
    #[must_use]
    pub fn export_telemetry_html(&self, level: &doom_map::Level) -> String {
        let mut base_svg = doom_map::svg::export_map_to_svg(level);

        let mut path_coords = String::new();
        let mut event_circles = String::new();

        for ev in &self.events {
            if ev.kind == TelemetryKind::Position {
                if !path_coords.is_empty() {
                    path_coords.push(' ');
                }
                path_coords.push_str(&format!("{},{}", ev.x, ev.y));
            } else {
                let color = match &ev.kind {
                    TelemetryKind::ItemPickup(_) => "#0f0",
                    TelemetryKind::MonsterKill(_) => "#f00",
                    TelemetryKind::DamageTaken(_) => "#fa0",
                    TelemetryKind::Position => unreachable!(),
                };
                event_circles.push_str(&format!(
                    r#"<circle cx="{}" cy="{}" r="8" fill="{}" />"#,
                    ev.x, ev.y, color
                ));
            }
        }

        let overlay = format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#00ffff\" stroke-width=\"2\" stroke-opacity=\"0.8\" />{}",
            path_coords, event_circles
        );

        base_svg = base_svg.replace("</g>\n</svg>\n", &format!("{}\n</g>\n</svg>\n", overlay));

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Telemetry Map Overlay: {}</title>
    <style>
        body {{ background: #111; color: #fff; font-family: sans-serif; }}
        svg {{ max-width: 100%; height: auto; }}
    </style>
</head>
<body>
    <h1>Telemetry Overlay: {}</h1>
    <div>{}</div>
</body>
</html>"#,
            level.name, level.name, base_svg
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_recording() {
        let mut t = SessionTelemetry::new();
        t.record(0, 10, 20, TelemetryKind::Position);
        t.record(5, 15, 25, TelemetryKind::ItemPickup("Shotgun".into()));

        assert_eq!(t.events.len(), 2);
        assert_eq!(t.events[0].x, 10);
    }

    #[test]
    fn test_geojson_export() {
        let mut t = SessionTelemetry::new();
        t.record(0, 0, 0, TelemetryKind::Position);
        t.record(1, 10, 10, TelemetryKind::Position);
        t.record(2, 10, 10, TelemetryKind::DamageTaken(15));

        let json = t.export_to_geojson();
        assert!(json.contains(r#""type": "FeatureCollection""#));
        assert!(json.contains(r#""type": "LineString""#));
        assert!(json.contains(r#""type": "Point""#));
        assert!(json.contains("[0, 0], [10, 10]"));
        assert!(json.contains("DamageTaken: 15"));
    }

    #[test]
    fn test_export_telemetry_html() {
        let mut t = SessionTelemetry::new();
        t.record(0, 0, 0, TelemetryKind::Position);
        t.record(1, 10, 10, TelemetryKind::DamageTaken(15));

        let level = doom_map::Level {
            name: "TEST".to_owned(),
            things: vec![],
            linedefs: vec![],
            sidedefs: vec![],
            vertexes: vec![],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![],
            reject: doom_map::lumps::Reject::parse_lump(&[0u8], 1).unwrap(),
            blockmap: doom_map::lumps::Blockmap::parse_lump(&[0u8; 14]).unwrap(),
        };

        let html = t.export_telemetry_html(&level);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<svg"));
        assert!(html.contains("<polyline points=\"0,0\""));
        assert!(html.contains("<circle cx=\"10\" cy=\"10\""));
    }
}
