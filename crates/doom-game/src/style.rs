//! Style Meter feature: tracks rapid multi-kills and assigns a DMC-like style rank.

use std::cmp;

/// Represents the current style rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StyleRank {
    /// The lowest rank, achieved after a single kill or when combos drop.
    Dismal, // D
    /// Achieved after a few quick sequential kills.
    Crazy, // C
    /// A solid combo string.
    Badass, // B
    /// A chaotic, rapid sequence of kills.
    Apocalyptic, // A
    /// A highly skilled combo requiring constant aggression.
    Savage, // S
    /// A near-perfect display of carnage.
    Sick, // SS
    /// The pinnacle of style, requiring a flawless, uninterrupted streak of destruction.
    SmokinSexyStyle, // SSS
}

impl StyleRank {
    /// Get the rank name.
    pub fn name(&self) -> &'static str {
        match self {
            StyleRank::Dismal => "Dismal",
            StyleRank::Crazy => "Crazy",
            StyleRank::Badass => "Badass",
            StyleRank::Apocalyptic => "Apocalyptic",
            StyleRank::Savage => "Savage",
            StyleRank::Sick => "Sick",
            StyleRank::SmokinSexyStyle => "Smokin Sexy Style!!",
        }
    }
}

/// A DMC-like style meter.
#[derive(Debug, Clone)]
pub struct StyleMeter {
    /// Current style score.
    pub score: u32,
    /// Last tic when a kill was registered.
    pub last_kill_tic: u32,
    /// Number of sequential kills without dropping combo.
    pub combo_count: u32,
}

impl Default for StyleMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl StyleMeter {
    /// Tics before the combo resets (approx 2 seconds).
    pub const COMBO_WINDOW_TICS: u32 = 70;

    /// Score required for SSS rank.
    pub const SSS_SCORE: u32 = 10000;

    /// Creates a new, empty style meter.
    ///
    /// The meter starts at 0 score and a `Dismal` rank.
    ///
    /// # Examples
    /// ```
    /// use doom_game::style::{StyleMeter, StyleRank};
    ///
    /// let meter = StyleMeter::new();
    /// assert_eq!(meter.score, 0);
    /// assert_eq!(meter.rank(), StyleRank::Dismal);
    /// ```
    pub fn new() -> Self {
        Self {
            score: 0,
            last_kill_tic: 0,
            combo_count: 0,
        }
    }

    /// Register a monster kill.
    pub fn register_kill(&mut self, current_tic: u32) {
        if current_tic <= self.last_kill_tic.saturating_add(Self::COMBO_WINDOW_TICS) {
            self.combo_count += 1;
        } else {
            self.combo_count = 1;
        }

        self.last_kill_tic = current_tic;

        let base_score = 100;
        let multiplier = cmp::min(self.combo_count, 10);
        self.score += base_score * multiplier;
    }

    /// Update the meter, applying score decay over time.
    pub fn tick(&mut self, current_tic: u32) {
        if current_tic > self.last_kill_tic.saturating_add(Self::COMBO_WINDOW_TICS) {
            // Decay score
            self.score = self.score.saturating_sub(5);
            self.combo_count = 0;
        }
    }

    /// Get the current rank based on score.
    pub fn rank(&self) -> StyleRank {
        if self.score >= Self::SSS_SCORE {
            StyleRank::SmokinSexyStyle
        } else if self.score >= 8000 {
            StyleRank::Sick
        } else if self.score >= 5000 {
            StyleRank::Savage
        } else if self.score >= 3000 {
            StyleRank::Apocalyptic
        } else if self.score >= 1500 {
            StyleRank::Badass
        } else if self.score >= 500 {
            StyleRank::Crazy
        } else {
            StyleRank::Dismal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_meter_combo() {
        let mut meter = StyleMeter::new();
        assert_eq!(meter.rank(), StyleRank::Dismal);

        meter.register_kill(10);
        assert_eq!(meter.combo_count, 1);
        assert_eq!(meter.score, 100);

        meter.register_kill(20);
        assert_eq!(meter.combo_count, 2);
        assert_eq!(meter.score, 300); // 100 + 200

        meter.register_kill(100); // Combo breaks (100 > 20 + 70)
        assert_eq!(meter.combo_count, 1);
        assert_eq!(meter.score, 400); // 300 + 100
    }

    #[test]
    fn test_style_meter_decay() {
        let mut meter = StyleMeter::new();
        meter.register_kill(10);
        assert_eq!(meter.score, 100);

        meter.tick(50); // No decay yet
        assert_eq!(meter.score, 100);

        meter.tick(100); // Decay starts
        assert_eq!(meter.score, 95);
        assert_eq!(meter.combo_count, 0);
    }
}
