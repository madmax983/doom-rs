use crate::PlayerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Represents the dynamic difficulty adjustments decided by the `AiDirector`.
pub enum DirectorAction {
    /// Spawns additional enemies because the player is doing too well.
    SpawnAmbush,
    /// Spawns health or ammo because the player is struggling.
    SpawnRelief,
    /// Takes no action; the current difficulty is optimal.
    Maintain,
}

/// A rudimentary AI director that monitors player performance to adjust difficulty dynamically.
pub struct AiDirector;

impl AiDirector {
    /// Awakens the AI Director to monitor the battlefield.
    ///
    /// The director starts in a dormant state and must be explicitly ticked to
    /// start analyzing the player's health and spawning dynamic encounters.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::director::AiDirector;
    ///
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Passes judgment on the player's performance.
    ///
    /// The director reads the player's vital signs and returns an action.
    /// If the player is thriving (high health), it will attempt to overwhelm them.
    /// If the player is dying, it will deploy relief supplies.
    ///
    /// ## Examples
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
    /// player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
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
