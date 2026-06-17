//! The "AI Director" dynamic difficulty subsystem.
//!
//! Unlike the classic deterministic Doom engine, this crate introduces an intelligent
//! Director that monitors player performance and adjusts the game's tension dynamically.
//! Instead of static monster closets, the Director can choose to apply pressure when
//! the player is thriving or offer brief moments of relief when the player is struggling.

use crate::PlayerState;

/// The verdict rendered by the AI Director for the current tic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is performing too well; spawn additional enemies to apply pressure.
    SpawnAmbush,
    /// The player is near death; spawn health items or delay enemy aggression.
    SpawnRelief,
    /// The current pacing is acceptable; do not intervene.
    Maintain,
}

/// The orchestrator of dynamic difficulty.
///
/// The `AiDirector` observes the player's state over time and issues commands
/// to adjust the spawning subsystem, ensuring the game remains engaging without
/// becoming overwhelmingly frustrating or boring.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current condition and returns the recommended action.
    ///
    /// The Director primarily focuses on the player's health pool to determine
    /// if the pacing is too easy or too hard.
    ///
    /// # Details
    ///
    /// - Health > 80: The player is practically untouched. The Director will attempt to `SpawnAmbush`.
    /// - Health < 30: The player is critically wounded. The Director will attempt to `SpawnRelief`.
    /// - Otherwise: The Director will `Maintain` the current tension.
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
    /// // The player is bleeding out!
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
