use crate::PlayerState;

/// The AI Director evaluates the current state of the game, particularly player
/// health, and determines the overarching pacing action to apply to the gameplay.
///
/// It aims to maintain player engagement by modulating difficulty dynamically.
///
/// ## Examples
///
/// ```rust
/// use doom_game::director::{AiDirector, DirectorAction};
/// use doom_game::PlayerState;
/// use doom_game::mobj::MobjHandle;
///
/// let mut director = AiDirector::new();
/// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
/// player.set_health_capped(100, 100);
///
/// // Player is doing well, the director will spawn an ambush!
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Indicates the player is performing exceptionally well and the difficulty
    /// should be increased by spawning additional enemies or traps.
    SpawnAmbush,
    /// Indicates the player is struggling (e.g., low health) and should be
    /// provided with relief, such as health packs, ammo, or fewer enemies.
    SpawnRelief,
    /// Indicates the current pacing is optimal and no significant interventions
    /// are needed.
    Maintain,
}

/// The main logic component for the AI Director feature.
///
/// It holds state (if needed) across ticks to manage long-term pacing and intensity
/// levels, deciding when to push the player harder or ease off based on their
/// current `PlayerState`.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector` with default settings.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_game::director::AiDirector;
    ///
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's status and determines the next pacing action.
    ///
    /// The director checks the player's health to decide the next `DirectorAction`.
    /// High health triggers an ambush, low health triggers relief, and medium
    /// health maintains the current state.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_game::director::{AiDirector, DirectorAction};
    /// use doom_game::PlayerState;
    /// use doom_game::mobj::MobjHandle;
    ///
    /// let mut director = AiDirector::new();
    /// let mut player = PlayerState::pistol_start(MobjHandle::NULL);
    /// player.set_health_capped(20, 100);
    ///
    /// // Player is low on health, the director will offer relief.
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
