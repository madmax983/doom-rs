use crate::lumps::Reject;

/// Analyzes map visibility to find tactical positions.
pub struct SightAnalyzer<'a> {
    reject: &'a Reject,
}

impl<'a> SightAnalyzer<'a> {
    /// Creates a new SightAnalyzer wrapping a REJECT table.
    pub fn new(reject: &'a Reject) -> Self {
        Self { reject }
    }

    /// Calculates the visibility score (number of other sectors visible) for each sector.
    pub fn visibility_scores(&self) -> Vec<(usize, usize)> {
        let n = self.reject.n_sectors();
        let mut scores = Vec::with_capacity(n);
        for i in 0..n {
            let mut count = 0;
            for j in 0..n {
                if i != j && self.reject.visible(i, j) {
                    count += 1;
                }
            }
            scores.push((i, count));
        }
        scores
    }

    /// Returns the top sectors that have the highest line-of-sight to other sectors.
    pub fn overwatch_sectors(&self, limit: usize) -> Vec<(usize, usize)> {
        let mut scores = self.visibility_scores();
        scores.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        scores.into_iter().take(limit).collect()
    }

    /// Returns the sectors with the lowest visibility to other sectors (safest spots).
    pub fn blind_spots(&self, limit: usize) -> Vec<(usize, usize)> {
        let mut scores = self.visibility_scores();
        scores.sort_unstable_by(|a, b| a.1.cmp(&b.1));
        scores.into_iter().take(limit).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::Reject;

    #[test]
    fn test_sight_analyzer() {
        // 3 sectors. Data needs ceil(3*3/8) = 2 bytes.
        // visible(a,b) = !bit
        // Let's make all visible except 0 and 2.
        // indices:
        // 0,0 (0) -> 0
        // 0,1 (1) -> 0
        // 0,2 (2) -> 1 (reject)
        // 1,0 (3) -> 0
        // 1,1 (4) -> 0
        // 1,2 (5) -> 0
        // 2,0 (6) -> 1 (reject)
        // 2,1 (7) -> 0
        // 2,2 (8) -> 0
        // byte 0: bit 2=1, bit 6=1 -> 0b01000100 = 68
        // byte 1: bit 0=0 -> 0b00000000 = 0
        let data = vec![68, 0];
        let reject = Reject::parse_lump(&data, 3).unwrap();
        let analyzer = SightAnalyzer::new(&reject);

        let scores = analyzer.visibility_scores();
        assert_eq!(scores[0], (0, 1)); // sees 1
        assert_eq!(scores[1], (1, 2)); // sees 0, 2
        assert_eq!(scores[2], (2, 1)); // sees 1

        let overwatch = analyzer.overwatch_sectors(1);
        assert_eq!(overwatch[0], (1, 2));

        let blind = analyzer.blind_spots(1);
        assert!(blind[0].0 == 0 || blind[0].0 == 2);
    }
}
