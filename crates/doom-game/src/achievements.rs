use crate::stats::LevelStats;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Achievement {
    FirstBlood,
    Pacifist,
    SecretFinder,
    Completionist,
    Speedrunner,
}

impl Achievement {
    pub fn name(&self) -> &'static str {
        match self {
            Achievement::FirstBlood => "First Blood",
            Achievement::Pacifist => "Pacifist",
            Achievement::SecretFinder => "Secret Finder",
            Achievement::Completionist => "Completionist",
            Achievement::Speedrunner => "Speedrunner",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Achievement::FirstBlood => "Get your first kill.",
            Achievement::Pacifist => "Finish a level without killing any monsters.",
            Achievement::SecretFinder => "Find all secrets in a level.",
            Achievement::Completionist => {
                "Find all items, kill all monsters, and find all secrets in a level."
            }
            Achievement::Speedrunner => "Finish a level in under 2 minutes (4200 tics).",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AchievementTracker {
    pub unlocked: HashSet<Achievement>,
}

impl AchievementTracker {
    pub fn new() -> Self {
        Self {
            unlocked: HashSet::new(),
        }
    }

    pub fn update_mid_level(&mut self, stats: &LevelStats) -> Vec<Achievement> {
        let mut newly_unlocked = Vec::new();

        if stats.kill_count > 0 && !self.unlocked.contains(&Achievement::FirstBlood) {
            self.unlocked.insert(Achievement::FirstBlood.clone());
            newly_unlocked.push(Achievement::FirstBlood);
        }

        newly_unlocked
    }

    pub fn update_end_of_level(&mut self, stats: &LevelStats) -> Vec<Achievement> {
        let mut newly_unlocked = Vec::new();

        if stats.kill_count == 0
            && stats.total_kills > 0
            && !self.unlocked.contains(&Achievement::Pacifist)
        {
            self.unlocked.insert(Achievement::Pacifist.clone());
            newly_unlocked.push(Achievement::Pacifist);
        }

        if stats.secret_count >= stats.total_secrets
            && stats.total_secrets > 0
            && !self.unlocked.contains(&Achievement::SecretFinder)
        {
            self.unlocked.insert(Achievement::SecretFinder.clone());
            newly_unlocked.push(Achievement::SecretFinder);
        }

        if stats.kill_count >= stats.total_kills
            && stats.item_count >= stats.total_items
            && stats.secret_count >= stats.total_secrets
            && (stats.total_kills > 0 || stats.total_items > 0 || stats.total_secrets > 0)
            && !self.unlocked.contains(&Achievement::Completionist)
        {
            self.unlocked.insert(Achievement::Completionist.clone());
            newly_unlocked.push(Achievement::Completionist);
        }

        if stats.level_time < 4200 && !self.unlocked.contains(&Achievement::Speedrunner) {
            self.unlocked.insert(Achievement::Speedrunner.clone());
            newly_unlocked.push(Achievement::Speedrunner);
        }

        newly_unlocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_blood() {
        let mut tracker = AchievementTracker::new();
        let mut stats = LevelStats::default();

        let unlocked = tracker.update_mid_level(&stats);
        assert!(unlocked.is_empty());

        stats.kill_count = 1;
        let unlocked = tracker.update_mid_level(&stats);
        assert_eq!(unlocked.len(), 1);
        assert_eq!(unlocked[0], Achievement::FirstBlood);

        let unlocked2 = tracker.update_mid_level(&stats);
        assert!(unlocked2.is_empty());
    }

    #[test]
    fn test_end_of_level_achievements() {
        let mut tracker = AchievementTracker::new();
        let mut stats = LevelStats {
            kill_count: 0,
            item_count: 5,
            secret_count: 2,
            total_kills: 10,
            total_items: 5,
            total_secrets: 2,
            level_time: 4000,
        };

        let unlocked = tracker.update_end_of_level(&stats);
        assert!(unlocked.contains(&Achievement::Pacifist));
        assert!(unlocked.contains(&Achievement::SecretFinder));
        assert!(!unlocked.contains(&Achievement::Completionist));
        assert!(unlocked.contains(&Achievement::Speedrunner));

        // Completionist
        tracker.unlocked.clear();
        stats.kill_count = 10;
        let unlocked = tracker.update_end_of_level(&stats);
        assert!(!unlocked.contains(&Achievement::Pacifist));
        assert!(unlocked.contains(&Achievement::Completionist));
    }
}
