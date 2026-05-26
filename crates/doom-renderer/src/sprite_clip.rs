//! Sprite clipping history for rendering optimization.
//!
//! Because sprites are drawn back-to-front, they must be clipped against
//! walls that are drawn front-to-back. This history structure records
//! the silhouette of walls as they are drawn, avoiding allocations per column.
//!
//! # Examples
//!
//! ```
//! use doom_renderer::sprite_clip::SpriteClipHistory;
//! use doom_renderer::render::SpriteClipStep;
//!
//! let mut history = SpriteClipHistory::new();
//! history.push(SpriteClipStep { depth: 10.0, row: 50, silhouette_height: 100.0 });
//!
//! assert_eq!(history.last().unwrap().row, 50);
//! ```

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
