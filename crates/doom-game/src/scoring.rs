use doom_types::Fixed16_16;

/// A trait for calculating a score modifier based on distance.
///
/// 🔮 **The Potential:** Could be used for spatial queries, sniper rifle mechanics,
/// or rewarding players for long-range takedowns.
pub trait DistanceBasedScoring {
    /// Calculate the modified score based on the distance between the attacker
    /// and the target.
    fn calculate_score(&self, base_score: u32, distance: Fixed16_16) -> u32;
}

/// A scoring implementation that rewards long-distance shots.
/// The further the target, the higher the score multiplier.
#[derive(Debug, Clone, Copy, Default)]
pub struct SniperScoring {
    /// The minimum distance to start applying a multiplier (in fixed-point map units).
    pub min_distance: Fixed16_16,
    /// The maximum multiplier that can be applied.
    pub max_multiplier: u32,
}

impl SniperScoring {
    pub const fn new(min_distance: Fixed16_16, max_multiplier: u32) -> Self {
        Self {
            min_distance,
            max_multiplier,
        }
    }
}

impl DistanceBasedScoring for SniperScoring {
    fn calculate_score(&self, base_score: u32, distance: Fixed16_16) -> u32 {
        if distance <= self.min_distance {
            return base_score;
        }

        // Calculate how many times min_distance goes into the total distance.
        let distance_int = distance.to_int();
        let min_dist_int = self.min_distance.to_int();

        if min_dist_int <= 0 {
            return base_score * self.max_multiplier;
        }

        let multiplier = 1 + (distance_int / min_dist_int) as u32;
        let final_multiplier = multiplier.min(self.max_multiplier);

        base_score * final_multiplier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniper_scoring_below_min_distance() {
        let scoring = SniperScoring::new(Fixed16_16::from_int(100), 5);
        let score = scoring.calculate_score(10, Fixed16_16::from_int(50));
        assert_eq!(score, 10, "Score should remain the same below min distance");
    }

    #[test]
    fn test_sniper_scoring_above_min_distance() {
        let scoring = SniperScoring::new(Fixed16_16::from_int(100), 5);
        // Distance is 250. 250 / 100 = 2. Multiplier = 1 + 2 = 3.
        let score = scoring.calculate_score(10, Fixed16_16::from_int(250));
        assert_eq!(score, 30, "Score should be multiplied by 3");
    }

    #[test]
    fn test_sniper_scoring_max_multiplier() {
        let scoring = SniperScoring::new(Fixed16_16::from_int(100), 5);
        // Distance is 1000. 1000 / 100 = 10. Multiplier = 1 + 10 = 11. Clamped to 5.
        let score = scoring.calculate_score(10, Fixed16_16::from_int(1000));
        assert_eq!(score, 50, "Score multiplier should be clamped to max_multiplier");
    }

    #[test]
    fn test_sniper_scoring_zero_min_distance() {
        let scoring = SniperScoring::new(Fixed16_16::from_int(0), 5);
        let score = scoring.calculate_score(10, Fixed16_16::from_int(10));
        assert_eq!(score, 50, "Score multiplier should be max if min_distance is 0");
    }
}
