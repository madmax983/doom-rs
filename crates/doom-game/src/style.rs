//! Style Meter feature: tracks rapid multi-kills and assigns a DMC-like style rank.

use std::cmp;

/// Represents the current style rank.
///
/// Derived from a [`StyleMeter`]'s current score. Ranks range from `Dismal` (the default)
/// all the way up to `SmokinSexyStyle` for massive unbroken kill streaks.
///
/// ## Examples
/// ```
/// use doom_game::style::{StyleMeter, StyleRank};
///
/// let mut meter = StyleMeter::new();
/// assert_eq!(meter.rank(), StyleRank::Dismal);
///
/// // Register a massive kill streak...
/// for _ in 0..20 {
///     meter.register_kill(10);
/// }
///
/// assert_eq!(meter.rank(), StyleRank::SmokinSexyStyle);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StyleRank {
    /// **D**ismal. The baseline. You are breathing, but just barely.
    Dismal,
    /// **C**razy. A few chained kills. The demons are noticing you.
    Crazy,
    /// **B**adass. A respectable streak. Blood is starting to pool.
    Badass,
    /// **A**pocalyptic. You are a walking disaster zone.
    Apocalyptic,
    /// **S**avage. Total mastery of the arena.
    Savage,
    /// **S**ick **S**kills. The framerate stutters from the carnage.
    Sick,
    /// **S**mokin **S**exy **S**tyle!! The absolute zenith of demonic slaughter.
    SmokinSexyStyle,
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
///
/// In standard Doom, survival is the only metric. The `StyleMeter` changes the narrative
/// by rewarding *aggression* and *momentum*. It tracks rapid, successive kills and
/// assigns a qualitative [`StyleRank`] to the player's current performance.
///
/// The core mechanic relies on a combo window (approx 2 seconds). If the player kills
/// another demon before the window closes, their combo count increases, granting a
/// multiplicative bonus to the score. If the window closes, the combo resets and the
/// score begins to rapidly decay.
///
/// ## Examples
/// ```
/// use doom_game::style::{StyleMeter, StyleRank};
///
/// let mut meter = StyleMeter::new();
///
/// // Kill a demon at tic 10
/// meter.register_kill(10);
/// assert_eq!(meter.score, 100);
/// assert_eq!(meter.combo_count, 1);
///
/// // Kill another demon quickly at tic 20! Combo multiplier kicks in.
/// meter.register_kill(20);
/// assert_eq!(meter.score, 300); // 100 + (100 * 2)
/// assert_eq!(meter.combo_count, 2);
///
/// // Wait too long... combo breaks and score decays.
/// meter.tick(100);
/// assert_eq!(meter.combo_count, 0);
/// assert_eq!(meter.score, 295);
/// ```
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

    /// Initializes a cold style meter.
    ///
    /// The meter begins at a score of 0 (Rank: `Dismal`) with no active combo.
    /// It must be fed blood to awaken.
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
