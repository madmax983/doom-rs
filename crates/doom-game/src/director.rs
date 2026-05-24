//! AI Director for dynamic difficulty scaling.
//!
//! This module provides a simple `AiDirector` that monitors the player's health
//! and decides whether to spawn additional ambush encounters or provide relief
//! to maintain an engaging pacing.

use crate::PlayerState;

/// Actions the AI Director can take to alter the game state based on player performance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn additional enemies to challenge the player.
    SpawnAmbush,
    /// Provide relief by dropping health or ammo, or reducing enemy aggression.
    SpawnRelief,
    /// Maintain the current difficulty level.
    Maintain,
}

/// An orchestrator that observes player state and suggests pacing actions.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_game::director::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current condition and determines the appropriate `DirectorAction`.
    ///
    /// # Parameters
    ///
    /// * `player`: The current state of the player.
    ///
    /// # Returns
    ///
    /// A `DirectorAction` dictating how the simulation should respond.
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
