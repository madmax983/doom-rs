//! The AI Director.
//!
//! The AI Director dynamically adjusts the pacing and difficulty of the game by monitoring
//! the player's performance metrics (like health) and determining the appropriate response.
//! It acts as the "manager" that decides whether to spawn ambushes to increase pressure
//! or provide relief items when the player is struggling.

use crate::PlayerState;

/// The action the AI Director has decided to take this tic based on the player's status.
///
/// # Examples
///
/// ```
/// use doom_game::director::DirectorAction;
///
/// let action = DirectorAction::SpawnAmbush;
/// assert_eq!(action, DirectorAction::SpawnAmbush);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing well; increase pressure by spawning an ambush.
    SpawnAmbush,
    /// The player is struggling; provide relief items like health or ammo.
    SpawnRelief,
    /// The player is in a balanced state; maintain current pacing.
    Maintain,
}

/// Monitors player state and makes dynamic difficulty adjustment decisions.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new AI Director instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_game::director::AiDirector;
    ///
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current state and returns the appropriate action.
    ///
    /// The director checks the player's health threshold:
    /// - `health > 80`: `SpawnAmbush`
    /// - `health < 30`: `SpawnRelief`
    /// - Otherwise: `Maintain`
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
    /// // High health triggers an ambush
    /// player.set_health_capped(100, MAX_HEALTH);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    ///
    /// // Low health triggers relief
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
