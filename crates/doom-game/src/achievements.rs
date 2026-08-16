//! Achievement tracking system.
//!
//! Monitors gameplay statistics and unlocks achievements when specific
//! conditions are met.

use crate::intermission::IntermissionStats;

/// Badges of honor earned by completing level-specific feats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Achievement {
    /// Beat the par time for a level.
    SpeedDemon,
    /// Complete a level with 0 kills.
    Pacifist,
    /// Discover 100% of the secrets in a level.
    Sleuth,
}

/// Tracks and manages unlocked achievements.
#[derive(Debug, Clone, Default)]
pub struct AchievementTracker {
    /// The list of achievements earned so far.
    pub unlocked: Vec<Achievement>,
}

impl AchievementTracker {
    /// Creates a fresh tracker with no achievements unlocked.
    pub fn new() -> Self {
        Self {
            unlocked: Vec::new(),
        }
    }

    /// Evaluates end-of-level stats and unlocks any earned achievements.
    pub fn evaluate_intermission(&mut self, stats: &IntermissionStats) {
        if stats.time_tics > 0
            && stats.par_time_tics > 0
            && stats.time_tics <= stats.par_time_tics
            && !self.unlocked.contains(&Achievement::SpeedDemon)
        {
            self.unlocked.push(Achievement::SpeedDemon);
        }
        if stats.kills == 0 && !self.unlocked.contains(&Achievement::Pacifist) {
            self.unlocked.push(Achievement::Pacifist);
        }
        if stats.total_secrets > 0
            && stats.secrets >= stats.total_secrets
            && !self.unlocked.contains(&Achievement::Sleuth)
        {
            self.unlocked.push(Achievement::Sleuth);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_demon_unlocks_under_par() {
        let mut tracker = AchievementTracker::new();
        let stats = IntermissionStats {
            time_tics: 100,
            par_time_tics: 200,
            ..Default::default()
        };
        tracker.evaluate_intermission(&stats);
        assert!(
            tracker.unlocked.contains(&Achievement::SpeedDemon),
            "Should unlock SpeedDemon"
        );
    }

    #[test]
    fn test_pacifist_unlocks_zero_kills() {
        let mut tracker = AchievementTracker::new();
        let stats = IntermissionStats {
            kills: 0,
            total_kills: 10,
            ..Default::default()
        };
        tracker.evaluate_intermission(&stats);
        assert!(
            tracker.unlocked.contains(&Achievement::Pacifist),
            "Should unlock Pacifist"
        );
    }

    #[test]
    fn test_sleuth_unlocks_all_secrets() {
        let mut tracker = AchievementTracker::new();
        let stats = IntermissionStats {
            secrets: 2,
            total_secrets: 2,
            ..Default::default()
        };
        tracker.evaluate_intermission(&stats);
        assert!(
            tracker.unlocked.contains(&Achievement::Sleuth),
            "Should unlock Sleuth"
        );
    }
}
