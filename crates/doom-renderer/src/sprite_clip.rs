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
    fn should_initialize_empty() {
        let history = SpriteClipHistory::new();
        assert_eq!(history.len, 0);
        assert!(history.last().is_none());
        assert_eq!(history.iter().count(), 0);

        let default_history = SpriteClipHistory::default();
        assert_eq!(default_history.len, 0);
    }

    #[test]
    fn should_push_and_retrieve_last() {
        let mut history = SpriteClipHistory::new();
        let step1 = SpriteClipStep {
            depth: 10.0,
            row: 5,
            silhouette_height: 20.0,
        };
        let step2 = SpriteClipStep {
            depth: 20.0,
            row: 10,
            silhouette_height: 30.0,
        };

        history.push(step1);
        assert_eq!(history.len, 1);
        assert_eq!(history.last(), Some(&step1));

        history.push(step2);
        assert_eq!(history.len, 2);
        assert_eq!(history.last(), Some(&step2));
    }

    #[test]
    fn should_iterate_in_order() {
        let mut history = SpriteClipHistory::new();
        let step1 = SpriteClipStep {
            depth: 10.0,
            row: 5,
            silhouette_height: 20.0,
        };
        let step2 = SpriteClipStep {
            depth: 20.0,
            row: 10,
            silhouette_height: 30.0,
        };

        history.push(step1);
        history.push(step2);

        let mut iter = history.iter();
        assert_eq!(iter.next(), Some(&step1));
        assert_eq!(iter.next(), Some(&step2));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn should_drop_clips_when_full() {
        let mut history = SpriteClipHistory::new();
        let base_step = SpriteClipStep {
            depth: 10.0,
            row: 5,
            silhouette_height: 20.0,
        };

        // Push 8 items to fill it
        for _ in 0..8 {
            history.push(base_step);
        }

        assert_eq!(history.len, 8);
        assert_eq!(history.last(), Some(&base_step));

        // Push a 9th item, which should be ignored
        let overflow_step = SpriteClipStep {
            depth: 99.0,
            row: 99,
            silhouette_height: 99.0,
        };
        history.push(overflow_step);

        assert_eq!(history.len, 8);
        assert_eq!(history.last(), Some(&base_step)); // Last item is still the 8th base_step, not overflow_step
    }
}
