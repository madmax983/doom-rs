//! The AI Director Module
//!
//! This module acts as the dungeon master of the simulation. Instead of relying
//! entirely on static monster placement, the [`AiDirector`] analyzes the player's
//! current status (such as their remaining health) and makes dynamic decisions about
//! the pacing of the game.
//!
//! Why does this exist? To maintain tension! If the player is effortlessly tearing
//! through demons with full health, the director will spawn an ambush to keep them
//! engaged. If the player is on death's door, it backs off and spawns relief items
//! to give them a fighting chance.

use crate::PlayerState;

/// The action dictated by the [`AiDirector`] for the current game tick.
///
/// This enum represents the high-level pacing decision made by the director.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing too well. Spawn enemies or increase aggression!
    SpawnAmbush,
    /// The player is struggling. Provide health or ammo to keep them in the fight.
    SpawnRelief,
    /// The tension is perfectly balanced. Do nothing.
    Maintain,
}

/// The orchestrator of dynamic pacing and difficulty.
///
/// The `AiDirector` monitors the game state and decides how the world should react
/// to the player's performance. It is designed to be lightweight and runs on every
/// simulation tick.
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
/// // A player with full health should trigger an ambush to increase difficulty.
/// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
///
/// // If the player is badly hurt, the director backs off.
/// player.set_health_capped(10, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new, blank-slate AI director.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current state and decides the next action.
    ///
    /// This is the core logic loop of the director. It reads the player's health
    /// and adjusts the pacing action accordingly.
    ///
    /// ## Panics
    ///
    /// This function will not panic under normal circumstances.
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
