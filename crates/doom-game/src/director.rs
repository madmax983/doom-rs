//! The AI Director.
//!
//! This module contains the logic for the dynamic game difficulty adjustment.
//! By analyzing the [`PlayerState`] (like health, ammo, and recent damage),
//! the AI Director decides whether to increase pressure by spawning ambushes,
//! or to ease up and spawn relief items.

use crate::PlayerState;

/// Represents the current action or decision made by the [`AiDirector`].
///
/// Based on the [`PlayerState`], the director chooses how to alter the pacing of the game.
///
/// ## Examples
///
/// ```
/// use doom_game::director::DirectorAction;
///
/// let action = DirectorAction::SpawnAmbush;
/// assert_eq!(action, DirectorAction::SpawnAmbush);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    SpawnAmbush,
    SpawnRelief,
    Maintain,
}

/// The `AiDirector` analyzes the player's performance and adjusts the game's difficulty dynamically.
///
/// It uses heuristics like current health to make decisions via the `tick` method.
///
/// ## Examples
///
/// ```
/// use doom_game::director::{AiDirector, DirectorAction};
/// use doom_game::PlayerState;
/// use doom_game::mobj::MobjHandle;
/// use doom_types::limits::MAX_HEALTH;
///
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
/// player.set_health_capped(MAX_HEALTH, MAX_HEALTH); // Healthy player
///
/// // The director decides to increase pressure
/// let action = director.tick(&player);
/// assert_eq!(action, DirectorAction::SpawnAmbush);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector` instance.
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

    /// Evaluates the current [`PlayerState`] and returns the recommended [`DirectorAction`].
    ///
    /// This should be called periodically during the game loop to maintain dynamic pacing.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    /// player.set_health_capped(10, MAX_HEALTH); // Badly injured player
    ///
    /// // The director decides to offer relief
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
