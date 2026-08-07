//! End-of-level Awards Mashup.
//!
//! This module combines `SessionTelemetry` and `LevelStats` to grant
//! achievements/awards to the player at the end of a level.

use crate::stats::LevelStats;
use crate::telemetry::{SessionTelemetry, TelemetryKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Award {
    /// Took 0 damage.
    Untouchable,
    /// Picked up 0 items but finished the level.
    Minimalist,
    /// Killed every monster.
    DoomSlayer,
    /// Took over 200 damage.
    BulletSponge,
}

pub struct AwardCeremony;

impl AwardCeremony {
    pub fn evaluate(stats: &LevelStats, telemetry: &SessionTelemetry) -> Vec<Award> {
        let mut awards = Vec::new();

        let mut total_damage = 0;
        for event in &telemetry.events {
            if let TelemetryKind::DamageTaken(damage) = event.kind {
                total_damage += damage;
            }
        }

        if total_damage == 0 {
            awards.push(Award::Untouchable);
        } else if total_damage > 200 {
            awards.push(Award::BulletSponge);
        }

        if stats.item_count == 0 {
            awards.push(Award::Minimalist);
        }

        if stats.kill_count > 0 && stats.kill_count >= stats.total_kills {
            awards.push(Award::DoomSlayer);
        }

        awards
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::TelemetryEvent;

    #[test]
    fn test_untouchable_and_minimalist() {
        let stats = LevelStats {
            kill_count: 5,
            total_kills: 10,
            item_count: 0,
            ..Default::default()
        };
        let telemetry = SessionTelemetry::new();

        let awards = AwardCeremony::evaluate(&stats, &telemetry);
        assert!(awards.contains(&Award::Untouchable));
        assert!(awards.contains(&Award::Minimalist));
        assert!(!awards.contains(&Award::DoomSlayer));
        assert!(!awards.contains(&Award::BulletSponge));
    }

    #[test]
    fn test_bullet_sponge_and_doom_slayer() {
        let stats = LevelStats {
            kill_count: 10,
            total_kills: 10,
            item_count: 5,
            ..Default::default()
        };
        let mut telemetry = SessionTelemetry::new();
        telemetry.record(10, 0, 0, TelemetryKind::DamageTaken(250));

        let awards = AwardCeremony::evaluate(&stats, &telemetry);
        assert!(!awards.contains(&Award::Untouchable));
        assert!(!awards.contains(&Award::Minimalist));
        assert!(awards.contains(&Award::DoomSlayer));
        assert!(awards.contains(&Award::BulletSponge));
    }
}
