//! Analyzes player performance and manipulates enemy spawns.
//!
//! The AI Director monitors the player's health, ammo, and general tension levels.
//! Based on these metrics, it decides whether to spawn an ambush to increase difficulty
//! or spawn relief items/weaker enemies to help a struggling player recover.

use crate::PlayerState;

/// The action the Director decides to take to influence the game state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawns a difficult enemy or ambush to challenge the player.
    SpawnAmbush,
    /// Spawns weaker enemies or relief items to help the player recover.
    SpawnRelief,
    /// Does nothing, allowing the current pacing to continue.
    Maintain,
}

/// The system that monitors the player and directs the pacing of the game.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new, empty AI Director.
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

    /// Evaluates the player's current state and determines the next action.
    ///
    /// If the player's health is above 80, the director will spawn an ambush.
    /// If the health is below 30, it will spawn relief.
    /// Otherwise, it maintains the current pacing.
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
