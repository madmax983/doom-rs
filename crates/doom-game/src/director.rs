//! The AI Director.
//!
//! While the standard Doom game loop relies on predefined spawn points and
//! static placement of monsters and health packs, the [`AiDirector`] introduces
//! dynamic tension management.
//!
//! By monitoring the player's current health and state, the Director determines
//! if the game should ease up to prevent a frustrating death, or ramp up the
//! pressure when the player is coasting.
//!
//! # The Tension Cycle
//!
//! 1. **Relief**: If the player is clinging to life (health < 30), the Director
//!    suggests a [`DirectorAction::SpawnRelief`] to offer a fighting chance.
//! 2. **Ambush**: If the player is heavily armored and healthy (health > 80),
//!    the Director may suggest a [`DirectorAction::SpawnAmbush`] to keep
//!    adrenaline high.
//! 3. **Maintain**: In the middle ground, the Director simply watches
//!    ([`DirectorAction::Maintain`]), allowing the organic level design to
//!    dictate the pace.

use crate::PlayerState;

/// An action determined by the [`AiDirector`] for the current tic.
///
/// This enum represents the Director's high-level intent. The actual execution
/// of spawning monsters or dropping health packs is handled upstream by the
/// game simulation loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// The player is doing too well. Spawn a sudden wave of monsters or a trap.
    SpawnAmbush,
    /// The player is struggling and near death. Drop a medkit or ease the pressure.
    SpawnRelief,
    /// The player's tension is in the sweet spot. Do nothing and let them play.
    Maintain,
}

/// The system that monitors player performance and manages game pacing.
///
/// Unlike standard Doom mechanics which are entirely deterministic, the
/// `AiDirector` provides a "guiding hand" to maintain optimal player tension.
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
/// // A nearly dead player triggers relief
/// player.set_health_capped(10, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
///
/// // A fully healed player triggers an ambush
/// player.set_health_capped(100, MAX_HEALTH);
/// assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
/// ```
#[doc(alias = "Director")]
pub struct AiDirector;

impl AiDirector {
    /// Creates a new, freshly initialized AI Director.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the player's current status and decides on an action.
    ///
    /// This should be called once per game tic (or at a defined interval) by the
    /// master game loop. It examines the [`PlayerState`] to assess the player's
    /// current "tension" level based on their health pool.
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
    /// player.set_health_capped(50, MAX_HEALTH);
    ///
    /// // The Director maintains the current pace if health is average.
    /// let action = director.tick(&player);
    /// assert_eq!(action, DirectorAction::Maintain);
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
    /// Creates a new AI Director with default settings.
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
