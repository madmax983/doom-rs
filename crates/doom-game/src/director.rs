//! Dynamic difficulty adjustment system.
//!
//! The AI Director monitors player performance and health metrics to adjust the game's
//! pacing and challenge dynamically. It acts as an invisible orchestrator, ensuring
//! the player experiences a continuous flow of tension and release.

use crate::PlayerState;

/// Actions the AI Director can take to adjust difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Increase tension by spawning additional monsters to ambush the player.
    SpawnAmbush,
    /// Decrease tension by spawning health or ammo items to assist the player.
    SpawnRelief,
    /// Maintain current difficulty, neither increasing nor decreasing tension.
    Maintain,
}

/// The system that monitors player state to dictate pacing adjustments.
pub struct AiDirector;

impl AiDirector {
    /// Initializes a new AI Director.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::director::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current condition and decides on a pacing action.
    ///
    /// The evaluation is primarily based on the player's health:
    /// - `Health > 80`: Spawns an ambush to increase challenge.
    /// - `Health < 30`: Spawns relief items to help the player recover.
    /// - `Otherwise`: Maintains the current state.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // Simulate high health leading to an ambush.
    /// player.set_health_capped(100, 100);
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
