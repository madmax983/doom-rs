//! The AI Director module.
//!
//! # Context
//!
//! The AI Director dynamically adjusts the game's difficulty and pacing by analyzing the player's
//! current state (such as health) and deciding whether to spawn additional challenges or offer relief.
//!
//! # Details
//!
//! The director evaluates the player's state every tick. If the player is doing well, it may spawn
//! ambushes to increase pressure. If the player is struggling, it may spawn relief items or weaker enemies.
//!
//! # Examples
//!
//! ```
//! use doom_game::director::AiDirector;
//!
//! let mut director = AiDirector::new();
//! // In a real game loop, you would pass the current `PlayerState`.
//! ```

use crate::PlayerState;

/// Actions the AI Director can take to adjust the game's pacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn additional enemies to challenge a healthy player.
    SpawnAmbush,
    /// Spawn health or ammo to assist a struggling player.
    SpawnRelief,
    /// Take no action, allowing the current game state to proceed normally.
    Maintain,
}

/// The AI Director evaluates the player's state and decides on pacing actions.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new AI Director instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_game::director::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the given `PlayerState` and determines the appropriate `DirectorAction`.
    ///
    /// # Context
    ///
    /// This method is called periodically during the game loop to assess the player's current health
    /// and adjust the game's difficulty accordingly.
    ///
    /// # Details
    ///
    /// - If health > 80, returns [`DirectorAction::SpawnAmbush`].
    /// - If health < 30, returns [`DirectorAction::SpawnRelief`].
    /// - Otherwise, returns [`DirectorAction::Maintain`].
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
    /// player.set_health_capped(100, MAX_HEALTH);
    ///
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
