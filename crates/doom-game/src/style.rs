//! The Style Meter subsystem tracks rapid multi-kills to assign a DMC-like [`StyleRank`].
//!
//! This module acts as the core scoring mechanism for the style system. It monitors
//! the frequency of enemy takedowns. Rapid consecutive kills extend a combo multiplier,
//! propelling the player's score and [`StyleRank`] higher. Conversely, passive play
//! causes the score to decay over time.
//!
//! The highest possible rank is [`StyleRank::SmokinSexyStyle`].

use std::cmp;

/// Represents the current style rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StyleRank {
    /// The lowest and starting rank. Indicates no recent combo activity.
    Dismal,
    /// A minor combo has been achieved.
    Crazy,
    /// Moderate combo streak.
    Badass,
    /// High combo streak, requiring consistent rapid kills.
    Apocalyptic,
    /// Excellent combo multiplier.
    Savage,
    /// Outstanding string of consecutive multi-kills.
    Sick,
    /// The absolute pinnacle of combat flow. Reached only through flawless execution.
    SmokinSexyStyle,
}

impl StyleRank {
    /// Returns the human-readable display string for this rank.
    ///
    /// This is typically used by UI components to flash the rank on screen.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::style::StyleRank;
    ///
    /// let rank = StyleRank::SmokinSexyStyle;
    /// assert_eq!(rank.name(), "Smokin Sexy Style!!");
    /// ```
    /// Gets the display name of the style rank.
    ///
    /// ## Examples
    /// ```
    /// use doom_game::style::StyleRank;
    /// assert_eq!(StyleRank::Dull.name(), "Dull");
    /// ```
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

    /// Initializes an empty style tracker.
    ///
    /// Begins at the `Dismal` rank with no active combo.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::style::StyleMeter;
    ///
    /// let meter = StyleMeter::new();
    /// assert_eq!(meter.score, 0);
    /// ```
    /// Creates a new style meter.
    ///
    /// ## Examples
    /// ```
    /// use doom_game::style::StyleMeter;
    /// let meter = StyleMeter::new();
    /// ```
    pub fn new() -> Self {
        Self {
            score: 0,
            last_kill_tic: 0,
            combo_count: 0,
        }
    }

    /// Records an enemy takedown, evaluating if it extends an ongoing combo.
    ///
    /// A kill within the [`StyleMeter::COMBO_WINDOW_TICS`] extends the combo multiplier up to 10x,
    /// rapidly accelerating score accumulation to reach higher [`StyleRank`] levels.
    /// If the combo window has expired, the multiplier resets.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::style::StyleMeter;
    ///
    /// let mut meter = StyleMeter::new();
    /// meter.register_kill(10);
    /// assert_eq!(meter.score, 100);
    /// ```
    /// Registers a kill to increase style.
    ///
    /// ## Examples
    /// ```
    /// use doom_game::style::StyleMeter;
    /// let mut meter = StyleMeter::new();
    /// meter.register_kill(10);
    /// ```
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

    /// Processes a game frame, decaying the score if the action has stalled.
    ///
    /// The style system penalizes passive play. If no kills occur within
    /// the [`StyleMeter::COMBO_WINDOW_TICS`], the score continuously drains, eventually
    /// degrading the current [`StyleRank`].
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::style::StyleMeter;
    ///
    /// let mut meter = StyleMeter::new();
    /// meter.register_kill(10);
    /// meter.tick(100); // Beyond the combo window
    /// assert_eq!(meter.score, 95);
    /// ```
    /// Ticks the style meter, decaying score over time.
    ///
    /// ## Examples
    /// ```
    /// use doom_game::style::StyleMeter;
    /// let mut meter = StyleMeter::new();
    /// meter.tick(10);
    /// ```
    pub fn tick(&mut self, current_tic: u32) {
        if current_tic > self.last_kill_tic.saturating_add(Self::COMBO_WINDOW_TICS) {
            // Decay score
            self.score = self.score.saturating_sub(5);
            self.combo_count = 0;
        }
    }

    /// Evaluates the current accumulated score to determine the active [`StyleRank`].
    ///
    /// This translates the internal numerical score into the discrete tiers
    /// expected by the presentation layer.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_game::style::{StyleMeter, StyleRank};
    ///
    /// let meter = StyleMeter::new();
    /// assert_eq!(meter.rank(), StyleRank::Dismal);
    /// ```
    /// Gets the current style rank.
    ///
    /// ## Examples
    /// ```
    /// use doom_game::style::{StyleMeter, StyleRank};
    /// let meter = StyleMeter::new();
    /// assert_eq!(meter.rank(), StyleRank::Dull);
    /// ```
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
