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
    fn sprite_clip_history_tests() {
        let mut history = SpriteClipHistory::default();
        assert_eq!(history.last(), None);
        let step = SpriteClipStep {
            depth: 1.0,
            row: 10,
            silhouette_height: 20.0,
        };
        history.push(step);
        assert_eq!(history.last(), Some(&step));
        for i in 0..10 {
            history.push(SpriteClipStep {
                depth: i as f32,
                row: i,
                silhouette_height: 0.0,
            });
        }
        let iter = history.iter();
        assert_eq!(iter.len(), 8);
    }
}
