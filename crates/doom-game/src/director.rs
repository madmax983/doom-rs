//! AI Director for adjusting game difficulty.
//!
//! The AI Director monitors the player's state (such as health)
//! and dynamically adjusts the spawning of enemies or items to
//! maintain an engaging challenge level without being overly punitive.

use crate::PlayerState;

/// The action the [`AiDirector`] decides to take for the current tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn additional enemies to challenge the player.
    SpawnAmbush,
    /// Spawn health or helpful items because the player is struggling.
    SpawnRelief,
    /// Do not spawn anything, maintain current difficulty.
    Maintain,
}

/// The `AiDirector` analyzes the player state and determines the next action.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new [`AiDirector`].
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_game::director::AiDirector;
    ///
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's health and returns a [`DirectorAction`].
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
    /// // If health is high, spawn an ambush
    /// player.set_health_capped(100, MAX_HEALTH);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    ///
    /// // If health is low, spawn relief
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
