//! ASCII Heatmap generation from session telemetry.
//!
//! Provides an immediate visual representation of player activity hotspots
//! directly in the terminal, without requiring external GIS tools.

use crate::telemetry::{SessionTelemetry, TelemetryKind};
use std::collections::HashMap;

/// The metric to visualize in the heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatmapMetric {
    /// Measures how much time the player spent in a cell.
    Traffic,
    /// Measures how much damage the player took in a cell.
    Damage,
}

/// Generates an ASCII spatial heatmap from `SessionTelemetry`.
pub struct TelemetryHeatmap {
    /// The size of each grid cell in world units (e.g., 64).
    pub cell_size: u32,
    /// Which metric to visualize.
    pub metric: HeatmapMetric,
}

impl TelemetryHeatmap {
    /// Creates a new heatmap generator.
    #[must_use]
    pub fn new(cell_size: u32, metric: HeatmapMetric) -> Self {
        Self { cell_size, metric }
    }

    /// Generates the ASCII string representing the heatmap.
    #[must_use]
    pub fn generate_ascii(&self, telemetry: &SessionTelemetry) -> String {
        if telemetry.events.is_empty() {
            return String::from("(no telemetry data)");
        }

        let mut grid: HashMap<(i32, i32), u32> = HashMap::new();
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;

        for ev in &telemetry.events {
            let value = match (self.metric, &ev.kind) {
                (HeatmapMetric::Traffic, TelemetryKind::Position) => 1,
                (HeatmapMetric::Damage, TelemetryKind::DamageTaken(amt)) => *amt,
                _ => 0,
            };

            if value > 0 {
                let cx = ev.x.div_euclid(self.cell_size as i32);
                let cy = ev.y.div_euclid(self.cell_size as i32);

                *grid.entry((cx, cy)).or_default() += value;

                min_x = min_x.min(cx);
                max_x = max_x.max(cx);
                min_y = min_y.min(cy);
                max_y = max_y.max(cy);
            }
        }

        if grid.is_empty() {
            return String::from("(no relevant events found)");
        }

        let max_val = grid.values().copied().max().unwrap_or(1) as f32;
        let chars = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
        let mut out = String::new();

        for y in (min_y..=max_y).rev() {
            for x in min_x..=max_x {
                if let Some(&val) = grid.get(&(x, y)) {
                    let ratio = val as f32 / max_val;
                    let idx = (ratio * (chars.len() - 1) as f32).round() as usize;
                    let idx = idx.clamp(1, chars.len() - 1);
                    out.push(chars[idx]);
                } else {
                    out.push(chars[0]);
                }
            }
            out.push('\n');
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heatmap_empty() {
        let heatmap = TelemetryHeatmap::new(64, HeatmapMetric::Traffic);
        let telemetry = SessionTelemetry::new();
        assert_eq!(heatmap.generate_ascii(&telemetry), "(no telemetry data)");
    }

    #[test]
    fn test_heatmap_traffic() {
        let mut t = SessionTelemetry::new();
        t.record(1, 10, 10, TelemetryKind::Position);
        t.record(2, 20, 20, TelemetryKind::Position);
        t.record(3, 80, 10, TelemetryKind::Position);

        let heatmap = TelemetryHeatmap::new(64, HeatmapMetric::Traffic);
        let out = heatmap.generate_ascii(&t);

        assert!(out.contains("@+"));
    }

    #[test]
    fn test_heatmap_damage() {
        let mut t = SessionTelemetry::new();
        t.record(1, 10, 10, TelemetryKind::DamageTaken(50));
        t.record(2, 10, 10, TelemetryKind::DamageTaken(50));
        t.record(3, 80, 10, TelemetryKind::DamageTaken(20));

        let heatmap = TelemetryHeatmap::new(64, HeatmapMetric::Damage);
        let out = heatmap.generate_ascii(&t);

        assert!(out.contains("@:"));
    }
}
