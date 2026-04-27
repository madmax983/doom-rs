//! The AI Director — a high-level manager that monitors player performance.
//!
//! The AI Director is an invisible hand that watches the player's health, ammo,
//! and general combat flow, adjusting the game's difficulty dynamically. It uses
//! the `tick` method once per game tic to evaluate the current `PlayerState` and
//! determine a strategic `DirectorAction`.
//!
//! This module brings an element of pacing to the demonic slaughter, ensuring
//! the player is continually challenged without being unfairly crushed.

use crate::PlayerState;

/// The strategic action decided by the AI Director for the current tic.
///
/// This enum represents the high-level intent of the Director. The actual
/// spawning of enemies or items is handled by the main game loop based on
/// this returned action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing well; the Director decides to spawn more enemies
    /// to increase the pressure and maintain challenge.
    SpawnAmbush,
    /// The player is severely wounded or low on resources; the Director decides
    /// to spawn health or ammo (or hold back enemy spawns) to provide relief.
    SpawnRelief,
    /// The player's performance is average; the Director takes no special action
    /// and maintains the current difficulty curve.
    Maintain,
}

/// The AI Director engine that calculates dynamic difficulty adjustments.
///
/// It holds no internal state between tics (currently), acting purely as a
/// stateless evaluator of the `PlayerState`.
pub struct AiDirector;

impl AiDirector {
    /// Constructs a new `AiDirector`.
    ///
    /// ## Examples
    /// ```
    /// use doom_game::director::AiDirector;
    ///
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current condition and returns a `DirectorAction`.
    ///
    /// This is called once per game tic. It checks the player's health to decide
    /// whether to apply pressure (`SpawnAmbush`), offer help (`SpawnRelief`), or
    /// do nothing (`Maintain`).
    ///
    /// ## Examples
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::player::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // Simulate a healthy player
    /// player.set_health_capped(100, MAX_HEALTH);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    ///
    /// // Simulate a wounded player
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
