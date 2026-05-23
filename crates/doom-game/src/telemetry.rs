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
    ///
    /// ⚡ Bolt Optimization:
    /// Pre-allocates a single string buffer and formats directly into it,
    /// eliminating intermediate `Vec<String>` allocations and `.join()` overhead.
    #[must_use]
    pub fn export_to_geojson(&self) -> String {
        use std::fmt::Write;

        // Estimate capacity to avoid reallocations:
        // ~25 bytes per path coordinate, ~200 bytes per event point
        let mut capacity = 256;
        let mut path_count = 0;
        for ev in &self.events {
            if ev.kind == TelemetryKind::Position {
                capacity += 25;
                path_count += 1;
            } else {
                capacity += 200;
            }
        }

        let mut out = String::with_capacity(capacity);
        out.push_str("{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n");

        let mut first_feature = true;

        if path_count > 0 {
            out.push_str("    {\n      \"type\": \"Feature\",\n      \"geometry\": {\n        \"type\": \"LineString\",\n        \"coordinates\": [");
            let mut first_coord = true;
            for ev in &self.events {
                if ev.kind == TelemetryKind::Position {
                    if !first_coord {
                        out.push_str(", ");
                    }
                    let _ = write!(&mut out, "[{}, {}]", ev.x, ev.y);
                    first_coord = false;
                }
            }
            out.push_str("]\n      },\n      \"properties\": {\n        \"name\": \"Player Path\"\n      }\n    }");
            first_feature = false;
        }

        for ev in &self.events {
            if ev.kind != TelemetryKind::Position {
                if !first_feature {
                    out.push_str(",\n");
                }

                let kind_str = match &ev.kind {
                    TelemetryKind::ItemPickup(_) => "ItemPickup",
                    TelemetryKind::MonsterKill(_) => "MonsterKill",
                    TelemetryKind::DamageTaken(_) => "DamageTaken",
                    TelemetryKind::Position => unreachable!(),
                };

                let _ = write!(
                    &mut out,
                    "    {{\n      \"type\": \"Feature\",\n      \"geometry\": {{\n        \"type\": \"Point\",\n        \"coordinates\": [{}, {}]\n      }},\n      \"properties\": {{\n        \"tic\": {},\n        \"kind\": \"{}\",\n        \"description\": \"{}\"\n      }}\n    }}",
                    ev.x, ev.y, ev.tic, kind_str, ev.kind
                );
                first_feature = false;
            }
        }

        out.push_str("\n  ]\n}");
        out
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
