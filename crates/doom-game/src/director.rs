//! The AI Director for dynamically controlling game pacing based on player health.

use crate::PlayerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    SpawnAmbush,
    SpawnRelief,
    Maintain,
}

pub struct AiDirector;

impl AiDirector {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's state and determines the next action.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // High health spawns ambushes
    /// player.set_health_capped(100, MAX_HEALTH);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    ///
    /// // Low health spawns relief
    /// player.set_health_capped(20, MAX_HEALTH);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
    /// ```
    pub fn tick(&mut self, player: &PlayerState) -> DirectorAction {
        let health = player.health();

        if health > 80 {
            DirectorAction::SpawnAmbush
        } else if health < 30 {
            DirectorAction::SpawnRelief
        } else {
            DirectorAction::Maintain
        }
    }
}

impl Default for AiDirector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::MobjHandle;
    use doom_types::limits::MAX_HEALTH;

    #[test]
    fn test_high_health_spawns_ambush() {
        let mut director = AiDirector::new();
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
        assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    }

    #[test]
    fn test_low_health_spawns_relief() {
        let mut director = AiDirector::new();
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(10, MAX_HEALTH);
        assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
    }

    #[test]
    fn test_medium_health_maintains() {
        let mut director = AiDirector::new();
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(50, MAX_HEALTH);
        assert_eq!(director.tick(&player), DirectorAction::Maintain);
    }
}
