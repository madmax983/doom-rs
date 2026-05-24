//! Sprite clipping buffer to track depth testing for 2D sprites against BSP level geometry.
//!
//! Because Doom draws columns of sprites, each column has a depth (`SpriteClipStep`)
//! based on where it falls behind walls or step heights. This module provides a fast,
//! heap-free stack (`SpriteClipHistory`) to track recent clip thresholds for a column.

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
    /// Creates a new, empty sprite clip history.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// let history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
    /// ```
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

    /// Pushes a new clipping step onto the history stack.
    ///
    /// If the stack exceeds the maximum capacity of 8 steps, additional steps are silently
    /// dropped. In practice, Doom columns rarely overlap more than a handful of portals.
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently pushed clipping step, or `None` if empty.
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the recorded clipping steps.
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
