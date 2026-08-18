//! Achievement tracking system.
//!
//! Evaluates player performance against various predefined goals.

use crate::stats::LevelStats;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Achievement {
    Pacifist,
    Completionist,
    Speedrunner,
}

pub struct AchievementTracker;

impl AchievementTracker {
    #[must_use]
    pub fn check_achievements(stats: &LevelStats, par_time_tics: u32) -> Vec<Achievement> {
        let mut unlocked = Vec::new();

        if stats.level_time > 0 {
            if stats.kill_count == 0 && stats.total_kills > 0 {
                unlocked.push(Achievement::Pacifist);
            }

            if stats.kill_count >= stats.total_kills
                && stats.item_count >= stats.total_items
                && stats.secret_count >= stats.total_secrets
                && (stats.total_kills > 0 || stats.total_items > 0 || stats.total_secrets > 0)
            {
                unlocked.push(Achievement::Completionist);
            }

            if stats.level_time <= par_time_tics {
                unlocked.push(Achievement::Speedrunner);
            }
        }

        unlocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::LevelStats;

    #[test]
    fn test_pacifist_achievement() {
        let stats = LevelStats {
            kill_count: 0,
            total_kills: 10,
            level_time: 1000,
            ..Default::default()
        };
        let achs = AchievementTracker::check_achievements(&stats, 2000);
        assert!(achs.contains(&Achievement::Pacifist));
        assert!(achs.contains(&Achievement::Speedrunner));
        assert!(!achs.contains(&Achievement::Completionist));
    }

    #[test]
    fn test_completionist_achievement() {
        let stats = LevelStats {
            kill_count: 10,
            total_kills: 10,
            item_count: 5,
            total_items: 5,
            secret_count: 1,
            total_secrets: 1,
            level_time: 3000,
        };
        let achs = AchievementTracker::check_achievements(&stats, 2000);
        assert!(achs.contains(&Achievement::Completionist));
        assert!(!achs.contains(&Achievement::Pacifist));
        assert!(!achs.contains(&Achievement::Speedrunner));
    }
}
