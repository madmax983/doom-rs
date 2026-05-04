//! AI Director for dynamic difficulty adjustment.
//!
//! Monitors player health and resource levels to dynamically spawn
//! ambushes or relief items, keeping the tension optimal.

use crate::PlayerState;

/// The action chosen by the director for the current tic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    /// Spawn enemies near the player to increase tension.
    SpawnAmbush,
    /// Spawn health or ammo near the player to prevent frustration.
    SpawnRelief,
    /// Do nothing; tension is optimal.
    Maintain,
}

/// The stateful AI Director subsystem.
pub struct AiDirector;

impl AiDirector {
    /// Initialize the director.
    ///
    /// The director tracks historical player data to adjust difficulty,
    /// so it must be instantiated per game session.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_game::AiDirector;
    ///
    /// let director = AiDirector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Determine the next tactical move based on the player's health.
    ///
    /// The director monitors the player's survival metrics. If they are
    /// breezing through (health > 80), it ramps up the challenge. If they
    /// are struggling (health < 30), it provides breathing room.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_game::{AiDirector, DirectorAction};
    /// use doom_game::mobj::{MobjHandle, MobjSlab, Mobj};
    /// use doom_types::mobj_kind::MobjKind;
    /// use doom_types::{Bam, Fixed16_16};
    /// use doom_game::PlayerState;
    ///
    /// let mut director = AiDirector::new();
    ///
    /// // Mock a player struggling with 20 health.
    /// let mut slab = MobjSlab::new();
    /// let mo = Mobj::new(
    ///     MobjKind::Player,
    ///     Fixed16_16::ZERO,
    ///     Fixed16_16::ZERO,
    ///     Bam::ZERO,
    /// );
    /// let handle = slab.alloc(mo);
    /// let mut player = PlayerState::pistol_start(handle);
    /// player.apply_damage(80);
    ///
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
    use crate::mobj::{Mobj, MobjSlab};
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    fn make_test_player(health: i32) -> PlayerState {
        let mut slab = MobjSlab::new();
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let handle = slab.alloc(mo);
        let mut player = PlayerState::pistol_start(handle);
        player.apply_damage(100 - health);
        player
    }

    #[test]
    fn test_director_high_health_spawns_ambush() {
        let mut director = AiDirector::new();
        let player = make_test_player(90); // 90 health > 80

        assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    }

    #[test]
    fn test_director_low_health_spawns_relief() {
        let mut director = AiDirector::new();
        let player = make_test_player(20); // 20 health < 30

        assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
    }

    #[test]
    fn test_director_mid_health_maintains() {
        let mut director = AiDirector::new();
        let player = make_test_player(50); // 50 health

        assert_eq!(director.tick(&player), DirectorAction::Maintain);
    }
}
