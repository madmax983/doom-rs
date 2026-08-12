//! Achievement tracking system.
//!
//! Evaluates `LevelStats` and other game metrics to unlock milestones.

use crate::stats::LevelStats;

/// Types of achievements that can be unlocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Achievement {
    /// Kill a single enemy.
    FirstBlood,
    /// Find 1 secret.
    Sleuth,
    /// Find all secrets in the map.
    MasterSleuth,
    /// Kill 50 enemies.
    Slayer,
    /// Collect 10 items.
    Hoarder,
}

/// Tracks unlocked achievements using a bitmask for O(1) deterministic state.
#[derive(Debug, Clone, Default)]
pub struct AchievementTracker {
    unlocked_mask: u32,
}

impl AchievementTracker {
    /// Creates a new, empty achievement tracker.
    #[must_use]
    pub fn new() -> Self {
        Self { unlocked_mask: 0 }
    }

    /// Checks if a specific achievement is unlocked.
    #[must_use]
    pub fn is_unlocked(&self, achievement: Achievement) -> bool {
        let bit = achievement as u32;
        (self.unlocked_mask & (1 << bit)) != 0
    }

    /// Evaluates current game stats and unlocks achievements if criteria are met.
    pub fn evaluate_stats(&mut self, stats: &LevelStats) {
        if stats.kill_count >= 1 {
            self.unlock(Achievement::FirstBlood);
        }
        if stats.kill_count >= 50 {
            self.unlock(Achievement::Slayer);
        }
        if stats.secret_count >= 1 {
            self.unlock(Achievement::Sleuth);
        }
        if stats.total_secrets > 0 && stats.secret_count >= stats.total_secrets {
            self.unlock(Achievement::MasterSleuth);
        }
        if stats.item_count >= 10 {
            self.unlock(Achievement::Hoarder);
        }
    }

    fn unlock(&mut self, achievement: Achievement) {
        let bit = achievement as u32;
        self.unlocked_mask |= 1 << bit;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_empty() {
        let tracker = AchievementTracker::new();
        assert!(!tracker.is_unlocked(Achievement::FirstBlood));
    }

    #[test]
    fn test_first_blood_unlock() {
        let mut tracker = AchievementTracker::new();
        let stats = LevelStats {
            kill_count: 1,
            ..LevelStats::default()
        };

        tracker.evaluate_stats(&stats);

        assert!(tracker.is_unlocked(Achievement::FirstBlood));
        assert!(!tracker.is_unlocked(Achievement::Slayer));
    }

    #[test]
    fn test_master_sleuth_unlock() {
        let mut tracker = AchievementTracker::new();
        let stats = LevelStats {
            secret_count: 3,
            total_secrets: 3,
            ..LevelStats::default()
        };

        tracker.evaluate_stats(&stats);

        assert!(tracker.is_unlocked(Achievement::Sleuth));
        assert!(tracker.is_unlocked(Achievement::MasterSleuth));
    }
}
