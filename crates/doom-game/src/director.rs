//! Provides an AI Director mechanism that observes the player's state
//! and determines whether the game should increase pressure (ambush)
//! or provide relief based on current health levels.

use crate::PlayerState;

/// The actions that the `AiDirector` can decide to take in order to balance gameplay.
///
/// ## Examples
/// ```
/// use doom_game::director::DirectorAction;
///
/// let action = DirectorAction::SpawnAmbush;
/// assert_eq!(action, DirectorAction::SpawnAmbush);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Indicates the player is doing well and the engine should spawn more enemies or an ambush.
    SpawnAmbush,
    /// Indicates the player is struggling and the engine should provide health or ammo relief.
    SpawnRelief,
    /// Indicates the current pressure is appropriate and no intervention is needed.
    Maintain,
}

/// Observes player state and calculates dynamic difficulty adjustments.
///
/// ## Examples
/// ```
/// use doom_game::director::{AiDirector, DirectorAction};
/// use doom_game::PlayerState;
/// use doom_game::mobj::MobjHandle;
///
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
/// player.set_health_capped(100, 100);
///
/// // Player has high health, the director will spawn an ambush!
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector` with default settings.
    ///
    /// ## Examples
    /// ```
    /// use doom_game::director::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current health and returns a `DirectorAction` recommending
    /// whether to increase pressure, provide relief, or maintain current state.
    ///
    /// ## Examples
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// player.set_health_capped(20, 100);
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
