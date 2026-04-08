//! Achievement evaluation based on end-of-level statistics.
//!
//! Provides the `Achievement` enum and an evaluation function to determine
//! which playstyle accolades a player has earned during a level.

use crate::intermission::IntermissionStats;

/// End-of-level achievements awarded for specific playstyles or accomplishments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Achievement {
    /// Finish a level in under par time.
    Speedrun,
    /// Finish a level without killing any monsters (must have >0 monsters available).
    Pacifist,
    /// Kill all monsters, find all secrets, and get all items.
    Completionist,
    /// Find all secrets in a map.
    Snoop,
    /// Kill all monsters in a map.
    Slayer,
    /// Pick up all items in a map.
    Hoarder,
}

impl Achievement {
    /// Returns the display name of the achievement.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Speedrun => "Speedrun",
            Self::Pacifist => "Pacifist",
            Self::Completionist => "Completionist",
            Self::Snoop => "Snoop",
            Self::Slayer => "Slayer",
            Self::Hoarder => "Hoarder",
        }
    }

    /// Returns a short description of the achievement.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Speedrun => "Beat the par time for the level.",
            Self::Pacifist => "Complete the level without killing any monsters.",
            Self::Completionist => "100% Kills, Items, and Secrets.",
            Self::Snoop => "Find 100% of the secrets.",
            Self::Slayer => "Kill 100% of the monsters.",
            Self::Hoarder => "Acquire 100% of the items.",
        }
    }
}

/// Evaluates a player's `IntermissionStats` to award achievements.
#[must_use]
pub fn evaluate_achievements(stats: &IntermissionStats) -> Vec<Achievement> {
    let mut achievements = Vec::new();

    // Speedrun: Beat the par time (par time must be > 0)
    if stats.par_time_tics > 0 && stats.time_tics < stats.par_time_tics {
        achievements.push(Achievement::Speedrun);
    }

    // Pacifist: 0 kills, but the level must have had at least 1 monster to spare
    if stats.kills == 0 && stats.total_kills > 0 {
        achievements.push(Achievement::Pacifist);
    }

    let mut completionist_criteria = 0;

    // Slayer: 100% kills
    if stats.total_kills > 0 && stats.kills >= stats.total_kills {
        achievements.push(Achievement::Slayer);
        completionist_criteria += 1;
    } else if stats.total_kills == 0 {
        completionist_criteria += 1; // Count as met if impossible
    }

    // Hoarder: 100% items
    if stats.total_items > 0 && stats.items >= stats.total_items {
        achievements.push(Achievement::Hoarder);
        completionist_criteria += 1;
    } else if stats.total_items == 0 {
        completionist_criteria += 1;
    }

    // Snoop: 100% secrets
    if stats.total_secrets > 0 && stats.secrets >= stats.total_secrets {
        achievements.push(Achievement::Snoop);
        completionist_criteria += 1;
    } else if stats.total_secrets == 0 {
        completionist_criteria += 1;
    }

    // Completionist: Met all 3 100% criteria, and at least one category actually existed
    if completionist_criteria == 3
        && (stats.total_kills > 0 || stats.total_items > 0 || stats.total_secrets > 0)
    {
        achievements.push(Achievement::Completionist);
    }

    achievements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speedrun() {
        let mut stats = IntermissionStats {
            par_time_tics: 1000,
            time_tics: 900,
            ..Default::default()
        };
        let ach = evaluate_achievements(&stats);
        assert!(ach.contains(&Achievement::Speedrun));

        stats.time_tics = 1100;
        let ach2 = evaluate_achievements(&stats);
        assert!(!ach2.contains(&Achievement::Speedrun));
    }

    #[test]
    fn test_pacifist() {
        let mut stats = IntermissionStats {
            kills: 0,
            total_kills: 10,
            ..Default::default()
        };
        let ach = evaluate_achievements(&stats);
        assert!(ach.contains(&Achievement::Pacifist));

        stats.kills = 1;
        let ach2 = evaluate_achievements(&stats);
        assert!(!ach2.contains(&Achievement::Pacifist));

        // No pacifist if there are no monsters
        stats.kills = 0;
        stats.total_kills = 0;
        let ach3 = evaluate_achievements(&stats);
        assert!(!ach3.contains(&Achievement::Pacifist));
    }

    #[test]
    fn test_slayer() {
        let stats = IntermissionStats {
            kills: 10,
            total_kills: 10,
            ..Default::default()
        };
        let ach = evaluate_achievements(&stats);
        assert!(ach.contains(&Achievement::Slayer));
    }

    #[test]
    fn test_hoarder() {
        let stats = IntermissionStats {
            items: 5,
            total_items: 5,
            ..Default::default()
        };
        let ach = evaluate_achievements(&stats);
        assert!(ach.contains(&Achievement::Hoarder));
    }

    #[test]
    fn test_snoop() {
        let stats = IntermissionStats {
            secrets: 2,
            total_secrets: 2,
            ..Default::default()
        };
        let ach = evaluate_achievements(&stats);
        assert!(ach.contains(&Achievement::Snoop));
    }

    #[test]
    fn test_completionist() {
        let stats = IntermissionStats {
            kills: 10,
            total_kills: 10,
            items: 5,
            total_items: 5,
            secrets: 2,
            total_secrets: 2,
            ..Default::default()
        };
        let ach = evaluate_achievements(&stats);
        assert!(ach.contains(&Achievement::Completionist));
        assert!(ach.contains(&Achievement::Slayer));
        assert!(ach.contains(&Achievement::Hoarder));
        assert!(ach.contains(&Achievement::Snoop));
    }

    #[test]
    fn test_completionist_empty_level_no_reward() {
        let stats = IntermissionStats::default();
        let ach = evaluate_achievements(&stats);
        assert!(!ach.contains(&Achievement::Completionist));
    }

    #[test]
    fn test_achievement_properties() {
        let ach = Achievement::Speedrun;
        assert_eq!(ach.name(), "Speedrun");
        assert_eq!(ach.description(), "Beat the par time for the level.");
    }
}
