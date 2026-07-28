use crate::stats::LevelStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Achievement {
    Pacifist,
    Untouchable,
    Speeddemon,
    Completionist,
}

#[derive(Default, Debug, Clone)]
pub struct AchievementTracker {
    damage_taken: u32,
}

impl AchievementTracker {
    #[must_use]
    pub fn new() -> Self {
        Self { damage_taken: 0 }
    }

    pub fn record_damage(&mut self, amount: u32) {
        self.damage_taken += amount;
    }

    #[must_use]
    pub fn evaluate(&self, stats: &LevelStats, par_time_tics: u32) -> Vec<Achievement> {
        let mut unlocked = Vec::new();

        if stats.kill_count == 0 {
            unlocked.push(Achievement::Pacifist);
        }
        if self.damage_taken == 0 {
            unlocked.push(Achievement::Untouchable);
        }
        if stats.level_time < par_time_tics {
            unlocked.push(Achievement::Speeddemon);
        }
        if stats.total_kills > 0
            && stats.total_items > 0
            && stats.total_secrets > 0
            && stats.kill_count == stats.total_kills
            && stats.item_count == stats.total_items
            && stats.secret_count == stats.total_secrets
        {
            unlocked.push(Achievement::Completionist);
        }

        unlocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::LevelStats;

    #[test]
    fn test_pacifist() {
        let tracker = AchievementTracker { damage_taken: 0 };
        let stats = LevelStats {
            kill_count: 0,
            ..Default::default()
        };
        assert!(
            tracker
                .evaluate(&stats, 1000)
                .contains(&Achievement::Pacifist)
        );
    }

    #[test]
    fn test_untouchable() {
        let mut tracker = AchievementTracker { damage_taken: 0 };
        tracker.record_damage(0);
        let stats = LevelStats::default();
        assert!(
            tracker
                .evaluate(&stats, 1000)
                .contains(&Achievement::Untouchable)
        );
    }

    #[test]
    fn test_speeddemon() {
        let tracker = AchievementTracker { damage_taken: 0 };
        let stats = LevelStats {
            level_time: 500,
            ..Default::default()
        };
        assert!(
            tracker
                .evaluate(&stats, 1000)
                .contains(&Achievement::Speeddemon)
        );
    }

    #[test]
    fn test_completionist() {
        let tracker = AchievementTracker { damage_taken: 0 };
        let stats = LevelStats {
            kill_count: 10,
            total_kills: 10,
            item_count: 5,
            total_items: 5,
            secret_count: 2,
            total_secrets: 2,
            ..Default::default()
        };
        assert!(
            tracker
                .evaluate(&stats, 1000)
                .contains(&Achievement::Completionist)
        );
    }
}
