//! The AI Director module.
//!
//! The AI Director dynamically adjusts the game's difficulty based on the player's performance.
//!
//! # The Invisible Hand
//!
//! Instead of static monster placements, the [`AiDirector`] monitors the player's [`PlayerState`]
//! and determines whether to spawn reinforcements or relief items depending on how well the player is doing.
//!

use crate::PlayerState;

/// The action the [`AiDirector`] has decided to take this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing too well. Spawn a monster ambush to increase pressure.
    SpawnAmbush,
    /// The player is near death. Spawn health or ammo to provide relief.
    SpawnRelief,
    /// The player is in the sweet spot. No action is required.
    Maintain,
}

/// The system responsible for dynamic difficulty adjustment.
///
/// By calling [`AiDirector::tick`] each frame, the game can decide how to respond
/// to the player's current health level.
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
/// // Player is doing great
/// player.set_health_capped(100, 100);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
///
/// // Player is almost dead
/// player.set_health_capped(10, 100);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new [`AiDirector`] instance.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's state and returns a [`DirectorAction`].
    ///
    /// The director checks the player's health to decide the next action:
    /// - Health > 80: Returns [`DirectorAction::SpawnAmbush`]
    /// - Health < 30: Returns [`DirectorAction::SpawnRelief`]
    /// - Otherwise: Returns [`DirectorAction::Maintain`]
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
