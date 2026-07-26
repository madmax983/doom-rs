//! AI Director for dynamic difficulty adjustment.
//!
//! The AI Director monitors the player's health and decides whether to
//! increase or decrease the pressure by spawning ambushes or relief items.
//! This ensures the game remains engaging by preventing it from becoming
//! too easy or too hard.

use crate::PlayerState;

/// Actions that the AI Director can take to adjust difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn enemies to increase pressure when the player is doing too well.
    SpawnAmbush,
    /// Spawn health or ammo to provide relief when the player is struggling.
    SpawnRelief,
    /// Do nothing; the current difficulty is appropriate.
    Maintain,
}

/// The AI Director monitors player state and determines the next course of action.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector`.
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

    /// Evaluates the player's state and returns the appropriate `DirectorAction`.
    ///
    /// If the player's health is above 80, it will return `SpawnAmbush`.
    /// If the player's health is below 30, it will return `SpawnRelief`.
    /// Otherwise, it returns `Maintain`.
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
