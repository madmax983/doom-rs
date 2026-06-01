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
    fn should_start_empty() {
        let history = SpriteClipHistory::new();
        assert_eq!(history.len, 0);
        assert_eq!(history.last(), None);
        assert_eq!(history.iter().count(), 0);
    }

    #[test]
    fn should_push_and_retrieve_steps() {
        let mut history = SpriteClipHistory::default();
        let step1 = SpriteClipStep {
            depth: 10.0,
            row: 5,
            silhouette_height: 20.0,
        };
        history.push(step1);

        assert_eq!(history.len, 1);
        assert_eq!(history.last(), Some(&step1));

        let step2 = SpriteClipStep {
            depth: 15.0,
            row: 8,
            silhouette_height: 25.0,
        };
        history.push(step2);

        assert_eq!(history.len, 2);
        assert_eq!(history.last(), Some(&step2));

        let steps: Vec<_> = history.iter().copied().collect();
        assert_eq!(steps, vec![step1, step2]);
    }

    #[test]
    fn should_drop_extra_clips_when_full() {
        let mut history = SpriteClipHistory::new();
        let mut expected_steps = Vec::new();

        // Fill the history
        for i in 0..8 {
            let step = SpriteClipStep {
                depth: i as f32,
                row: i,
                silhouette_height: 0.0,
            };
            history.push(step);
            expected_steps.push(step);
        }

        assert_eq!(history.len, 8);
        assert_eq!(history.last(), Some(&expected_steps[7]));

        // Push another step, it should be dropped
        let overflow_step = SpriteClipStep {
            depth: 100.0,
            row: 100,
            silhouette_height: 100.0,
        };
        history.push(overflow_step);

        assert_eq!(history.len, 8);
        assert_eq!(history.last(), Some(&expected_steps[7]));
    }
}
