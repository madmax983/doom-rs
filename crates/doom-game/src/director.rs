//! Dynamic AI Director for adjusting game pacing and difficulty.
//!
//! The `AiDirector` monitors the player's current status (like health) and
//! decides how the game should react. It can spawn ambushes if the player
//! is doing too well, or spawn relief items if the player is struggling.

use crate::PlayerState;

/// The action the AI Director has decided to take this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn additional enemies to challenge a healthy player.
    SpawnAmbush,
    /// Spawn health or ammo to assist a struggling player.
    SpawnRelief,
    /// Do nothing; the current difficulty is appropriate.
    Maintain,
}

/// The AI Director monitors player state and influences the game world.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's state and determines the next action.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::player::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // If the player is doing very well (full health 100)...
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    ///
    /// // If the player is near death (health drops below 30)...
    /// player.apply_damage(80);
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
