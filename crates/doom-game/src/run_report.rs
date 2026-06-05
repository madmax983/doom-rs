//! Exporting end-of-level statistics and full session telemetry into structured formats.
//!
//! Connecting `IntermissionStats` with the optional `SessionTelemetry` allows
//! players to export a complete, analytical breakdown of their map playthrough.

use crate::intermission::IntermissionStats;
#[cfg(feature = "telemetry")]
use crate::telemetry::SessionTelemetry;

/// A full report of a level playthrough.
#[derive(Debug, Clone)]
pub struct RunReport {
    /// The map name (e.g. "E1M1").
    pub level_name: String,
    /// Statistics captured at the map exit.
    pub stats: IntermissionStats,
    /// Optional spatial tracking data of the playthrough.
    #[cfg(feature = "telemetry")]
    pub telemetry: Option<SessionTelemetry>,
}

impl RunReport {
    /// Create a new report.
    pub fn new(level_name: String, stats: IntermissionStats) -> Self {
        Self {
            level_name,
            stats,
            #[cfg(feature = "telemetry")]
            telemetry: None,
        }
    }

    /// Attach session telemetry to the report.
    #[cfg(feature = "telemetry")]
    #[must_use]
    pub fn with_telemetry(mut self, telemetry: SessionTelemetry) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    /// Export the basic statistics as a CSV string.
    #[must_use]
    pub fn export_csv(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "Level,Kills,TotalKills,Items,TotalItems,Secrets,TotalSecrets,TimeTics,ParTimeTics\n",
        );
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            self.level_name,
            self.stats.kills,
            self.stats.total_kills,
            self.stats.items,
            self.stats.total_items,
            self.stats.secrets,
            self.stats.total_secrets,
            self.stats.time_tics,
            self.stats.par_time_tics
        ));
        out
    }

    /// Export the report as a JSON string.
    #[must_use]
    pub fn export_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str(&format!("  \"level\": \"{}\",\n", self.level_name));
        out.push_str("  \"stats\": {\n");
        out.push_str(&format!("    \"kills\": {},\n", self.stats.kills));
        out.push_str(&format!(
            "    \"total_kills\": {},\n",
            self.stats.total_kills
        ));
        out.push_str(&format!("    \"items\": {},\n", self.stats.items));
        out.push_str(&format!(
            "    \"total_items\": {},\n",
            self.stats.total_items
        ));
        out.push_str(&format!("    \"secrets\": {},\n", self.stats.secrets));
        out.push_str(&format!(
            "    \"total_secrets\": {},\n",
            self.stats.total_secrets
        ));
        out.push_str(&format!("    \"time_tics\": {},\n", self.stats.time_tics));
        out.push_str(&format!(
            "    \"par_time_tics\": {}\n",
            self.stats.par_time_tics
        ));
        out.push_str("  }");

        #[cfg(feature = "telemetry")]
        if let Some(ref tel) = self.telemetry {
            out.push_str(",\n  \"telemetry\": ");
            // We just embed the GeoJSON
            let geo_json = tel.export_to_geojson();
            // GeoJSON string might need indentation if we wanted it pretty,
            // but for now just append it directly
            let indented: String = geo_json
                .lines()
                .map(|l| format!("  {}", l))
                .collect::<Vec<_>>()
                .join("\n");
            out.push_str(&indented);
        }

        out.push_str("\n}\n");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermission::IntermissionStats;
    #[cfg(feature = "telemetry")]
    use crate::telemetry::{SessionTelemetry, TelemetryKind};

    fn make_stats() -> IntermissionStats {
        IntermissionStats {
            kills: 10,
            total_kills: 20,
            items: 5,
            total_items: 15,
            secrets: 1,
            total_secrets: 3,
            time_tics: 1050,
            par_time_tics: 1050,
        }
    }

    #[test]
    fn test_export_csv() {
        let rep = RunReport::new("E1M1".to_string(), make_stats());
        let csv = rep.export_csv();
        assert!(csv.starts_with("Level,Kills"));
        assert!(csv.contains("E1M1,10,20,5,15,1,3,1050,1050"));
    }

    #[test]
    fn test_export_json_basic() {
        let rep = RunReport::new("E1M1".to_string(), make_stats());
        let json = rep.export_json();
        assert!(json.contains("\"level\": \"E1M1\""));
        assert!(json.contains("\"kills\": 10"));
        assert!(json.contains("\"par_time_tics\": 1050"));
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn test_export_json_with_telemetry() {
        let mut tel = SessionTelemetry::new();
        tel.record(0, 0, 0, TelemetryKind::Position);

        let rep = RunReport::new("E1M1".to_string(), make_stats()).with_telemetry(tel);
        let json = rep.export_json();
        assert!(json.contains("\"telemetry\":"));
        assert!(json.contains("\"FeatureCollection\""));
    }
}
