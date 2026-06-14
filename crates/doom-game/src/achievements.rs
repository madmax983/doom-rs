use crate::state::GameState;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Achievement {
    Pacifist,
    Untouchable,
}

#[derive(Default, Debug, Clone)]
pub struct AchievementTracker {
    pub unlocked: std::collections::HashSet<Achievement>,
}

impl AchievementTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self, gs: &GameState) {
        if gs.exit_request.is_some() {
            if gs.stats.kill_count == 0 {
                self.unlocked.insert(Achievement::Pacifist);
            }
            if gs.player.health() == doom_types::limits::MAX_HEALTH {
                self.unlocked.insert(Achievement::Untouchable);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;

    #[test]
    fn test_pacifist_achievement() {
        let mut tracker = AchievementTracker::new();
        let mut gs = GameState::new("E1M1");
        gs.stats.kill_count = 0;
        gs.exit_request = Some(crate::state::ExitRequest::Normal);
        tracker.tick(&gs);
        assert!(tracker.unlocked.contains(&Achievement::Pacifist));
    }
}
