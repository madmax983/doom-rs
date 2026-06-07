//! The invisible hand orchestrating the game's tension.
//!
//! # The Story
//! The `AiDirector` monitors the current `PlayerState` to determine how to adjust
//! the difficulty on the fly. Instead of static spawns, the director reacts dynamically
//! to keep the player engaged—spawning ambushes when the player is fully stocked and
//! dropping relief (health or ammo) when the player is near death.
//!
//! This module provides the logic for determining which [`DirectorAction`] should be
//! taken during each game tic.

use crate::PlayerState;

/// The action the `AiDirector` recommends taking based on the current player state.
///
/// ## Examples
/// ```
/// use doom_game::director::DirectorAction;
///
/// let action = DirectorAction::Maintain;
/// assert_eq!(action, DirectorAction::Maintain);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Recommended when the player is doing well and needs a challenge.
    SpawnAmbush,
    /// Recommended when the player is near death and needs assistance.
    SpawnRelief,
    /// Recommended when the tension is balanced and no intervention is required.
    Maintain,
}

/// A stateless orchestrator that decides how to adjust gameplay tension.
///
/// ## Examples
/// ```
/// use doom_game::director::AiDirector;
///
/// let director = AiDirector::new();
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new, default `AiDirector`.
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

    /// Evaluates the player's current health and returns a recommended `DirectorAction`.
    ///
    /// ## Examples
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // Simulate high health
    /// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    ///
    /// // Simulate low health
    /// player.set_health_capped(10, MAX_HEALTH);
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
