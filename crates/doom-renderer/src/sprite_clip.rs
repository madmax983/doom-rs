//! Tracks portals to clip sprites correctly without heap allocations.
//!
//! This module provides [`SpriteClipHistory`], a fixed-capacity structure that
//! records the screen bounds of portal openings for a single screen column.
//! When a sprite is drawn, it uses this history to avoid rendering pixels that
//! would fall behind a wall or door frame.

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

    /// Appends a new clipping step to the history.
    ///
    /// If the history is already at maximum capacity, the step is silently
    /// discarded to avoid a heap allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// let step = SpriteClipStep {
    ///     depth: 100.0,
    ///     row: 50,
    ///     silhouette_height: 20.0,
    /// };
    /// history.push(step);
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently added clip step, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// let step = SpriteClipStep {
    ///     depth: 100.0,
    ///     row: 50,
    ///     silhouette_height: 20.0,
    /// };
    /// history.push(step);
    /// assert!(history.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the stored clip steps.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// let history = SpriteClipHistory::new();
    /// assert_eq!(history.iter().count(), 0);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
