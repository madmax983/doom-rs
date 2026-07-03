//! Dynamic difficulty adjustment based on player health.
//!
//! The AI Director monitors the player's status and determines the overall
//! strategy for the game, deciding whether to spawn challenging ambushes
//! or provide relief items based on how well the player is doing.

use crate::PlayerState;

/// The strategy decided by the AI Director based on current player state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn a difficult ambush. Triggered when the player is doing very well.
    SpawnAmbush,
    /// Spawn relief items (health/ammo). Triggered when the player is near death.
    SpawnRelief,
    /// Maintain the current difficulty level.
    Maintain,
}

/// The system that monitors the player and dynamically adjusts game difficulty.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new AI Director.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_game::director::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the current player state and determines the next action.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    ///
    /// let mut director = AiDirector::new();
    /// // Simulate player getting hurt
    /// // In reality, we'd use `player.health()` and see DirectorAction::SpawnRelief
    /// let _action = director.tick(&PlayerState::pistol_start(MobjHandle::NULL));
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
