//! Artificial Intelligence Director for pacing gameplay.
//!
//! The AI Director monitors the player's performance and adjusts the game's difficulty dynamically.
//! It aims to maintain a state of "flow" by providing challenges when the player is doing well,
//! and relief when the player is struggling.
//!
//! Currently, the director simply evaluates the player's health to decide whether to spawn an ambush
//! or a relief item.

use crate::PlayerState;

/// The action the [`AiDirector`] has decided to take this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn an ambush to challenge the player (triggered when health is high).
    SpawnAmbush,
    /// Spawn a relief item (like health or ammo) to help the player (triggered when health is low).
    SpawnRelief,
    /// Do nothing and maintain the current pacing.
    Maintain,
}

/// The AI Director responsible for monitoring player state and issuing pacing actions.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new [`AiDirector`].
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the current game state and determines the next action.
    ///
    /// The director checks the player's health and decides whether to spawn an ambush
    /// (health > 80), spawn relief (health < 30), or do nothing.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // Simulate high health to trigger an ambush.
    /// player.set_health_capped(100, MAX_HEALTH);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    ///
    /// // Simulate low health to trigger relief.
    /// player.set_health_capped(20, MAX_HEALTH);
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
