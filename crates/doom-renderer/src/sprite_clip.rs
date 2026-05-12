//! Sprite clipping structures for rendering sprites correctly behind multiple portals.
//!
//! When the software renderer draws the BSP tree, it draws walls from front to back,
//! but sprites must be drawn back to front. To handle sprites clipping behind walls,
//! the renderer records a "clip history" for each screen column.
//!
//! # Purpose
//! This module provides the `SpriteClipHistory` structure, which acts as a small,
//! stack-allocated array (like `ArrayVec`) to store clipping steps for a single
//! vertical screen column. Since a single column rarely clips through more than 4-8
//! portals, this avoids heap allocations in the tightest rendering loop.

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

    /// Pushes a new clip step onto the history.
    ///
    /// If the history is already full (contains 8 steps), the step is silently dropped
    /// to avoid a panic or a heap allocation. In practice, visual artifacts from dropping
    /// the 9th portal clip on a single column are negligible.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// let step = SpriteClipStep {
    ///     depth: 100.0,
    ///     row: 50,
    ///     silhouette_height: 32.0,
    /// };
    /// history.push(step);
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

    /// Returns a reference to the most recently pushed clip step, or `None` if empty.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// assert_eq!(history.last(), None);
    ///
    /// history.push(SpriteClipStep { depth: 50.0, row: 10, silhouette_height: 20.0 });
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
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 10, silhouette_height: 20.0 });
    ///
    /// let mut iter = history.iter();
    /// assert!(iter.next().is_some());
    /// assert!(iter.next().is_none());
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
