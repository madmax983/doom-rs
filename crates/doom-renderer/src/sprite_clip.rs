//! Structures for maintaining a history of sprite clipping operations.
//!
//! This module provides `SpriteClipHistory`, which is a manual ArrayVec-like structure
//! to avoid allocating Vecs on the heap for short sprite clip histories. In Doom, a single
//! column rarely clips through more than 4-8 portals.

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
    /// Creates a new, empty [`SpriteClipHistory`].
    ///
    /// ## Examples
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

    /// Appends a new `SpriteClipStep` to the history.
    ///
    /// If the history is already at its maximum capacity (8), the extra clip will be silently dropped
    /// to avoid heap allocations.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep {
    ///     depth: 10.0,
    ///     row: 50,
    ///     silhouette_height: 32.0,
    /// });
    /// assert_eq!(history.iter().count(), 1);
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently added `SpriteClipStep`, or `None` if the history is empty.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
    ///
    /// let step = SpriteClipStep { depth: 10.0, row: 50, silhouette_height: 32.0 };
    /// history.push(step);
    /// assert_eq!(history.last().unwrap().depth, 10.0);
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the stored `SpriteClipStep`s.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 10.0, row: 50, silhouette_height: 32.0 });
    /// history.push(SpriteClipStep { depth: 20.0, row: 60, silhouette_height: 16.0 });
    ///
    /// let mut iter = history.iter();
    /// assert_eq!(iter.next().unwrap().depth, 10.0);
    /// assert_eq!(iter.next().unwrap().depth, 20.0);
    /// assert!(iter.next().is_none());
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
