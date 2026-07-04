//! The Artificial Intelligence Director module.
//!
//! This module manages the pacing of the game by spawning enemies or items
//! depending on the player's current health state. It's designed to keep
//! the tension high without feeling unfair to the player.

use crate::PlayerState;

/// The action chosen by the [`AiDirector`] for a given tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawns more aggressive enemies because the player is doing too well.
    SpawnAmbush,
    /// Spawns health and ammo because the player is struggling.
    SpawnRelief,
    /// Does nothing, allowing the current situation to play out naturally.
    Maintain,
}

/// The system responsible for monitoring the player and dynamically adjusting difficulty.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new, idle [`AiDirector`].
    pub fn new() -> Self {
        Self
    }

    /// Analyzes the [`PlayerState`] and decides on the next [`DirectorAction`].
    ///
    /// The director monitors the player's health and dynamically adjusts the
    /// intensity of the game by spawning ambushes when health is high, or
    /// providing relief when health is low.
    ///
    /// ## Examples
    ///
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
    /// // High health triggers an ambush!
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
