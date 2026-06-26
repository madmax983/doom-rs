use crate::intermission::par_time;
use crate::state::GameState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Achievement {
    Pacifist,
    Completionist,
    Speedrunner,
}

pub fn evaluate_achievements(gs: &GameState) -> Vec<Achievement> {
    let mut achievements = Vec::new();

    // Pacifist: 0 kills but level completed
    if gs.player.kill_count == 0 {
        achievements.push(Achievement::Pacifist);
    }

    // Completionist: 100% kills, items, and secrets
    if gs.stats.total_kills > 0
        && gs.player.kill_count >= gs.stats.total_kills
        && gs.stats.total_items > 0
        && gs.player.item_count >= gs.stats.total_items
        && gs.stats.total_secrets > 0
        && gs.player.secret_count >= gs.stats.total_secrets
    {
        achievements.push(Achievement::Completionist);
    }

    // Speedrunner: beat par time
    let par = par_time(&gs.level_name);
    if par > 0 && gs.stats.level_time < par {
        achievements.push(Achievement::Speedrunner);
    }

    achievements
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_state(level: &str, kills: u32, total_kills: u32, time: u32) -> GameState {
        let mut gs = GameState::new(level);
        gs.player.kill_count = kills;
        gs.stats.total_kills = total_kills;
        gs.stats.level_time = time;
        gs
    }

    #[test]
    fn test_pacifist() {
        let gs = make_test_state("E1M1", 0, 10, 5000);
        let ach = evaluate_achievements(&gs);
        assert!(ach.contains(&Achievement::Pacifist));
    }

    #[test]
    fn test_speedrunner() {
        let gs = make_test_state("E1M1", 5, 10, 500); // E1M1 par is 30s (1050 tics)
        let ach = evaluate_achievements(&gs);
        assert!(ach.contains(&Achievement::Speedrunner));
    }
}
