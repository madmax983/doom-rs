//! The `director` module provides an AI director system for dynamic difficulty adjustment.
//!
//! This module analyzes player metrics (such as health) to dynamically adjust the game's
//! pacing and challenge. It ensures the player is kept in a "flow state" by spawning
//! ambushes when they are performing too well, or providing relief when they are struggling.

use crate::PlayerState;

/// An action determined by the AI Director to alter game pacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn enemies to increase difficulty (player is doing well).
    SpawnAmbush,
    /// Provide health or resources (player is struggling).
    SpawnRelief,
    /// Keep the current pacing (player is in the target challenge zone).
    Maintain,
}

/// Analyzes player state and dictates pacing adjustments.
///
/// The `AiDirector` is a stateless system (currently) that monitors the `PlayerState`
/// on each tick and issues a `DirectorAction` based on predefined thresholds.
///
/// ## Examples
///
/// ```
/// # use doom_game::director::{AiDirector, DirectorAction};
/// # use doom_game::PlayerState;
/// # use doom_game::mobj::MobjHandle;
/// # use doom_types::limits::MAX_HEALTH;
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
///
/// // If the player is at full health, the director will spawn an ambush.
/// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
///
/// // If the player is near death, the director will provide relief.
/// player.set_health_capped(10, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's state and returns an action to adjust difficulty.
    ///
    /// Currently, this operates on simple health thresholds:
    /// - Health > 80: Returns `DirectorAction::SpawnAmbush`
    /// - Health < 30: Returns `DirectorAction::SpawnRelief`
    /// - Otherwise: Returns `DirectorAction::Maintain`
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
