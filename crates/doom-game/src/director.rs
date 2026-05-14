use crate::PlayerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorAction {
    SpawnAmbush,
    SpawnRelief,
    Maintain,
}

pub struct AiDirector;

impl AiDirector {
    pub fn new() -> Self {
        Self
    }

    #[cfg(not(feature = "style_meter"))]
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

    #[cfg(feature = "style_meter")]
    pub fn tick(
        &mut self,
        player: &PlayerState,
        style: &crate::style::StyleMeter,
    ) -> DirectorAction {
        let health = player.health();
        let rank = style.rank();

        if rank == crate::style::StyleRank::SmokinSexyStyle || rank == crate::style::StyleRank::Sick
        {
            DirectorAction::SpawnAmbush
        } else if health < 30 && rank <= crate::style::StyleRank::Crazy {
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

    #[cfg(not(feature = "style_meter"))]
    #[test]
    fn test_high_health_spawns_ambush() {
        let mut director = AiDirector::new();
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(MAX_HEALTH, MAX_HEALTH);
        assert_eq!(director.tick(&player), DirectorAction::SpawnAmbush);
    }

    #[cfg(not(feature = "style_meter"))]
    #[test]
    fn test_low_health_spawns_relief() {
        let mut director = AiDirector::new();
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(10, MAX_HEALTH);
        assert_eq!(director.tick(&player), DirectorAction::SpawnRelief);
    }

    #[cfg(not(feature = "style_meter"))]
    #[test]
    fn test_medium_health_maintains() {
        let mut director = AiDirector::new();
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(50, MAX_HEALTH);
        assert_eq!(director.tick(&player), DirectorAction::Maintain);
    }

    #[cfg(feature = "style_meter")]
    #[test]
    fn test_style_sss_spawns_ambush() {
        let mut director = AiDirector::new();
        let player = PlayerState::pistol_start(MobjHandle::NULL);
        let mut style = crate::style::StyleMeter::new();
        style.score = crate::style::StyleMeter::SSS_SCORE;
        assert_eq!(director.tick(&player, &style), DirectorAction::SpawnAmbush);
    }

    #[cfg(feature = "style_meter")]
    #[test]
    fn test_style_low_health_low_style_spawns_relief() {
        let mut director = AiDirector::new();
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(10, MAX_HEALTH);
        let style = crate::style::StyleMeter::new();
        assert_eq!(director.tick(&player, &style), DirectorAction::SpawnRelief);
    }

    #[cfg(feature = "style_meter")]
    #[test]
    fn test_style_high_health_low_style_maintains() {
        let mut director = AiDirector::new();
        let mut player = PlayerState::pistol_start(MobjHandle::NULL);
        player.set_health_capped(100, MAX_HEALTH);
        let style = crate::style::StyleMeter::new();
        assert_eq!(director.tick(&player, &style), DirectorAction::Maintain);
    }
}
