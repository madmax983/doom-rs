//! Provides structures for tracking the clipping history of vertical sprite columns.
//!
//! Because sprites are drawn back-to-front and may be occluded by complex world geometry (multiple portal transitions),
//! the renderer needs to keep a history of depth values and clip bounds for each column to know what portions of
//! the sprite to render. `SpriteClipHistory` achieves this without allocating on the heap for every column.

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

    /// Pushes a new `SpriteClipStep` onto the history.
    ///
    /// If the history is full (capacity 8), the new step is silently ignored to prevent heap allocation.
    /// In typical Doom maps, 8 layers of clipping are rarely exceeded for a single sprite column.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep {
    ///     depth: 100.0,
    ///     row: 10,
    ///     silhouette_height: 50.0,
    /// });
    ///
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

    /// Returns a reference to the most recently added `SpriteClipStep`, or `None` if empty.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// assert_eq!(history.last(), None);
    ///
    /// history.push(SpriteClipStep {
    ///     depth: 100.0,
    ///     row: 10,
    ///     silhouette_height: 50.0,
    /// });
    /// assert!(history.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the `SpriteClipStep`s currently in the history.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep {
    ///     depth: 100.0,
    ///     row: 10,
    ///     silhouette_height: 50.0,
    /// });
    ///
    /// let mut iter = history.iter();
    /// assert_eq!(iter.next().unwrap().row, 10);
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
