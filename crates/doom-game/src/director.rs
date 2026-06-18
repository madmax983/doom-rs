//! AI Director for dynamic game pacing.
//!
//! The `AiDirector` analyzes the player's current state (e.g., health, ammo)
//! and determines the appropriate `DirectorAction` to maintain engaging gameplay.
//! Instead of static enemy spawns, the director adjusts difficulty on the fly.
//!
//! # Context
//! This module provides the `AiDirector`, a high-level manager that decides
//! whether to spawn health pickups (`SpawnRelief`), ambush the player (`SpawnAmbush`),
//! or do nothing (`Maintain`).
//!
//! # Details
//! The `tick` function evaluates the `PlayerState`. If health is above 80, it triggers an ambush.
//! If health drops below 30, it provides relief. Otherwise, it maintains the current pacing.
//!
//! # Examples
//! ```
//! use doom_game::director::{AiDirector, DirectorAction};
//! use doom_game::PlayerState;
//! use doom_game::mobj::MobjHandle;
//!
//! let mut director = AiDirector::new();
//! let mut player = PlayerState::pistol_start(MobjHandle::NULL);
//!
//! // Simulate low health
//! player.set_health_capped(20, 100);
//!
//! let action = director.tick(&player);
//! assert_eq!(action, DirectorAction::SpawnRelief);
//! ```

use crate::PlayerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    SpawnAmbush,
    SpawnRelief,
    Maintain,
}

pub struct AiDirector;

impl AiDirector {
    pub fn new() -> Self {
        Self
    }

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
