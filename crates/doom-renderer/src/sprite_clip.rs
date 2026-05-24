//! Sprite clipping subsystem.
//!
//! This module provides mechanisms to clip 2D sprites against solid walls,
//! windows, and variable-height sectors to ensure correct occlusion during software rendering.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating `Vec`s on the heap for short sprite clip histories.
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
    /// Creates a new, empty [`SpriteClipHistory`].
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    ///
    /// let history = SpriteClipHistory::new();
    /// assert_eq!(history.iter().count(), 0);
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

    /// Pushes a new [`SpriteClipStep`] into the history.
    ///
    /// If the history is full (capacity of 8), the step is silently dropped to avoid heap allocations.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// let step = SpriteClipStep { depth: 10.0, row: 5, silhouette_height: 1.0 };
    /// history.push(step);
    ///
    /// assert_eq!(history.iter().count(), 1);
    /// assert_eq!(history.last(), Some(&step));
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the last [`SpriteClipStep`] pushed into the history, if any.
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the clip steps in the history.
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
