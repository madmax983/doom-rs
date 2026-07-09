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
            match &ev.kind {
                TelemetryKind::Position => {
                    path_coords.push(format!("[{}, {}]", ev.x, ev.y));
                }
                _ => {
                    let kind_str = match &ev.kind {
                        TelemetryKind::ItemPickup(_) => "ItemPickup",
                        TelemetryKind::MonsterKill(_) => "MonsterKill",
                        TelemetryKind::DamageTaken(_) => "DamageTaken",
                        _ => "Unknown", // Fallback replacing unreachable!()
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
}
