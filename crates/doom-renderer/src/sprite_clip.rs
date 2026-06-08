use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteClipHistory {
    steps: [SpriteClipStep; 8],
    len: usize,
}

impl Default for SpriteClipHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteClipHistory {
    pub const fn new() -> Self {
        Self {
            steps: [SpriteClipStep {
                depth: 0.0,
                row: 0,
                silhouette_height: 0.0,
            }; 8],
            len: 0,
        }
    }

    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_default_are_empty() {
        let history1 = SpriteClipHistory::new();
        let history2 = SpriteClipHistory::default();

        assert_eq!(history1.len, 0);
        assert_eq!(history2.len, 0);
        assert!(history1.last().is_none());
        assert_eq!(history1.iter().count(), 0);
    }

    #[test]
    fn push_adds_steps_and_increases_len() {
        let mut history = SpriteClipHistory::new();

        let step1 = SpriteClipStep {
            depth: 1.0,
            row: 10,
            silhouette_height: 5.0,
        };
        let step2 = SpriteClipStep {
            depth: 2.0,
            row: 20,
            silhouette_height: 10.0,
        };

        history.push(step1);
        assert_eq!(history.len, 1);
        assert_eq!(history.last(), Some(&step1));

        history.push(step2);
        assert_eq!(history.len, 2);
        assert_eq!(history.last(), Some(&step2));

        let items: Vec<_> = history.iter().cloned().collect();
        assert_eq!(items, vec![step1, step2]);
    }

    #[test]
    fn push_drops_extra_clips_beyond_capacity() {
        let mut history = SpriteClipHistory::new();

        // Push exactly capacity (8) items
        for i in 0..8 {
            let step = SpriteClipStep {
                depth: i as f32,
                row: i,
                silhouette_height: 0.0,
            };
            history.push(step);
        }

        assert_eq!(history.len, 8);
        assert_eq!(history.last().unwrap().row, 7);

        // Push one more, it should be dropped
        let overflow_step = SpriteClipStep {
            depth: 99.0,
            row: 99,
            silhouette_height: 0.0,
        };
        history.push(overflow_step);

        assert_eq!(history.len, 8); // Length shouldn't increase
        assert_eq!(history.last().unwrap().row, 7); // Last item should be the 8th item pushed, not overflow_step
    }
}
