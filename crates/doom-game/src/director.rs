//! The AI Director dynamically influences the game based on the player's performance.
//!
//! This module determines what kinds of events or monsters should be spawned
//! in response to the current [`PlayerState`].

use crate::PlayerState;

/// The action the AI Director has decided to take this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing well; spawn additional enemies or threats.
    SpawnAmbush,
    /// The player is struggling; spawn health or ammo, or reduce threat.
    SpawnRelief,
    /// Maintain the current status quo.
    Maintain,
}

/// The AI Director evaluates the player's status and issues commands to alter the game environment.
///
/// ## Examples
///
/// ```rust
/// use doom_game::director::{AiDirector, DirectorAction};
/// use doom_game::player::PlayerState;
/// use doom_game::mobj::MobjHandle;
///
/// let mut director = AiDirector::new();
/// let player = PlayerState::pistol_start(MobjHandle::NULL);
/// let action = director.tick(&player);
/// assert_eq!(action, DirectorAction::SpawnAmbush); // Pistol start health is 100 > 80
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new AI Director instance.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the given [`PlayerState`] and determines the next [`DirectorAction`].
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
