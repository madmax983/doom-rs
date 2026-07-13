//! The Artificial Intelligence Director module.
//!
//! The AI Director acts as the invisible game master, monitoring the player's
//! state and dynamically adjusting the game's difficulty to maintain a flow state
//! between stress and relief. Instead of static enemy placements, the Director
//! can spawn reinforcements if the player is dominating, or drop health if the
//! player is close to death.
//!
//! # How it works
//!
//! The [`AiDirector`] is ticked every frame (or every N frames) and observes
//! the [`PlayerState`]. Based on various metrics (currently just health), it
//! emits a [`DirectorAction`] which the main game loop then executes.

use crate::PlayerState;

/// The specific action the AI Director has decided the game should take.
///
/// This enum represents the high-level strategic decision made by the Director.
/// The game engine is responsible for interpreting these actions (e.g., finding
/// a suitable spawn point for an ambush or relief package).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing too well. The game should spawn enemies to increase pressure.
    SpawnAmbush,
    /// The player is struggling. The game should spawn health/ammo to prevent frustration.
    SpawnRelief,
    /// The current pacing is optimal. The game should proceed normally without intervention.
    Maintain,
}

/// The stateful orchestrator that monitors the player and controls game pacing.
///
/// The Director exists to prevent the game from becoming boring or overly punishing.
/// It observes the player's performance metrics and decides when to intervene.
///
/// # Examples
///
/// ```
/// use doom_game::{AiDirector, DirectorAction, PlayerState};
/// use doom_game::mobj::MobjHandle;
/// use doom_types::limits::MAX_HEALTH;
///
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
///
/// // If the player is at full health, the director will spawn an ambush to increase tension.
/// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
///
/// // If the player gets severely injured, the director will spawn relief items.
/// player.set_health_capped(20, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new AI Director with default pacing parameters.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the current state of the player and determines the next action.
    ///
    /// This function analyzes the `player`'s health to gauge their current stress level.
    /// It returns a [`DirectorAction`] that the game engine should execute.
    ///
    /// # Arguments
    ///
    /// * `player` - The current state of the player, used to assess their stress level.
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
