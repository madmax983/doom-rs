//! Combo system for chaining kills together.

#[derive(Debug, Clone, Default)]
pub struct ComboSystem {
    pub current_combo: u32,
    pub max_combo: u32,
    pub tics_since_last_kill: u32,
}

impl ComboSystem {
    pub const COMBO_TIMEOUT: u32 = 105; // 3 seconds at 35 tics/sec

    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self) {
        self.tics_since_last_kill = self.tics_since_last_kill.saturating_add(1);
        if self.tics_since_last_kill > Self::COMBO_TIMEOUT {
            self.current_combo = 0;
        }
    }

    pub fn register_kill(&mut self) {
        self.current_combo += 1;
        self.tics_since_last_kill = 0;
        if self.current_combo > self.max_combo {
            self.max_combo = self.current_combo;
        }
    }

    pub fn multiplier(&self) -> u32 {
        1 + (self.current_combo / 3) // 1x, then 2x at 3 kills, 3x at 6 kills...
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combo_multiplier_increases() {
        let mut combo = ComboSystem::new();
        assert_eq!(combo.multiplier(), 1);
        combo.register_kill();
        combo.register_kill();
        combo.register_kill();
        assert_eq!(combo.multiplier(), 2);
    }
}
