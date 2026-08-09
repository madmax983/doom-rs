//! Dynamic AI difficulty adjustment module.
//!
//! This module provides a mechanism to monitor the player's state and dynamically
//! determine whether the game should spawn more enemies (ambush) or fewer (relief)
//! to maintain a balanced difficulty level.

use crate::PlayerState;

/// The action recommended by the AI director based on the player's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Recommended when the player is doing very well (high health).
    SpawnAmbush,
    /// Recommended when the player is struggling (low health).
    SpawnRelief,
    /// Recommended when the player is at a stable state.
    Maintain,
}

/// Monitors player state to provide dynamic difficulty adjustments.
///
/// The `AiDirector` evaluates the player's current health during each tick and
/// recommends actions to keep the gameplay engaging without being overwhelmingly
/// difficult or trivially easy.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::director::AiDirector;
    ///
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's health and returns a recommended action.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::player::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    /// player.set_health_capped(100, MAX_HEALTH);
    ///
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
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
