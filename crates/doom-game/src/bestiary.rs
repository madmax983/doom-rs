//! Bestiary system for tracking encountered monsters and their stats.
//!
//! Tracks which monsters the player has killed and unlocks progressive
//! details (name, stats, lore) as they defeat more of the same type.

use doom_types::mobj_kind::MobjKind;
use std::collections::HashMap;

/// The level of detail unlocked for a specific monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnlockLevel {
    /// Monster has never been killed.
    Locked,
    /// Killed once: Name unlocked.
    NameOnly,
    /// Killed 5 times: Stats unlocked (health, speed, etc).
    Stats,
    /// Killed 20 times: Full lore unlocked.
    Full,
}

/// Tracks the player's kill counts per monster type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bestiary {
    kill_counts: HashMap<MobjKind, u32>,
}

impl Bestiary {
    /// Create a new, empty Bestiary.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kill_counts: HashMap::new(),
        }
    }

    /// Record a monster kill.
    pub fn record_kill(&mut self, kind: MobjKind) {
        // Only track killable monsters (basic filter, actual implementation might be more specific)
        if (kind as u16) <= 17 || (68..=71).contains(&(kind as u16)) {
            *self.kill_counts.entry(kind).or_insert(0) += 1;
        }
    }

    /// Get the number of times a specific monster has been killed.
    #[must_use]
    pub fn get_kill_count(&self, kind: MobjKind) -> u32 {
        self.kill_counts.get(&kind).copied().unwrap_or(0)
    }

    /// Get the current unlock level for a specific monster.
    #[must_use]
    pub fn get_unlock_level(&self, kind: MobjKind) -> UnlockLevel {
        let count = self.get_kill_count(kind);
        if count >= 20 {
            UnlockLevel::Full
        } else if count >= 5 {
            UnlockLevel::Stats
        } else if count >= 1 {
            UnlockLevel::NameOnly
        } else {
            UnlockLevel::Locked
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bestiary_unlock_levels() {
        let mut bestiary = Bestiary::new();
        let kind = MobjKind::Imp;

        assert_eq!(bestiary.get_unlock_level(kind), UnlockLevel::Locked);

        bestiary.record_kill(kind);
        assert_eq!(bestiary.get_unlock_level(kind), UnlockLevel::NameOnly);

        for _ in 0..4 {
            bestiary.record_kill(kind);
        }
        assert_eq!(bestiary.get_unlock_level(kind), UnlockLevel::Stats);

        for _ in 0..15 {
            bestiary.record_kill(kind);
        }
        assert_eq!(bestiary.get_unlock_level(kind), UnlockLevel::Full);
    }

    #[test]
    fn test_bestiary_filters_non_monsters() {
        let mut bestiary = Bestiary::new();
        // A projectile shouldn't be tracked
        bestiary.record_kill(MobjKind::Rocket);
        assert_eq!(bestiary.get_kill_count(MobjKind::Rocket), 0);
    }
}
