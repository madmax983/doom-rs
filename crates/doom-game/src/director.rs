use crate::PlayerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Represents the discrete actions the `AiDirector` can mandate.
pub enum DirectorAction {
    SpawnAmbush,
    SpawnRelief,
    Maintain,
}

/// `AiDirector` observes the player's condition and decides what action
/// the game should take to keep the tension high but fair.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector` instance.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::AiDirector;
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Determines the next action the AI director should take based on the player's state.
    ///
    /// Evaluates the player's current health to decide whether to spawn an ambush
    /// (if health is high), provide relief (if health is low), or simply maintain the
    /// current state of affairs.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::{AiDirector, DirectorAction, PlayerState};
    /// use doom_game::mobj::MobjHandle;
    /// use doom_types::limits::MAX_HEALTH;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    /// player.set_health_capped(100, MAX_HEALTH);
    ///
    /// // The director spawns an ambush because health is high.
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
