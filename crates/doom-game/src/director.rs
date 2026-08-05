//! Dynamic difficulty adjustment algorithm.
//!
//! The AiDirector monitors the player's performance during gameplay
//! and adjusts the hostility of the environment dynamically. Similar to the
//! "Left 4 Dead" director concept, it measures the player's `health`
//! and chooses an appropriate [`DirectorAction`] to spawn relief items if the player is
//! struggling, or ambushes if they are cruising.

use crate::PlayerState;

/// The action the [`AiDirector`] decides to take on this tic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing well; spawn additional enemies or traps.
    SpawnAmbush,
    /// The player is struggling; spawn health or ammo.
    SpawnRelief,
    /// The player is in a normal state; do not interfere.
    Maintain,
}

/// A lightweight state machine that monitors the player and drives the game's dynamic difficulty.
///
/// ## Examples
///
/// ```
/// use doom_game::director::{AiDirector, DirectorAction};
/// use doom_game::player::PlayerState;
/// use doom_game::mobj::MobjHandle;
/// use doom_types::limits::MAX_HEALTH;
///
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
///
/// // Player is fully healed, so the director gets aggressive.
/// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
///
/// // Player gets hurt badly, so the director spawns relief.
/// player.set_health_capped(10, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new, resting [`AiDirector`].
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the given [`PlayerState`] and returns the chosen [`DirectorAction`].
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
