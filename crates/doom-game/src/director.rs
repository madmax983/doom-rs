//! The AI Director manages enemy spawning intensity based on the player's current status.
//!
//! # Context
//! This module contains the logic for scaling the game's tension dynamically.
//! Rather than static placement, the director evaluates the player's health and
//! dictates actions to either increase the threat or provide relief.

use crate::PlayerState;

/// The action dictated by the AI Director for the current simulation tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn more enemies to increase pressure on a healthy player.
    SpawnAmbush,
    /// Spawn health or beneficial items to help a struggling player.
    SpawnRelief,
    /// Maintain the current state without intervention.
    Maintain,
}

/// The main structure for the AI Director logic.
///
/// # Examples
/// ```
/// use doom_game::director::AiDirector;
/// let mut director = AiDirector::new();
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Bootstraps the director logic, initializing any required tracking metrics.
    ///
    /// # Examples
    /// ```
    /// use doom_game::director::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Computes the optimal pacing adjustment for the current tick by analyzing the [`PlayerState`].
    ///
    /// # Details
    /// The director monitors physiological stress metrics (primarily health) to prevent the
    /// simulation from becoming overly static. If the player is excelling (health > 80), it dictates
    /// [`DirectorAction::SpawnAmbush`] to increase tension. Conversely, if the player is struggling
    /// (health < 30), it provides breathing room via [`DirectorAction::SpawnRelief`].
    ///
    /// # Examples
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
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
