//! The Director module handles dynamic difficulty adjustments.
//!
//! The [`AiDirector`] monitors the player's performance and issues a [`DirectorAction`]
//! to maintain optimal tension during gameplay, dynamically altering spawn rates or pickups.

use crate::PlayerState;

/// The action requested by the [`AiDirector`] to alter gameplay difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn enemies to increase pressure on a healthy player.
    SpawnAmbush,
    /// Provide health or ammo to assist a struggling player.
    SpawnRelief,
    /// Keep the current tension level.
    Maintain,
}

/// A dynamic difficulty adjustment engine.
///
/// The `AiDirector` evaluates the [`PlayerState`] every tick and decides whether to
/// increase or decrease the pressure on the player by returning a [`DirectorAction`].
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
/// // A healthy player triggers an ambush
/// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Initializes a new, stateless `AiDirector`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's status and determines the next action.
    ///
    /// This method looks at the player's health to decide if the game needs to provide
    /// more challenge ([`DirectorAction::SpawnAmbush`]) or a breather ([`DirectorAction::SpawnRelief`]).
    ///
    /// # Arguments
    /// * `player` - The current [`PlayerState`] to evaluate.
    ///
    /// # Returns
    /// The recommended [`DirectorAction`] for the current tick.
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
    /// Creates a default `AiDirector`. See [`AiDirector::new`].
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
