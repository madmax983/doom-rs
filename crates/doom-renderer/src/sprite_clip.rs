//! Sprite clipping stack.
//!
//! Provides a history stack for tracking the sprite depth clipping across multiple portals to avoid heap allocations.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals.
///
/// ## Examples
/// ```
/// use doom_renderer::sprite_clip::SpriteClipHistory;
/// use doom_renderer::render::SpriteClipStep;
///
/// let mut history = SpriteClipHistory::new();
/// history.push(SpriteClipStep { depth: 10.0, row: 5, silhouette_height: 20.0 });
/// assert_eq!(history.last().unwrap().row, 5);
/// ```
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
    /// Creates a new, empty `SpriteClipHistory`.
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

    /// Pushes a new `SpriteClipStep` onto the history stack.
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently pushed `SpriteClipStep`, if any.
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the stored `SpriteClipStep`s.
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
