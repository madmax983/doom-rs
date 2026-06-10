//! AI Director for managing gameplay pacing and difficulty dynamically.
//!
//! The `director` module provides the `AiDirector`, a high-level system that
//! monitors the player's performance and adjusts the game's intensity.
//! Inspired by the Left 4 Dead AI Director, it analyzes player health,
//! ammo, and pacing to spawn ambushes or provide relief, ensuring a
//! balanced and engaging experience.

use crate::PlayerState;

/// Actions the AI Director can take to adjust gameplay pacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn an ambush of monsters to increase intensity.
    SpawnAmbush,
    /// Spawn health or ammo to provide relief to a struggling player.
    SpawnRelief,
    /// Maintain the current pacing; take no action.
    Maintain,
}

/// The AI Director monitors the player's state and determines the next action.
///
/// ## Examples
///
/// ```
/// use doom_game::PlayerState;
/// use doom_game::director::{AiDirector, DirectorAction};
/// use doom_game::mobj::MobjHandle;
/// use doom_types::limits::MAX_HEALTH;
///
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
/// player.set_health_capped(10, MAX_HEALTH);
///
/// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
/// ```
pub struct AiDirector;

impl AiDirector {
    #[allow(missing_docs)]
    pub fn new() -> Self {
        Self
    }

    /// Evaluate the player's state and decide on the next pacing action.
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
