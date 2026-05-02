//! The AI Director: Dynamic Difficulty Adjustment
//!
//! The `AiDirector` monitors the player's performance (primarily their current health)
//! and dynamically adjusts the game's difficulty on the fly.
//!
//! Instead of relying entirely on static, pre-placed WAD entities, the engine can
//! invoke the director to decide if the map should spawn surprise ambushes or provide
//! emergency relief.
//!
//! # The Story
//! When a player is completely stacked (health > 80), the director considers them "bored"
//! and will attempt to spice things up via [`DirectorAction::SpawnAmbush`]. Conversely,
//! a bleeding player (health < 30) is shown mercy via [`DirectorAction::SpawnRelief`].
//! Otherwise, the game maintains the status quo.

use crate::PlayerState;

/// The action the AI Director has decided to take this tick.
///
/// This enum is the output of the director's analysis. The main game loop will
/// match on this result and mutate the world state accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn an ambush because the player is doing too well.
    SpawnAmbush,
    /// Spawn relief (like medkits or ammo) because the player is severely injured.
    SpawnRelief,
    /// Do nothing; maintain the current pacing.
    Maintain,
}

/// The AI Director analyzes the player and orchestrates game pacing.
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
/// // Player is doing great! The director decides to strike.
/// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
///
/// // Player is almost dead. The director shows mercy.
/// player.set_health_capped(10, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new AI Director.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current state and decides the next action.
    ///
    /// ## Panics
    /// This function does not panic.
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
