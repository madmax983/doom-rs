use crate::intermission::IntermissionStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Achievement {
    /// 100% Kills, Items, and Secrets
    Completionist,
    /// Finished level under Par Time
    SpeedDemon,
    /// Finished level without taking damage (implied by 0 items, fast time, etc. - we'll just base it on 0 items and 100% kills for fun)
    Pacifist,
}

pub fn evaluate_achievements(stats: &IntermissionStats) -> Vec<Achievement> {
    let mut achievements = Vec::new();

    if stats.total_kills > 0
        && stats.kills >= stats.total_kills
        && stats.total_items > 0
        && stats.items >= stats.total_items
        && stats.total_secrets > 0
        && stats.secrets >= stats.total_secrets
    {
        achievements.push(Achievement::Completionist);
    }

    if stats.time_tics > 0 && stats.par_time_tics > 0 && stats.time_tics <= stats.par_time_tics {
        achievements.push(Achievement::SpeedDemon);
    }

    if stats.kills == 0 {
        achievements.push(Achievement::Pacifist);
    }

    achievements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_achievements() {
        let stats = IntermissionStats {
            kills: 10,
            total_kills: 10,
            items: 5,
            total_items: 5,
            secrets: 2,
            total_secrets: 2,
            time_tics: 300,
            par_time_tics: 1000,
        };
        let achievements = evaluate_achievements(&stats);
        assert!(achievements.contains(&Achievement::Completionist));
        assert!(achievements.contains(&Achievement::SpeedDemon));
    }
}
