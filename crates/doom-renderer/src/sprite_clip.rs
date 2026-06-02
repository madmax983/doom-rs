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
    fn default_and_new_are_empty() {
        let history1 = SpriteClipHistory::new();
        let history2 = SpriteClipHistory::default();

        assert_eq!(history1.len, 0);
        assert_eq!(history2.len, 0);
        assert!(history1.last().is_none());
        assert_eq!(history1.iter().count(), 0);
    }

    #[test]
    fn push_within_capacity_updates_len_and_last() {
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
    }

    #[test]
    fn push_beyond_capacity_drops_extra() {
        let mut history = SpriteClipHistory::new();

        // Fill up to capacity (8)
        for i in 0..8 {
            history.push(SpriteClipStep {
                depth: i as f32,
                row: i,
                silhouette_height: 0.0,
            });
        }

        assert_eq!(history.len, 8);
        assert_eq!(history.last().expect("must have last element").row, 7);

        // Push one more, should be ignored
        let extra_step = SpriteClipStep {
            depth: 99.0,
            row: 99,
            silhouette_height: 99.0,
        };
        history.push(extra_step);

        assert_eq!(history.len, 8); // length shouldn't increase
        assert_ne!(history.last(), Some(&extra_step)); // last item shouldn't be the extra one
        assert_eq!(history.last().expect("must have last element").row, 7);
    }

    #[test]
    fn iter_returns_pushed_items_in_order() {
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
        history.push(step2);

        let items: Vec<_> = history.iter().copied().collect();
        assert_eq!(items, vec![step1, step2]);
    }
}
