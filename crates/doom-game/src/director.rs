//! The AI Director manages combat pacing and dynamic difficulty adjustments.
//!
//! Rather than spawning all monsters immediately or relying on static triggers,
//! the [`AiDirector`] monitors the player's performance (specifically their health)
//! and makes real-time decisions about what type of encounters to generate next.
//! This ensures that the player is always challenged but not overwhelmed, creating
//! a more engaging "rubber-band" difficulty curve.
//!
//! ## Examples
//!
//! ```
//! use doom_game::director::{AiDirector, DirectorAction};
//! use doom_game::player::PlayerState;
//! use doom_game::mobj::MobjHandle;
//! use doom_types::limits::MAX_HEALTH;
//!
//! let mut director = AiDirector::new();
//! let mut player = PlayerState::pistol_start(MobjHandle::NULL);
//!
//! // If the player is doing very well, spawn an ambush
//! player.set_health_capped(100, MAX_HEALTH);
//! assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
//!
//! // If the player is near death, spawn relief (like health items or fewer monsters)
//! player.set_health_capped(15, MAX_HEALTH);
//! assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
//! ```

use crate::PlayerState;

/// The action decided by the [`AiDirector`] for the current game tick.
///
/// This enum represents the high-level strategy the director wants to employ
/// to keep the player engaged. The game engine translates these intents into
/// concrete events like spawning a Cacodemon or dropping a Medikit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing very well; increase pressure by spawning a difficult encounter.
    SpawnAmbush,
    /// The player is struggling; decrease pressure by providing health, ammo, or easier enemies.
    SpawnRelief,
    /// The player is in the "sweet spot" of difficulty; continue with the current pacing.
    Maintain,
}

/// A dynamic pacing manager that monitors the [`PlayerState`] and adjusts encounters.
///
/// The `AiDirector` is essentially a state machine that outputs a [`DirectorAction`]
/// based on the player's current status. Currently, it evaluates the player's health,
/// but future iterations could consider ammo reserves, recent kill combos, or speed.
pub struct AiDirector;

impl AiDirector {
    /// Creates a new `AiDirector` with default pacing parameters.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current state and determines the next pacing action.
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
    /// // Simulate the player taking heavy damage
    /// player.set_health_capped(20, MAX_HEALTH);
    ///
    /// // The director should notice the low health and offer relief
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
