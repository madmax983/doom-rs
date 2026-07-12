//! The Artificial Intelligence Director for the Doom engine.
//!
//! This module acts as the "dungeon master" during a level, monitoring the player's performance
//! and adjusting the game's difficulty dynamically. It analyzes the player's health and dictates
//! high-level strategy decisions (like spawning ambushes or health relief).

use crate::PlayerState;

/// The specific action chosen by the AI Director based on current player state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing very well (high health) and needs an extra challenge.
    SpawnAmbush,
    /// The player is severely injured (low health) and should be given a break or supplies.
    SpawnRelief,
    /// The player is at an average performance level; continue normal operation.
    Maintain,
}

/// Evaluates the player's performance and chooses an appropriate [`DirectorAction`].
///
/// The Director monitors the player's health every tic and calculates the optimal
/// game pacing. It returns an action that the core game loop can then execute
/// (e.g., spawning new monsters or dropping health pickups).
pub struct AiDirector;

impl AiDirector {
    /// Constructs a new, idle AI Director.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the given player state and returns the next recommended action.
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
    ///
    /// // If the player is doing great, the director spawns an ambush.
    /// player.set_health_capped(100, MAX_HEALTH);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    ///
    /// // If the player is nearly dead, the director spawns relief.
    /// player.set_health_capped(15, MAX_HEALTH);
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
