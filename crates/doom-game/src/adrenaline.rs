//! Adrenaline meter system.
//!
//! Rewards players for aggressive gameplay by building up a meter
//! that can eventually trigger a super-state.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdrenalineMeter {
    pub level: u32,
}

impl AdrenalineMeter {
    pub fn new() -> Self {
        Self { level: 0 }
    }

    pub fn add_kill(&mut self) {
        self.level = self.level.saturating_add(10);
    }

    pub fn add_damage_taken(&mut self, damage: u32) {
        self.level = self.level.saturating_add(damage / 2);
    }

    pub fn decay(&mut self) {
        self.level = self.level.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adrenaline_builds_on_kill() {
        let mut meter = AdrenalineMeter::new();
        meter.add_kill();
        assert_eq!(meter.level, 10);
    }

    #[test]
    fn test_adrenaline_builds_on_damage() {
        let mut meter = AdrenalineMeter::new();
        meter.add_damage_taken(20);
        assert_eq!(meter.level, 10);
    }

    #[test]
    fn test_adrenaline_decays() {
        let mut meter = AdrenalineMeter::new();
        meter.add_kill();
        meter.decay();
        assert_eq!(meter.level, 9);
    }
}
