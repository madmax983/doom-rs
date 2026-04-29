//! Achievement Engine for tracking and unlocking game badges.
//!
//! Evaluates game state events and unlocks specific achievements (badges)
//! such as "Smokin' Sexy Style" for reaching the maximum style rank, or
//! "Pacifist" for beating a level without killing any monsters.

#[cfg(feature = "style_meter")]
use crate::style::StyleRank;

/// Represents an unlockable achievement or badge in the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Badge {
    /// Awarded when the player attains the highest possible style rank.
    SmokinSexyStyle,
    /// Awarded when the player reaches 200% health via a Soulsphere or Megasphere.
    Overhealed,
    /// Awarded when the player discovers all secret sectors in a level.
    Explorer,
    /// Awarded when the player kills a Baron of Hell using only their fists or chainsaw.
    RipAndTear,
}

impl Badge {
    /// Returns the human-readable title of the achievement.
    pub fn title(&self) -> &'static str {
        match self {
            Badge::SmokinSexyStyle => "Smokin' Sexy Style!!",
            Badge::Overhealed => "Overhealed",
            Badge::Explorer => "Explorer",
            Badge::RipAndTear => "Rip and Tear",
        }
    }

    /// Returns a short description of how to unlock the achievement.
    pub fn description(&self) -> &'static str {
        match self {
            Badge::SmokinSexyStyle => "Reach the maximum style rank.",
            Badge::Overhealed => "Reach 200% health.",
            Badge::Explorer => "Find all secrets in a single level.",
            Badge::RipAndTear => "Defeat a Baron of Hell in melee combat.",
        }
    }
}

/// The core engine that evaluates game states to grant achievements.
#[derive(Debug, Clone, Default)]
pub struct AchievementEngine {
    /// The set of badges the player has already unlocked.
    pub unlocked_badges: std::collections::HashSet<Badge>,
}

impl AchievementEngine {
    /// Creates a new, empty achievement engine.
    pub fn new() -> Self {
        Self {
            unlocked_badges: std::collections::HashSet::new(),
        }
    }

    /// Checks if a specific badge has been unlocked.
    pub fn has_badge(&self, badge: Badge) -> bool {
        self.unlocked_badges.contains(&badge)
    }

    /// Evaluates if the current style rank warrants the SmokinSexyStyle badge.
    #[cfg(feature = "style_meter")]
    pub fn evaluate_style_rank(&mut self, rank: StyleRank) -> bool {
        if rank == StyleRank::SmokinSexyStyle && !self.has_badge(Badge::SmokinSexyStyle) {
            self.unlocked_badges.insert(Badge::SmokinSexyStyle);
            return true;
        }
        false
    }

    /// Evaluates if the player's health warrants the Overhealed badge.
    pub fn evaluate_health(&mut self, health: i32) -> bool {
        if health >= 200 && !self.has_badge(Badge::Overhealed) {
            self.unlocked_badges.insert(Badge::Overhealed);
            return true;
        }
        false
    }

    /// Evaluates if the player's secret count warrants the Explorer badge.
    pub fn evaluate_secrets(&mut self, found: u32, total: u32) -> bool {
        if total > 0 && found >= total && !self.has_badge(Badge::Explorer) {
            self.unlocked_badges.insert(Badge::Explorer);
            return true;
        }
        false
    }

    /// Evaluates if a melee kill on a Baron of Hell warrants the RipAndTear badge.
    pub fn evaluate_baron_melee_kill(&mut self) -> bool {
        if !self.has_badge(Badge::RipAndTear) {
            self.unlocked_badges.insert(Badge::RipAndTear);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_badge_titles_and_descriptions() {
        assert_eq!(Badge::SmokinSexyStyle.title(), "Smokin' Sexy Style!!");
        assert_eq!(Badge::Overhealed.title(), "Overhealed");
        assert_eq!(Badge::Explorer.title(), "Explorer");
        assert_eq!(Badge::RipAndTear.title(), "Rip and Tear");

        assert_eq!(
            Badge::SmokinSexyStyle.description(),
            "Reach the maximum style rank."
        );
        assert_eq!(Badge::Overhealed.description(), "Reach 200% health.");
        assert_eq!(
            Badge::Explorer.description(),
            "Find all secrets in a single level."
        );
        assert_eq!(
            Badge::RipAndTear.description(),
            "Defeat a Baron of Hell in melee combat."
        );
    }

    #[test]
    fn test_engine_initialization() {
        let engine = AchievementEngine::new();
        assert!(engine.unlocked_badges.is_empty());
        assert!(!engine.has_badge(Badge::Overhealed));
    }

    #[test]
    #[cfg(feature = "style_meter")]
    fn test_evaluate_style_rank() {
        let mut engine = AchievementEngine::new();
        assert!(!engine.evaluate_style_rank(StyleRank::Dismal));
        assert!(!engine.has_badge(Badge::SmokinSexyStyle));

        assert!(engine.evaluate_style_rank(StyleRank::SmokinSexyStyle));
        assert!(engine.has_badge(Badge::SmokinSexyStyle));

        // Subsequent evaluations should return false (already unlocked)
        assert!(!engine.evaluate_style_rank(StyleRank::SmokinSexyStyle));
    }

    #[test]
    fn test_evaluate_health() {
        let mut engine = AchievementEngine::new();
        assert!(!engine.evaluate_health(100));
        assert!(!engine.has_badge(Badge::Overhealed));

        assert!(engine.evaluate_health(200));
        assert!(engine.has_badge(Badge::Overhealed));

        assert!(!engine.evaluate_health(200));
    }

    #[test]
    fn test_evaluate_secrets() {
        let mut engine = AchievementEngine::new();
        assert!(!engine.evaluate_secrets(0, 0)); // No secrets in level -> no badge
        assert!(!engine.has_badge(Badge::Explorer));

        assert!(!engine.evaluate_secrets(1, 3));
        assert!(!engine.has_badge(Badge::Explorer));

        assert!(engine.evaluate_secrets(3, 3));
        assert!(engine.has_badge(Badge::Explorer));

        assert!(!engine.evaluate_secrets(3, 3));
    }

    #[test]
    fn test_evaluate_baron_melee_kill() {
        let mut engine = AchievementEngine::new();
        assert!(engine.evaluate_baron_melee_kill());
        assert!(engine.has_badge(Badge::RipAndTear));
        assert!(!engine.evaluate_baron_melee_kill());
    }
}
