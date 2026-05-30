use crate::PlayerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    SpawnAmbush,
    SpawnRelief,
    Maintain,
}

use doom_map::analyzer::MapAnalyzer;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TacticalAction {
    SpawnAmbushAtChokepoint(usize),
    SpawnReliefAtSafeZone(Vec<HashSet<usize>>),
    Maintain,
}

pub struct TacticalDirector<'a> {
    analyzer: MapAnalyzer<'a>,
}

impl<'a> TacticalDirector<'a> {
    pub fn new(analyzer: MapAnalyzer<'a>) -> Self {
        Self { analyzer }
    }

    pub fn evaluate(&mut self, player: &PlayerState) -> Vec<TacticalAction> {
        let health = player.health();
        let mut actions = Vec::new();

        if health > 80 {
            if let Some(&chokepoint) = self.analyzer.chokepoints().first() {
                actions.push(TacticalAction::SpawnAmbushAtChokepoint(chokepoint));
            } else {
                actions.push(TacticalAction::Maintain);
            }
        } else if health < 30 {
            let isolated = self.analyzer.isolated_areas();
            if !isolated.is_empty() {
                actions.push(TacticalAction::SpawnReliefAtSafeZone(isolated));
            } else {
                actions.push(TacticalAction::Maintain);
            }
        } else {
            actions.push(TacticalAction::Maintain);
        }

        actions
    }
}

pub struct AiDirector;

impl AiDirector {
    pub fn new() -> Self {
        Self
    }

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
    fn test_tactical_director_ambush() {
        use doom_map::{SectorGraph, analyzer::MapAnalyzer};
        use std::collections::{HashMap, HashSet};

        // Linear map: 0 <-> 1 <-> 2
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1]));
        adj.insert(1, HashSet::from([0, 2]));
        adj.insert(2, HashSet::from([1]));
        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);

        let mut td = TacticalDirector::new(analyzer);
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(MAX_HEALTH, MAX_HEALTH);

        let actions = td.evaluate(&player);
        assert_eq!(actions.len(), 1);

        if let TacticalAction::SpawnAmbushAtChokepoint(sector) = actions[0] {
            assert_eq!(sector, 1);
        } else {
            panic!("Expected SpawnAmbushAtChokepoint");
        }
    }

    #[test]
    fn test_tactical_director_relief() {
        use doom_map::{SectorGraph, analyzer::MapAnalyzer};
        use std::collections::{HashMap, HashSet};

        // Linear map: 0 <-> 1 <-> 2
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1]));
        adj.insert(1, HashSet::from([0, 2]));
        adj.insert(2, HashSet::from([1]));
        let graph = SectorGraph {
            adjacency_list: adj,
        };
        let analyzer = MapAnalyzer::new(&graph);

        let mut td = TacticalDirector::new(analyzer);
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(10, MAX_HEALTH);

        let actions = td.evaluate(&player);
        assert_eq!(actions.len(), 1);

        if let TacticalAction::SpawnReliefAtSafeZone(areas) = &actions[0] {
            assert_eq!(areas.len(), 1);
            assert!(areas[0].contains(&0));
        } else {
            panic!("Expected SpawnReliefAtSafeZone");
        }
    }

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
