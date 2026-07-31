//! Demonology Bestiary
//!
//! Tracks monster encounters and kill counts to provide a mastery level
//! per monster type.

use doom_types::mobj_kind::MobjKind;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MasteryLevel {
    Unknown,
    Encountered,
    Slayer,
    Master,
}

#[derive(Debug, Clone, Default)]
pub struct DemonologyDex {
    pub encounters: HashSet<MobjKind>,
    pub kill_counts: HashMap<MobjKind, u32>,
}

impl DemonologyDex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_encounter(&mut self, kind: MobjKind) {
        self.encounters.insert(kind);
    }

    pub fn record_kill(&mut self, kind: MobjKind) {
        self.encounters.insert(kind);
        *self.kill_counts.entry(kind).or_insert(0) += 1;
    }

    pub fn get_mastery(&self, kind: MobjKind) -> MasteryLevel {
        if !self.encounters.contains(&kind) {
            return MasteryLevel::Unknown;
        }

        let kills = self.kill_counts.get(&kind).copied().unwrap_or(0);
        match kills {
            0..=9 => MasteryLevel::Encountered,
            10..=49 => MasteryLevel::Slayer,
            _ => MasteryLevel::Master,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let dex = DemonologyDex::new();
        assert_eq!(
            dex.get_mastery(MobjKind::BaronOfHell),
            MasteryLevel::Unknown
        );
    }

    #[test]
    fn test_encounter_only() {
        let mut dex = DemonologyDex::new();
        dex.record_encounter(MobjKind::HellKnight);
        assert_eq!(
            dex.get_mastery(MobjKind::HellKnight),
            MasteryLevel::Encountered
        );
    }

    #[test]
    fn test_kill_thresholds() {
        let mut dex = DemonologyDex::new();

        dex.record_kill(MobjKind::Arachnotron);
        assert_eq!(
            dex.get_mastery(MobjKind::Arachnotron),
            MasteryLevel::Encountered
        );

        for _ in 0..10 {
            dex.record_kill(MobjKind::Arachnotron);
        }
        assert_eq!(dex.get_mastery(MobjKind::Arachnotron), MasteryLevel::Slayer);

        for _ in 0..40 {
            dex.record_kill(MobjKind::Arachnotron);
        }
        assert_eq!(dex.get_mastery(MobjKind::Arachnotron), MasteryLevel::Master);
    }
}
