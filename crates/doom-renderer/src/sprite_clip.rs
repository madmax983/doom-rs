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
    fn should_initialize_empty_history() {
        let history = SpriteClipHistory::new();
        assert_eq!(history.len, 0);
        assert!(history.last().is_none());
        assert_eq!(history.iter().count(), 0);

        let default_history = SpriteClipHistory::default();
        assert_eq!(default_history, history);
    }

    #[test]
    fn should_push_and_retain_elements_up_to_capacity() {
        let mut history = SpriteClipHistory::new();

        for i in 0..8 {
            history.push(SpriteClipStep {
                depth: i as f32,
                row: i,
                silhouette_height: i as f32,
            });
            assert_eq!(history.len, (i + 1) as usize);
            assert_eq!(history.last().unwrap().row, i);
        }

        assert_eq!(history.iter().count(), 8);
        for (i, step) in history.iter().enumerate() {
            assert_eq!(step.row, i as i32);
        }
    }

    #[test]
    fn should_drop_elements_when_pushed_beyond_capacity() {
        let mut history = SpriteClipHistory::new();

        for i in 0..10 {
            history.push(SpriteClipStep {
                depth: i as f32,
                row: i,
                silhouette_height: i as f32,
            });
        }

        assert_eq!(history.len, 8);
        assert_eq!(history.last().unwrap().row, 7);
        assert_eq!(history.iter().count(), 8);
    }
}
