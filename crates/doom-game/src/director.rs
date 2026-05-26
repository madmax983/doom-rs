//! AI Director for adjusting game difficulty dynamically.
//!
//! The `AiDirector` analyzes player state (like health) and determines
//! whether to spawn more enemies, provide relief items, or maintain
//! the current pacing.
//!
//! # Examples
//!
//! ```
//! use doom_game::director::{AiDirector, DirectorAction};
//! use doom_game::PlayerState;
//! use doom_game::mobj::MobjHandle;
//! use doom_types::limits::MAX_HEALTH;
//!
//! let mut director = AiDirector::new();
//! let mut player = PlayerState::pistol_start(MobjHandle::NULL);
//!
//! // Healthy player gets ambushed
//! player.set_health_capped(100, MAX_HEALTH);
//! assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
//! ```

use crate::PlayerState;

/// The action recommended by the AI director for the current tic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn enemies to increase pressure on a healthy player.
    SpawnAmbush,
    /// Spawn health or ammo to help a struggling player.
    SpawnRelief,
    /// Keep the current pacing without intervening.
    Maintain,
}

/// Analyzes player state and determines dynamic difficulty adjustments.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new AI Director.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's health and returns a recommended action.
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
