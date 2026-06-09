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
        assert_eq!(history1.last(), None);
        assert_eq!(history1.iter().count(), 0);
    }

    #[test]
    fn push_adds_elements_up_to_capacity() {
        let mut history = SpriteClipHistory::new();

        for i in 0..8 {
            history.push(SpriteClipStep {
                depth: i as f32,
                row: i,
                silhouette_height: i as f32,
            });
            assert_eq!(history.len, i as usize + 1);
            assert_eq!(history.last().expect("last item should exist").row, i);
        }

        let items: Vec<_> = history.iter().collect();
        assert_eq!(items.len(), 8);
        assert_eq!(items[0].row, 0);
        assert_eq!(items[7].row, 7);

        // Pushing beyond capacity should drop the item without panic
        history.push(SpriteClipStep {
            depth: 99.0,
            row: 99,
            silhouette_height: 99.0,
        });

        assert_eq!(history.len, 8);
        assert_eq!(history.last().expect("last item should exist").row, 7);
    }
}
