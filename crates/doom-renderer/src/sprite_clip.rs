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

    fn dummy_step(depth: f32) -> SpriteClipStep {
        SpriteClipStep {
            depth,
            row: 0,
            silhouette_height: 0.0,
        }
    }

    #[test]
    fn new_history_is_empty() {
        let h = SpriteClipHistory::new();
        assert_eq!(h.len, 0);
        assert!(h.last().is_none());
        assert_eq!(h.iter().count(), 0);
    }

    #[test]
    fn default_history_is_empty() {
        let h = SpriteClipHistory::default();
        assert_eq!(h.len, 0);
    }

    #[test]
    fn push_adds_steps() {
        let mut h = SpriteClipHistory::new();
        h.push(dummy_step(1.0));
        assert_eq!(h.len, 1);
        assert_eq!(h.last().unwrap().depth, 1.0);

        h.push(dummy_step(2.0));
        assert_eq!(h.len, 2);
        assert_eq!(h.last().unwrap().depth, 2.0);
    }

    #[test]
    fn push_drops_extra_steps_when_full() {
        let mut h = SpriteClipHistory::new();
        for i in 0..10 {
            h.push(dummy_step(i as f32));
        }

        // Max capacity is 8
        assert_eq!(h.len, 8);
        assert_eq!(h.last().unwrap().depth, 7.0);
    }

    #[test]
    fn iter_returns_all_steps() {
        let mut h = SpriteClipHistory::new();
        h.push(dummy_step(1.0));
        h.push(dummy_step(2.0));

        let steps: Vec<_> = h.iter().collect();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].depth, 1.0);
        assert_eq!(steps[1].depth, 2.0);
    }
}
