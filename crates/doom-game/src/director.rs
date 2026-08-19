//! Manages dynamic difficulty adjustment during gameplay.
//!
//! The AI Director monitors the player's health and alters enemy
//! spawn rates or behavior to ensure the game remains engaging,
//! stepping in to provide either an `Ambush` if the player is doing
//! too well, or `Relief` if they are struggling.

use crate::PlayerState;

/// The action decided by the [`AiDirector`] for the current tic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawns an ambush to increase difficulty.
    SpawnAmbush,
    /// Spawns health or reduces difficulty to help the player.
    SpawnRelief,
    /// Maintains the current difficulty level.
    Maintain,
}

/// Dynamic difficulty manager that monitors player status.
///
/// The `AiDirector` analyzes the [`PlayerState`] every tic and
/// determines if the game should spawn more enemies or provide relief.
///
/// ## Examples
/// ```
/// use doom_game::director::{AiDirector, DirectorAction};
/// use doom_game::PlayerState;
/// use doom_game::mobj::MobjHandle;
/// use doom_types::limits::MAX_HEALTH;
///
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
/// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
///
/// // The player is doing well, so the director spawns an ambush!
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new [`AiDirector`].
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's status and returns the appropriate action.
    ///
    /// ## Examples
    /// ```
    /// # use doom_game::director::{AiDirector, DirectorAction};
    /// # use doom_game::PlayerState;
    /// # use doom_game::mobj::MobjHandle;
    /// # use doom_types::limits::MAX_HEALTH;
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    /// player.set_health_capped(10, MAX_HEALTH);
    ///
    /// // The player is low on health, so the director provides relief.
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
