use crate::PlayerState;
use doom_map::analyzer::MapAnalyzer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TacticalAction {
    SpawnAmbush(usize),
    SpawnRelief(usize),
    Maintain,
}

pub struct TacticalDirector<'a> {
    pub analyzer: MapAnalyzer<'a>,
}

impl<'a> TacticalDirector<'a> {
    pub fn new(analyzer: MapAnalyzer<'a>) -> Self {
        Self { analyzer }
    }

    pub fn tick(&mut self, player: &PlayerState) -> TacticalAction {
        let health = player.health();

        if health > 80 {
            if let Some(&choke) = self.analyzer.chokepoints().first() {
                return TacticalAction::SpawnAmbush(choke);
            }
        } else if health < 30 {
            if let Some(area) = self.analyzer.isolated_areas().first() {
                if let Some(&safe_sector) = area.iter().next() {
                    return TacticalAction::SpawnRelief(safe_sector);
                }
            }
        }

        TacticalAction::Maintain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::MobjHandle;
    use doom_map::graph::SectorGraph;
    use doom_types::limits::MAX_HEALTH;
    use std::collections::{HashMap, HashSet};

    fn setup_graph() -> SectorGraph {
        let mut adj = HashMap::new();
        adj.insert(0, HashSet::from([1]));
        adj.insert(1, HashSet::from([0, 2]));
        adj.insert(2, HashSet::from([1]));
        adj.insert(3, HashSet::from([4]));
        adj.insert(4, HashSet::from([3]));
        SectorGraph {
            adjacency_list: adj,
        }
    }

    #[test]
    fn test_high_health_spawns_ambush_at_chokepoint() {
        let graph = setup_graph();
        let analyzer = MapAnalyzer::new(&graph);
        let mut director = TacticalDirector::new(analyzer);

        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(MAX_HEALTH, MAX_HEALTH);

        assert_eq!(director.tick(&player), TacticalAction::SpawnAmbush(1));
    }

    #[test]
    fn test_low_health_spawns_relief_in_isolated_area() {
        let graph = setup_graph();
        let analyzer = MapAnalyzer::new(&graph);
        let mut director = TacticalDirector::new(analyzer);

        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(10, MAX_HEALTH);

        let action = director.tick(&player);
        match action {
            TacticalAction::SpawnRelief(sector) => {
                assert!(sector == 0 || sector == 1 || sector == 2 || sector == 3 || sector == 4);
            }
            _ => panic!("Expected SpawnRelief"),
        }
    }
}
