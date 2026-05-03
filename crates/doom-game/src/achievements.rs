//! Achievements subsystem tracking gameplay milestones.
//!
//! Evaluates the `GameState` each frame or at level exit to unlock
//! specific challenges and achievements.

use crate::state::GameState;
use std::collections::HashSet;

/// Defined game achievements.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Achievement {
    /// Finish the level with zero kills.
    Pacifist,
    /// Find 100% of the secrets in a level.
    Snoop,
    /// Finish a level in under 1 minute (35 * 60 = 2100 tics).
    SpeedDemon,
    /// Achieve 100 kills in a single session.
    RipAndTear,
}

/// Tracks and unlocks achievements over the course of a play session.
#[derive(Debug, Clone, Default)]
pub struct AchievementTracker {
    /// Achievements that have been unlocked.
    pub unlocked: HashSet<Achievement>,
}

impl AchievementTracker {
    /// Create a new, empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            unlocked: HashSet::new(),
        }
    }

    /// Update the tracker with the current game state, unlocking achievements if criteria are met.
    pub fn update(&mut self, gs: &GameState) {
        // Continuous checks
        if gs.stats.kill_count >= 100 {
            self.unlocked.insert(Achievement::RipAndTear);
        }

        // Level exit checks
        if gs.exit_request.is_some() {
            if gs.stats.kill_count == 0 {
                self.unlocked.insert(Achievement::Pacifist);
            }

            if gs.stats.secret_count >= gs.stats.total_secrets && gs.stats.total_secrets > 0 {
                self.unlocked.insert(Achievement::Snoop);
            }

            if gs.stats.level_time <= 2100 {
                self.unlocked.insert(Achievement::SpeedDemon);
            }
        }
    }

    /// Check if a specific achievement is unlocked.
    #[must_use]
    pub fn is_unlocked(&self, achievement: &Achievement) -> bool {
        self.unlocked.contains(achievement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ExitRequest;

    #[test]
    fn test_rip_and_tear_continuous() {
        let mut tracker = AchievementTracker::new();
        let mut gs = GameState::new("E1M1");

        gs.stats.kill_count = 50;
        tracker.update(&gs);
        assert!(!tracker.is_unlocked(&Achievement::RipAndTear));

        gs.stats.kill_count = 100;
        tracker.update(&gs);
        assert!(tracker.is_unlocked(&Achievement::RipAndTear));
    }

    #[test]
    fn test_pacifist_on_exit() {
        let mut tracker = AchievementTracker::new();
        let mut gs = GameState::new("E1M1");

        gs.stats.kill_count = 0;
        tracker.update(&gs);
        assert!(
            !tracker.is_unlocked(&Achievement::Pacifist),
            "Should not unlock before exit"
        );

        gs.exit_request = Some(ExitRequest::Normal);
        tracker.update(&gs);
        assert!(tracker.is_unlocked(&Achievement::Pacifist));
    }

    #[test]
    fn test_snoop_on_exit() {
        let mut tracker = AchievementTracker::new();
        let mut gs = GameState::new("E1M1");

        gs.stats.secret_count = 2;
        gs.stats.total_secrets = 2;

        tracker.update(&gs);
        assert!(!tracker.is_unlocked(&Achievement::Snoop));

        gs.exit_request = Some(ExitRequest::Normal);
        tracker.update(&gs);
        assert!(tracker.is_unlocked(&Achievement::Snoop));
    }

    #[test]
    fn test_speed_demon_on_exit() {
        let mut tracker = AchievementTracker::new();
        let mut gs = GameState::new("E1M1");

        gs.stats.level_time = 2000;

        tracker.update(&gs);
        assert!(!tracker.is_unlocked(&Achievement::SpeedDemon));

        gs.exit_request = Some(ExitRequest::Normal);
        tracker.update(&gs);
        assert!(tracker.is_unlocked(&Achievement::SpeedDemon));
    }
}
