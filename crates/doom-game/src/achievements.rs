//! Achievement tracking and evaluation.
//!
//! This module processes level end statistics to grant player achievements
//! like "Pacifist" (no monster kills) and "Speedrunner" (beating par time).

use crate::intermission::IntermissionStats;

/// Possible achievements a player can earn at the end of a level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Achievement {
    /// Finished level with 0 monsters killed by player.
    Pacifist,
    /// Finished level faster than par time.
    Speedrunner,
    /// Found 100% of the secret areas.
    Explorer,
    /// Picked up 100% of the items.
    Hoarder,
    /// Killed 100% of the monsters.
    Slayer,
}

/// Evaluates achievements based on intermission stats.
pub fn evaluate_achievements(stats: &IntermissionStats) -> Vec<Achievement> {
    let mut achievements = Vec::new();

    if stats.kills == 0 {
        achievements.push(Achievement::Pacifist);
    }

    if stats.time_tics < stats.par_time_tics {
        achievements.push(Achievement::Speedrunner);
    }

    if stats.total_secrets > 0 && stats.secrets >= stats.total_secrets {
        achievements.push(Achievement::Explorer);
    }

    if stats.total_items > 0 && stats.items >= stats.total_items {
        achievements.push(Achievement::Hoarder);
    }

    if stats.total_kills > 0 && stats.kills >= stats.total_kills {
        achievements.push(Achievement::Slayer);
    }

    achievements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pacifist_achievement() {
        let stats = IntermissionStats {
            kills: 0,
            total_kills: 10,
            items: 0,
            total_items: 0,
            secrets: 0,
            total_secrets: 0,
            time_tics: 100,
            par_time_tics: 200,
        };
        let achievements = evaluate_achievements(&stats);
        assert!(achievements.contains(&Achievement::Pacifist));
    }

    #[test]
    fn test_speedrunner_achievement() {
        let stats = IntermissionStats {
            kills: 5,
            total_kills: 10,
            items: 0,
            total_items: 0,
            secrets: 0,
            total_secrets: 0,
            time_tics: 100,
            par_time_tics: 200,
        };
        let achievements = evaluate_achievements(&stats);
        assert!(achievements.contains(&Achievement::Speedrunner));
    }

    #[test]
    fn test_explorer_achievement() {
        let stats = IntermissionStats {
            kills: 5,
            total_kills: 10,
            items: 0,
            total_items: 0,
            secrets: 3,
            total_secrets: 3,
            time_tics: 300,
            par_time_tics: 200,
        };
        let achievements = evaluate_achievements(&stats);
        assert!(achievements.contains(&Achievement::Explorer));
    }

    #[test]
    fn test_hoarder_achievement() {
        let stats = IntermissionStats {
            kills: 5,
            total_kills: 10,
            items: 25,
            total_items: 25,
            secrets: 0,
            total_secrets: 3,
            time_tics: 300,
            par_time_tics: 200,
        };
        let achievements = evaluate_achievements(&stats);
        assert!(achievements.contains(&Achievement::Hoarder));
    }

    #[test]
    fn test_slayer_achievement() {
        let stats = IntermissionStats {
            kills: 10,
            total_kills: 10,
            items: 0,
            total_items: 25,
            secrets: 0,
            total_secrets: 3,
            time_tics: 300,
            par_time_tics: 200,
        };
        let achievements = evaluate_achievements(&stats);
        assert!(achievements.contains(&Achievement::Slayer));
    }

    #[test]
    fn test_no_achievements() {
        let stats = IntermissionStats {
            kills: 5,
            total_kills: 10,
            items: 10,
            total_items: 25,
            secrets: 1,
            total_secrets: 3,
            time_tics: 300,
            par_time_tics: 200,
        };
        let achievements = evaluate_achievements(&stats);
        assert!(achievements.is_empty());
    }
}
