//! Achievement tracking and evaluation system.
//!
//! Evaluates the player's performance at the end of a level using `LevelStats`,
//! `SessionTelemetry`, and the style system to award gameplay achievements.

use crate::stats::LevelStats;
#[cfg(feature = "style_meter")]
use crate::style::StyleRank;
#[cfg(feature = "telemetry")]
use crate::telemetry::{SessionTelemetry, TelemetryKind};

/// Represents an unlocked achievement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Achievement {
    /// Complete the level without taking any damage.
    Untouchable,
    /// Complete the level without killing any monsters.
    Pacifist,
    /// Find all secrets and kill all monsters.
    Completionist,
    /// Reach the maximum style rank (Smokin' Sexy Style!!).
    Stylish,
    /// Complete the level in under a specific par time.
    SpeedDemon,
}

impl Achievement {
    /// Returns the human-readable name of the achievement.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Untouchable => "Untouchable",
            Self::Pacifist => "Pacifist",
            Self::Completionist => "Completionist",
            Self::Stylish => "Smokin' Sexy Style!!",
            Self::SpeedDemon => "Speed Demon",
        }
    }

    /// Returns a short description of how to unlock the achievement.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Untouchable => "Complete the level without taking any damage.",
            Self::Pacifist => "Complete the level without killing any monsters.",
            Self::Completionist => "Find all secrets and kill all monsters.",
            Self::Stylish => "Reach the maximum style rank.",
            Self::SpeedDemon => "Complete the level under par time.",
        }
    }
}

/// Evaluates player performance to award achievements.
#[derive(Debug)]
pub struct AchievementTracker;

impl AchievementTracker {
    /// Evaluates the current level performance and returns a list of unlocked achievements.
    #[must_use]
    pub fn evaluate(
        stats: &LevelStats,
        #[cfg(feature = "telemetry")] telemetry: &SessionTelemetry,
        #[cfg(feature = "style_meter")] max_style_reached: Option<StyleRank>,
        par_time_tics: u32,
    ) -> Vec<Achievement> {
        let mut achievements = Vec::new();

        // Check Untouchable: No damage taken events in telemetry
        #[cfg(feature = "telemetry")]
        {
            let took_damage = telemetry
                .events
                .iter()
                .any(|e| matches!(e.kind, TelemetryKind::DamageTaken(_)));

            if !took_damage {
                achievements.push(Achievement::Untouchable);
            }
        }

        // Check Pacifist: 0 kills
        if stats.kill_count == 0 {
            achievements.push(Achievement::Pacifist);
        }

        // Check Completionist: 100% kills and secrets (and more than 0 total)
        if stats.total_kills > 0
            && stats.kill_count >= stats.total_kills
            && stats.secret_count >= stats.total_secrets
        {
            achievements.push(Achievement::Completionist);
        }

        // Check Stylish
        #[cfg(feature = "style_meter")]
        {
            if let Some(StyleRank::SmokinSexyStyle) = max_style_reached {
                achievements.push(Achievement::Stylish);
            }
        }

        // Check SpeedDemon
        if stats.level_time > 0 && stats.level_time <= par_time_tics {
            achievements.push(Achievement::SpeedDemon);
        }

        achievements
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pacifist_achievement() {
        let stats = LevelStats {
            kill_count: 0,
            total_kills: 10,
            level_time: 5000,
            ..Default::default()
        };
        #[cfg(feature = "telemetry")]
        let telemetry = SessionTelemetry::new();

        #[allow(unused_variables)]
        let achievements = AchievementTracker::evaluate(
            &stats,
            #[cfg(feature = "telemetry")]
            &telemetry,
            #[cfg(feature = "style_meter")]
            None,
            1000,
        );
        assert!(achievements.contains(&Achievement::Pacifist));
        #[cfg(feature = "telemetry")]
        assert!(achievements.contains(&Achievement::Untouchable));
        assert!(!achievements.contains(&Achievement::SpeedDemon));
    }

    #[test]
    fn test_speed_demon_and_stylish() {
        let stats = LevelStats {
            kill_count: 5,
            total_kills: 10,
            level_time: 800,
            ..Default::default()
        };
        #[cfg(feature = "telemetry")]
        let telemetry = SessionTelemetry::new();

        let achievements = AchievementTracker::evaluate(
            &stats,
            #[cfg(feature = "telemetry")]
            &telemetry,
            #[cfg(feature = "style_meter")]
            Some(StyleRank::SmokinSexyStyle),
            1000,
        );
        assert!(achievements.contains(&Achievement::SpeedDemon));
        #[cfg(feature = "style_meter")]
        assert!(achievements.contains(&Achievement::Stylish));
        assert!(!achievements.contains(&Achievement::Completionist));
    }

    #[test]
    #[cfg(feature = "telemetry")]
    fn test_completionist_achievement() {
        let stats = LevelStats {
            kill_count: 10,
            total_kills: 10,
            secret_count: 2,
            total_secrets: 2,
            level_time: 2000,
            ..Default::default()
        };
        let mut telemetry = SessionTelemetry::new();
        telemetry.record(10, 0, 0, TelemetryKind::DamageTaken(5));

        let achievements = AchievementTracker::evaluate(
            &stats,
            &telemetry,
            #[cfg(feature = "style_meter")]
            None,
            1000,
        );
        assert!(achievements.contains(&Achievement::Completionist));
        assert!(!achievements.contains(&Achievement::Untouchable));
    }
}
