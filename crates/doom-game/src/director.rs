//! AI Director for dynamic difficulty adjustment.
//!
//! The AI Director monitors player performance and dynamically alters
//! the game's difficulty by spawning ambushes or providing relief.

use crate::PlayerState;

/// The action decided by the `AiDirector` to adjust game difficulty dynamically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Indicates the director should spawn additional enemies to challenge a healthy player.
    SpawnAmbush,
    /// Indicates the director should spawn health or ammo to assist a struggling player.
    SpawnRelief,
    /// Indicates the director should take no action, maintaining the current game state.
    Maintain,
}

/// An entity responsible for monitoring player performance and adjusting game difficulty dynamically.
///
/// The `AiDirector` observes the `PlayerState` and makes decisions to either challenge
/// the player if they are doing well, or provide relief if they are struggling.
///
/// # Examples
///
/// ```
/// use doom_game::{AiDirector, DirectorAction};
/// use doom_game::PlayerState;
/// use doom_game::mobj::MobjHandle;
///
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
///
/// // Player is doing well
/// player.set_health_capped(100, 100);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector` instance.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the current `PlayerState` and returns a `DirectorAction`.
    ///
    /// This method is typically called once per tick or at regular intervals
    /// to continuously monitor the player's status and adjust the game
    /// environment accordingly.
    ///
    /// # Parameters
    ///
    /// * `player`: A reference to the current `PlayerState`.
    ///
    /// # Returns
    ///
    /// Returns a `DirectorAction` based on the player's health.
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
