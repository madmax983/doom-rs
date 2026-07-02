//! AI Director module for dynamic difficulty adjustment.
//!
//! The AI director monitors the player's health and spawns enemies or relief drops based on the state.

use crate::PlayerState;

/// Actions that the director can take based on player state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn an ambush for the player if they have high health.
    SpawnAmbush,
    /// Spawn relief for the player if they have low health.
    SpawnRelief,
    /// Maintain the current difficulty if health is average.
    Maintain,
}

/// The AI Director responsible for adjusting game difficulty based on player state.
///
/// ## Examples
/// ```
/// use doom_game::director::AiDirector;
///
/// let mut director = AiDirector::new();
/// // director.tick(&player) would be called here
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's state and returns the appropriate action.
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
