//! The `director` module provides an AI director for the game, deciding
//! what actions to take based on the player's current status (like health).
//!
//! # Architecture
//! The director evaluates the player's health and decides whether to spawn
//! an ambush, provide relief, or maintain the current state.
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
//! // The director provides relief when the player is near death
//! player.set_health_capped(10, MAX_HEALTH);
//! assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
//! ```

use crate::PlayerState;

/// The action chosen by the AI director to affect the game state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn an ambush to challenge a healthy player.
    SpawnAmbush,
    /// Spawn relief (like health or ammo) to aid a dying player.
    SpawnRelief,
    /// Maintain the current state (do nothing special).
    Maintain,
}

/// The AI director, a higher-level logic system that monitors the player
/// and decides whether to spawn ambushes or provide relief.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_game::director::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's status and returns a [`DirectorAction`].
    ///
    /// If the player's health is above 80, it will spawn an ambush.
    /// If the player's health is below 30, it will spawn relief.
    /// Otherwise, it will maintain the current state.
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
    /// player.set_health_capped(100, MAX_HEALTH);
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
