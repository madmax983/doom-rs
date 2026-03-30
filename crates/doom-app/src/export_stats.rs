use doom_game::intermission::IntermissionStats;

/// Exports a `IntermissionStats` to a JSON string.
///
/// This provides a headless way to extract map metadata (totals) without
/// requiring external serialization crates.
pub fn export_map_stats_to_json(map_name: &str, stats: &IntermissionStats) -> String {
    format!(
        r#"{{
  "map": "{}",
  "total_kills": {},
  "total_items": {},
  "total_secrets": {},
  "par_time_tics": {}
}}"#,
        map_name, stats.total_kills, stats.total_items, stats.total_secrets, stats.par_time_tics
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_map_stats_to_json() {
        let stats = IntermissionStats {
            kills: 0,
            total_kills: 142,
            items: 0,
            total_items: 25,
            secrets: 0,
            total_secrets: 3,
            time_tics: 0,
            par_time_tics: 1050,
        };

        let json = export_map_stats_to_json("E1M1", &stats);

        assert!(json.contains(r#""map": "E1M1""#));
        assert!(json.contains(r#""total_kills": 142"#));
        assert!(json.contains(r#""total_items": 25"#));
        assert!(json.contains(r#""total_secrets": 3"#));
        assert!(json.contains(r#""par_time_tics": 1050"#));
    }
}
