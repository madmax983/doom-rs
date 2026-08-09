//! AI Director system for dynamic spawn control based on player health.

use crate::PlayerState;

/// The action the AI Director has decided to take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn an ambush if the player is healthy.
    SpawnAmbush,
    /// Spawn relief items if the player is struggling.
    SpawnRelief,
    /// Maintain the current spawn rate.
    Maintain,
}

/// A high-level AI Director that monitors the player's health and alters game difficulty dynamically.
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
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
/// ```
pub struct AiDirector;

impl AiDirector {
    pub fn new() -> Self {
        Self
    }

    /// Measures the player's current distress level against survival thresholds
    /// to determine if the next spawned monster should be a relief item or an ambush.
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
