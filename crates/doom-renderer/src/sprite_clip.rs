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
    fn new_creates_empty_history() {
        let history = SpriteClipHistory::new();
        assert_eq!(history.len, 0);
        assert!(history.last().is_none());
        assert_eq!(history.iter().count(), 0);
    }

    #[test]
    fn default_creates_empty_history() {
        let history = SpriteClipHistory::default();
        assert_eq!(history.len, 0);
    }

    #[test]
    fn push_adds_elements_up_to_capacity() {
        let mut history = SpriteClipHistory::new();

        for i in 0..8usize {
            let step = SpriteClipStep {
                depth: i as f32,
                row: i as i32,
                silhouette_height: (i * 2) as f32,
            };
            history.push(step);
            assert_eq!(history.len, i + 1);
            assert_eq!(history.last().unwrap(), &step);
        }

        // Test over capacity - should be ignored
        let overflow_step = SpriteClipStep {
            depth: 99.0,
            row: 99,
            silhouette_height: 99.0,
        };
        history.push(overflow_step);
        assert_eq!(history.len, 8);
        assert_ne!(history.last().unwrap(), &overflow_step);
        assert_eq!(history.last().unwrap().row, 7);
    }

    #[test]
    fn iter_yields_all_pushed_elements() {
        let mut history = SpriteClipHistory::new();

        for i in 0..3i32 {
            history.push(SpriteClipStep {
                depth: i as f32,
                row: i,
                silhouette_height: 0.0,
            });
        }

        let mut iter = history.iter();
        assert_eq!(iter.next().unwrap().row, 0);
        assert_eq!(iter.next().unwrap().row, 1);
        assert_eq!(iter.next().unwrap().row, 2);
        assert!(iter.next().is_none());
    }
}
