//! Tension subsystem.
//! Tracks the current danger level based on nearby enemies and player health.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TensionLevel {
    Relaxed,
    Alert,
    Combat,
    Panic,
}

#[derive(Debug, Clone, Default)]
pub struct TensionMeter {
    pub threat_score: u32,
    pub last_update_tic: u32,
}

impl TensionMeter {
    pub fn new() -> Self {
        Self {
            threat_score: 0,
            last_update_tic: 0,
        }
    }

    pub fn level(&self) -> TensionLevel {
        if self.threat_score >= 100 {
            TensionLevel::Panic
        } else if self.threat_score >= 50 {
            TensionLevel::Combat
        } else if self.threat_score > 0 {
            TensionLevel::Alert
        } else {
            TensionLevel::Relaxed
        }
    }

    pub fn add_threat(&mut self, amount: u32, current_tic: u32) {
        self.threat_score = self.threat_score.saturating_add(amount).min(200);
        self.last_update_tic = current_tic;
    }

    pub fn tick(&mut self, current_tic: u32) {
        // Decay score over time if no new threats
        if current_tic > self.last_update_tic.saturating_add(35) {
            // 1 second
            self.threat_score = self.threat_score.saturating_sub(1);
            // Reset update tic so it keeps decaying
            self.last_update_tic = current_tic;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tension_starts_relaxed() {
        let meter = TensionMeter::new();
        assert_eq!(meter.level(), TensionLevel::Relaxed);
    }

    #[test]
    fn test_tension_increases_with_threats() {
        let mut meter = TensionMeter::new();
        meter.add_threat(50, 0);
        assert_eq!(meter.level(), TensionLevel::Combat);
        meter.add_threat(50, 0);
        assert_eq!(meter.level(), TensionLevel::Panic);
    }

    #[test]
    fn test_tension_decay() {
        let mut meter = TensionMeter::new();
        meter.add_threat(10, 0);
        meter.tick(36);
        assert_eq!(meter.threat_score, 9);
        meter.tick(72);
        assert_eq!(meter.threat_score, 8);
    }
}
