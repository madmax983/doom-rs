//! Dynamic pacing system for controlling game intensity.
//!
//! The `director` module acts as a backstage manager for the player's experience.
//! Rather than just placing static items and enemies into a level, the [`AiDirector`]
//! continually monitors the player's performance—chiefly their health pool—and dynamically
//! orchestrates encounters to maximize tension without causing frustration.
//! It tries to push the player to the brink, and then offers a reprieve.
//!
//! If the player is cruising along with full health, the director will spawn an ambush.
//! Conversely, if the player is barely clinging to life, it will drop relief items
//! so they can recover and get back into the fight.

use crate::PlayerState;

/// The resulting action the director decided to take this tic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing too well; spawn enemies or hazards to increase pressure.
    SpawnAmbush,
    /// The player is near death; spawn health or ammo to give them a fighting chance.
    SpawnRelief,
    /// The pacing is currently optimal; do not intervene.
    Maintain,
}

/// The stateful orchestrator of game pacing.
///
/// An `AiDirector` is instantiated once per level and ticked every frame. It observes
/// the [`PlayerState`] and returns a [`DirectorAction`] dictating how the world should
/// mutate around the player to keep the tension curve ideal.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector` instance.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current status and decides on an intervention.
    ///
    /// This is called every tic. The director will analyze the player's current `health`
    /// to determine the appropriate response.
    ///
    /// # Pacing Logic
    /// - **Health > 80**: The player is too comfortable. Returns [`DirectorAction::SpawnAmbush`].
    /// - **Health < 30**: The player is in severe danger. Returns [`DirectorAction::SpawnRelief`].
    /// - **Otherwise**: The tension is balanced. Returns [`DirectorAction::Maintain`].
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_game::player::PlayerState;
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    ///
    /// // The player just picked up a soul sphere and is fully healed.
    /// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
    ///
    /// // The director notices they are doing great, and decides to punish them.
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
