//! An AI Director system that adjusts gameplay based on the player's performance.
//!
//! This module contains the logic that monitors player health and decides
//! whether to pressure the player or provide relief, akin to modern "AI Director"
//! mechanics found in games like Left 4 Dead.

use crate::PlayerState;

/// Action decided by the AI Director based on the player's current status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Increase tension by spawning an ambush when the player is healthy.
    SpawnAmbush,
    /// Provide relief by spawning health or ammo when the player is near death.
    SpawnRelief,
    /// Keep the current status quo if the player's health is within standard limits.
    Maintain,
}

/// The AI Director evaluates the game state and determines the next course of action.
///
/// `AiDirector` makes adjustments to the spawn queues or internal difficulty based
/// on the player's health.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::director::AiDirector;
    ///
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's health and returns a `DirectorAction`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::player::PlayerState;
    /// use doom_types::mobj_kind::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    /// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
    ///
    /// // A healthy player gets ambushed
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
