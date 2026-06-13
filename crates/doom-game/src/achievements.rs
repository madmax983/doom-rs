//! Achievement tracking system.
//!
//! Monitors game state and events to unlock achievements based on player actions.

/// Represents an unlockable achievement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Achievement {
    /// Kill 10 monsters.
    Slayer,
    /// Find 3 secrets.
    Explorer,
    /// Finish a level under par time.
    Speedrunner,
}

impl Achievement {
    pub fn name(&self) -> &'static str {
        match self {
            Achievement::Slayer => "Slayer",
            Achievement::Explorer => "Explorer",
            Achievement::Speedrunner => "Speedrunner",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Achievement::Slayer => "Kill 10 monsters",
            Achievement::Explorer => "Find 3 secrets",
            Achievement::Speedrunner => "Finish a level under par time",
        }
    }
}

/// Tracks the unlock status of achievements.
#[derive(Debug, Clone, Default)]
pub struct AchievementTracker {
    pub unlocked: Vec<Achievement>,
}

impl AchievementTracker {
    pub fn new() -> Self {
        Self {
            unlocked: Vec::new(),
        }
    }

    /// Checks game parameters to see if any new achievements should be unlocked.
    /// Returns a list of newly unlocked achievements this tick.
    pub fn tick(
        &mut self,
        kill_count: u32,
        secret_count: u32,
        level_time: u32,
        level_completed: bool,
        par_time_tics: u32,
    ) -> Vec<Achievement> {
        let mut newly_unlocked = Vec::new();

        if !self.unlocked.contains(&Achievement::Slayer) && kill_count >= 10 {
            self.unlocked.push(Achievement::Slayer.clone());
            newly_unlocked.push(Achievement::Slayer);
        }

        if !self.unlocked.contains(&Achievement::Explorer) && secret_count >= 3 {
            self.unlocked.push(Achievement::Explorer.clone());
            newly_unlocked.push(Achievement::Explorer);
        }

        if level_completed && !self.unlocked.contains(&Achievement::Speedrunner) && level_time <= par_time_tics {
            self.unlocked.push(Achievement::Speedrunner.clone());
            newly_unlocked.push(Achievement::Speedrunner);
        }

        newly_unlocked
    }

    pub fn has_achievement(&self, achievement: &Achievement) -> bool {
        self.unlocked.contains(achievement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slayer_achievement() {
        let mut tracker = AchievementTracker::new();

        // Not unlocked initially
        assert!(tracker.tick(0, 0, 0, false, 1000).is_empty());

        // Unlocked after 10 kills
        let unlocked = tracker.tick(10, 0, 0, false, 1000);
        assert_eq!(unlocked, vec![Achievement::Slayer]);

        // Not unlocked again
        assert!(tracker.tick(10, 0, 0, false, 1000).is_empty());
    }

    #[test]
    fn test_explorer_achievement() {
        let mut tracker = AchievementTracker::new();

        let unlocked = tracker.tick(0, 3, 0, false, 1000);
        assert_eq!(unlocked, vec![Achievement::Explorer]);
    }

    #[test]
    fn test_speedrunner_achievement() {
        let mut tracker = AchievementTracker::new();

        // Not unlocked if level not completed
        assert!(tracker.tick(0, 0, 500, false, 1000).is_empty());

        // Unlocked if level completed under par
        let unlocked = tracker.tick(0, 0, 500, true, 1000);
        assert_eq!(unlocked, vec![Achievement::Speedrunner]);

        // Not unlocked if over par
        let mut tracker2 = AchievementTracker::new();
        assert!(tracker2.tick(0, 0, 1500, true, 1000).is_empty());
    }
}
