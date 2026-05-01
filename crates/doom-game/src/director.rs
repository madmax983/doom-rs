//! The AI Director monitors player performance to dynamically adjust difficulty.
//!
//! This system continuously evaluates the player's health and alters the game's
//! spawning behavior. If the player is doing exceptionally well (health > 80%), it will
//! attempt to spawn ambushes to keep the tension high. Conversely, if the player is
//! struggling (health < 30%), it provides relief to avoid frustrating deaths.

use crate::PlayerState;

/// Actions the AI Director can take to adjust the game's pacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Increase difficulty by spawning additional enemies.
    SpawnAmbush,
    /// Decrease difficulty by pausing spawns or providing health.
    SpawnRelief,
    /// Keep the current game state as is.
    Maintain,
}

/// A system that monitors the player and decides on pacing adjustments.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
    ///
    /// # Examples
    /// ```
    /// use doom_game::director::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's state and returns the appropriate pacing action.
    ///
    /// # Panics
    ///
    /// This function does not panic.
    ///
    /// # Examples
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::player::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // If health is > 80, it spawns an ambush
    /// player.set_health_capped(100, 100);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
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
