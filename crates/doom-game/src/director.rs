//! Dynamic adjustment of game difficulty based on player state.
//!
//! The AI Director monitors the [`PlayerState`] to determine the player's performance
//! and decides whether to increase or decrease the intensity of the game by taking
//! a [`DirectorAction`].

use crate::PlayerState;

/// An action determined by the AI Director to adjust game difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Increase difficulty by spawning an ambush when the player is performing too well.
    SpawnAmbush,
    /// Decrease difficulty by spawning relief items when the player is struggling.
    SpawnRelief,
    /// Maintain the current difficulty level.
    Maintain,
}

/// The AI Director evaluates the current state of the game and determines actions to keep the player engaged.
///
/// ## Examples
///
/// ```
/// use doom_game::{AiDirector, DirectorAction, PlayerState};
/// use doom_game::mobj::MobjHandle;
/// use doom_types::limits::MAX_HEALTH;
///
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
///
/// // If the player is struggling, the director spawns relief.
/// player.set_health_capped(10, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new, default `AiDirector`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's state and returns the recommended [`DirectorAction`].
    ///
    /// Currently, this checks the player's health. High health triggers an ambush,
    /// while low health triggers relief items.
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
