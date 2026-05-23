//! Provides functionality to track and store sprite clipping steps.
//!
//! A sprite can clip against multiple portal boundaries. This module provides a fast, no-allocation
//! structure to record these boundary depths.

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
    /// Creates a new, empty `SpriteClipHistory`.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// let history = SpriteClipHistory::new();
    /// assert_eq!(history.last(), None);
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

    /// Pushes a new clipping step onto the history.
    ///
    /// If the history is already at its maximum capacity, the new step is silently ignored
    /// to avoid heap allocations. This is acceptable as Doom sprites rarely clip through
    /// more than a few portals.
    ///
    /// # Arguments
    ///
    /// * `step` - The `SpriteClipStep` to add.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 10.0 });
    /// assert!(history.last().is_some());
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Inspects the most recent clipping boundary.
    ///
    /// This is typically used to determine if a sprite is occluded by the immediate foreground.
    ///
    /// # Returns
    ///
    /// * `Some(&SpriteClipStep)` if the history is not empty.
    /// * `None` if the history is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// let history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Traverses the recorded portal boundaries from foreground to background.
    ///
    /// The iterator yields steps in the order they were added.
    ///
    /// # Returns
    ///
    /// * `core::slice::Iter<'_, SpriteClipStep>`
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// let history = SpriteClipHistory::new();
    /// for step in history.iter() {
    ///     println!("Depth: {}", step.depth);
    /// }
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
