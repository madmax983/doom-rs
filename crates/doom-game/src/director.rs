//! AI Director for dynamic difficulty adjustment.
//!
//! The AI Director dynamically responds to the player's performance and state,
//! allowing the game to change its pacing and difficulty on the fly. By observing
//! metrics like health, ammo, and combat intensity, the Director decides whether
//! to spawn more enemies, spawn health/ammo relief, or maintain the current state.
//!
//! # Purpose
//! This system exists to prevent the player from getting completely stuck on low health,
//! or breezing through the game with too many resources. It provides a feedback loop
//! that attempts to keep the player in a "flow state".
//!
//! # Examples
//! ```
//! use doom_game::director::{AiDirector, DirectorAction};
//! use doom_game::PlayerState;
//! use doom_game::mobj::MobjHandle;
//! use doom_types::limits::MAX_HEALTH;
//!
//! let mut director = AiDirector::new();
//! let mut player = PlayerState::pistol_start(MobjHandle::NULL);
//!
//! // Player is doing great, director spawns an ambush!
//! player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
//! assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
//! ```

use crate::PlayerState;

/// Actions the Director can take to adjust the game state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing too well, spawn an ambush or increase difficulty.
    SpawnAmbush,
    /// The player is struggling, spawn health/ammo or reduce pressure.
    SpawnRelief,
    /// The player is in the "flow state", maintain the current difficulty.
    Maintain,
}

/// The main AI Director state machine.
///
/// It tracks the game state over time to make intelligent decisions
/// about pacing and difficulty.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new, uninitialized AI Director.
    ///
    /// # Examples
    /// ```
    /// use doom_game::director::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current state and decides the next action.
    ///
    /// The Director looks at the player's health to determine if they need
    /// an ambush (health > 80), relief (health < 30), or to maintain the
    /// current state.
    ///
    /// # Examples
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // Simulate a struggling player
    /// player.set_health_capped(20, MAX_HEALTH);
    ///
    /// // The director should provide relief
    /// let action = director.tick(&player);
    /// assert_eq!(action, DirectorAction::SpawnRelief);
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
