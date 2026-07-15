#[derive(Clone, Debug)]
pub struct RpgSystem {
    pub xp: u32,
    pub level: u32,
}

impl RpgSystem {
    pub fn new() -> Self {
        Self { xp: 0, level: 1 }
    }

    pub fn grant_xp(&mut self, kind: doom_types::mobj_kind::MobjKind) -> bool {
        let xp_gain = match kind {
            doom_types::mobj_kind::MobjKind::Trooper
            | doom_types::mobj_kind::MobjKind::Sergeant => 10,
            doom_types::mobj_kind::MobjKind::Imp
            | doom_types::mobj_kind::MobjKind::Demon
            | doom_types::mobj_kind::MobjKind::Spectre => 25,
            doom_types::mobj_kind::MobjKind::BaronOfHell
            | doom_types::mobj_kind::MobjKind::HellKnight => 100,
            doom_types::mobj_kind::MobjKind::Cyberdemon => 1000,
            _ => 5,
        };

        let old_level = self.level;
        self.xp += xp_gain;
        self.level = 1 + (self.xp / 100);

        self.level > old_level
    }

    pub fn current_max_health(&self) -> i32 {
        100 + (self.level as i32 - 1) * 20
    }
}

impl Default for RpgSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_types::mobj_kind::MobjKind;

    #[test]
    fn test_level_up() {
        let mut rpg = RpgSystem::new();
        assert_eq!(rpg.level, 1);
        assert_eq!(rpg.current_max_health(), 100);

        let leveled = rpg.grant_xp(MobjKind::Cyberdemon);
        assert!(leveled);
        assert!(rpg.level > 1);
        assert!(rpg.current_max_health() > 100);
    }
}
