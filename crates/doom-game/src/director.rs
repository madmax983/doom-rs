//! AI Director for adjusting game difficulty dynamically.
//!
//! # The Grand Puppeteer
//! The AI Director monitors the [`PlayerState`] during gameplay to apply pressure or provide relief.
//! Instead of static, predictable monster spawns, the director attempts to maintain an engaging
//! flow by spawning ambushes when the player is healthy, and scaling back or providing health
//! items when the player is near death.
//!
//! # Mechanics
//! The [`AiDirector`] is ticked every frame and returns a [`DirectorAction`] based on player health.
//! This returned action is then consumed by the spawn managers to dictate what entities are placed
//! into the level.

use crate::PlayerState;

/// The action dictated by the AI Director to alter the game's spawning mechanics.
///
/// Every tick, the director analyzes the game state and outputs one of these directives,
/// determining whether the engine should ramp up difficulty or pull its punches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is performing too well (health > 80%). The director will attempt to spawn
    /// additional enemies or tougher variants to apply pressure and maintain tension.
    SpawnAmbush,
    /// The player is struggling (health < 30%). The director will attempt to spawn
    /// fewer enemies, or spawn health packs and ammo to give the player a chance to recover.
    SpawnRelief,
    /// The player is in a balanced state. The engine should proceed with the default
    /// spawn tables without any special intervention.
    Maintain,
}

/// The orchestrator of dynamic difficulty.
///
/// The [`AiDirector`] evaluates the current [`PlayerState`] (specifically health) to decide
/// what the engine should do next to maintain an optimal flow state for the player.
///
/// # Examples
///
/// ```
/// use doom_game::director::AiDirector;
///
/// let director = AiDirector::new();
/// ```
pub struct AiDirector;

impl AiDirector {
    /// Creates a new [`AiDirector`] ready to monitor the game state.
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

    /// Evaluates the player's current health and returns a [`DirectorAction`].
    ///
    /// The tick function is the core decision-making loop. It returns:
    /// - `SpawnAmbush` if health > 80
    /// - `SpawnRelief` if health < 30
    /// - `Maintain` for anything in between
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
    /// // Simulate a healthy player
    /// player.set_health_capped(100, MAX_HEALTH);
    /// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    ///
    /// // Simulate a dying player
    /// player.set_health_capped(15, MAX_HEALTH);
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
