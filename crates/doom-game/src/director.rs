//! The AI Director module.
//!
//! The AI Director dynamically responds to the player's health and manages encounters
//! to create dramatic pacing. It spawns monsters when the player is healthy or grants relief
//! when the player is struggling.

use crate::PlayerState;

/// The actions chosen by the [`AiDirector`] to adjust the game difficulty dynamically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn enemies to increase tension when the player is doing well.
    SpawnAmbush,
    /// Spawn health or beneficial items to help the player recover.
    SpawnRelief,
    /// Do nothing; maintain the current difficulty level.
    Maintain,
}

/// The orchestrator of dynamic difficulty adjustments.
///
/// It observes the player's status (such as health) each tick and decides
/// whether to punish or reward the player to keep the gameplay engaging.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new [`AiDirector`].
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the current state of the player and decides on a [`DirectorAction`].
    ///
    /// The director decides based on the player's health. High health triggers ambushes,
    /// while very low health provides relief.
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
    /// let action = director.tick(&player);
    /// assert_eq!(action, DirectorAction::SpawnAmbush);
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
