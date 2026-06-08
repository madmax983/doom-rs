//! AI Director logic for dynamic enemy spawns.
//!
//! Controls enemy spawns based on the player's health to create dynamic difficulty.

use crate::PlayerState;

/// Actions the director can take on a tick.
///
/// # Examples
/// ```
/// use doom_game::director::DirectorAction;
///
/// let action = DirectorAction::Maintain;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn enemies to ambush the player because they have too much health.
    SpawnAmbush,
    /// Spawn relief items or weaker enemies because the player has low health.
    SpawnRelief,
    /// Do nothing special.
    Maintain,
}

/// The AI Director.
///
/// # Examples
/// ```
/// use doom_game::director::AiDirector;
///
/// let director = AiDirector::new();
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Instantiates a new director ready to watch over the player's survival.
    ///
    /// The director has no state of its own, but serves as the orchestrator for
    /// dynamically adjusting the game's cruelty in response to the player's performance.
    ///
    /// # Examples
    /// ```
    /// use doom_game::director::AiDirector;
    ///
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current distress level and decides on the next intervention.
    ///
    /// If the player is cruising along comfortably (health > 80), the director will
    /// spawn an ambush to keep things interesting. Conversely, if the player is
    /// clinging to life (health < 30), it will spawn relief items to give them a fighting chance.
    ///
    /// # Examples
    /// ```
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::player::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    /// player.set_health_capped(10, MAX_HEALTH);
    ///
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
