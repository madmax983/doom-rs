use std::collections::HashMap;

/// Tracks interactions with different monster types to establish a "Nemesis".
/// This tracks damage dealt, damage received, and total kills per monster type.
#[derive(Debug, Clone, Default)]
pub struct NemesisSystem {
    /// Damage dealt to the player by specific enemy types.
    pub damage_received: HashMap<usize, u32>,
    /// How many times the player has been killed by specific enemy types.
    pub player_deaths: HashMap<usize, u32>,
    /// How many of each enemy type the player has killed.
    pub kills: HashMap<usize, u32>,
}

impl NemesisSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record damage received by the player from a specific enemy type.
    pub fn record_damage_received(&mut self, mobj_type: usize, amount: u32) {
        *self.damage_received.entry(mobj_type).or_insert(0) += amount;
    }

    /// Record a player death caused by a specific enemy type.
    pub fn record_player_death(&mut self, mobj_type: usize) {
        *self.player_deaths.entry(mobj_type).or_insert(0) += 1;
    }

    /// Record a kill of a specific enemy type by the player.
    pub fn record_kill(&mut self, mobj_type: usize) {
        *self.kills.entry(mobj_type).or_insert(0) += 1;
    }

    /// Identify the current Nemesis based on deaths and damage received.
    /// Returns the Mobj type index of the nemesis, or None if there is no clear nemesis.
    pub fn current_nemesis(&self) -> Option<usize> {
        let mut max_score = 0;
        let mut nemesis = None;

        // Score formula: (Deaths * 100) + Damage Received
        for (&mobj_type, &damage) in &self.damage_received {
            let deaths = self.player_deaths.get(&mobj_type).copied().unwrap_or(0);
            let score = deaths * 100 + damage;

            if score > max_score {
                max_score = score;
                nemesis = Some(mobj_type);
            }
        }

        nemesis
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nemesis_tracking() {
        let mut system = NemesisSystem::new();

        // 1 is Imp, 2 is Demon

        // Imp deals 50 damage
        system.record_damage_received(1, 50);

        // Demon deals 20 damage, but kills player
        system.record_damage_received(2, 20);
        system.record_player_death(2);

        // Demon should be nemesis due to death multiplier (1 * 100 + 20 = 120 vs 50)
        assert_eq!(system.current_nemesis(), Some(2));

        // Imp deals 100 more damage
        system.record_damage_received(1, 100);

        // Imp is now nemesis (150 vs 120)
        assert_eq!(system.current_nemesis(), Some(1));

        system.record_kill(1);
        system.record_kill(2);
        assert_eq!(system.kills.get(&1), Some(&1));
        assert_eq!(system.kills.get(&2), Some(&1));
    }
}
