//! The Artificial Intelligence Director orchestrates dynamic difficulty by analyzing
//! the player's performance and adjusting the game's intensity.
//!
//! Rather than relying on a static map population, the AI Director monitors the player's
//! state (such as their current health) during each simulation tick. By determining if the
//! player is thriving or struggling, it issues a [`DirectorAction`] which instructs the
//! engine to spawn ambushes, offer relief items, or maintain the current pressure.
//!
//! # Examples
//!
//! ```rust
//! use doom_game::director::{AiDirector, DirectorAction};
//! use doom_game::player::PlayerState;
//! use doom_game::mobj::MobjHandle;
//! use doom_types::limits::MAX_HEALTH;
//!
//! let mut director = AiDirector::new();
//! let mut player = PlayerState::pistol_start(MobjHandle::NULL);
//!
//! // A fully healed player is ready for a challenge.
//! player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
//! assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
//! ```

use crate::PlayerState;

/// The tactical decision issued by the [`AiDirector`] for a given simulation tick.
///
/// This enum dictates how the engine should alter the current map's pacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing exceptionally well. The engine should spawn additional monsters
    /// or spring a trap to increase the pressure and maintain engagement.
    SpawnAmbush,
    /// The player is in critical condition. The engine should reduce aggression or spawn
    /// health pickups to provide a lifeline.
    SpawnRelief,
    /// The player is in a balanced state. The engine should continue standard behavior
    /// without intervening.
    Maintain,
}

/// The hidden mastermind that regulates game pacing and dynamic difficulty.
///
/// The `AiDirector` analyzes the [`PlayerState`] each frame and decides whether the current
/// situation is too easy, too punishing, or just right.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new AI Director with a fresh slate.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current status and issues a pacing command.
    ///
    /// This function acts as the core decision loop. It checks the player's health threshold
    /// and issues a [`DirectorAction`] to either increase pressure if the player is healthy,
    /// provide relief if they are near death, or maintain the status quo.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use doom_game::director::{AiDirector, DirectorAction};
    /// # use doom_game::player::PlayerState;
    /// # use doom_game::mobj::MobjHandle;
    /// # use doom_types::limits::MAX_HEALTH;
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // If the player drops below 30 health, the director will send relief.
    /// player.set_health_capped(20, MAX_HEALTH);
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
