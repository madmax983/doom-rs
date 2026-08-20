//! Bounded, non-allocating history tracking for sprite occlusion.
//!
//! When drawing sprites in Doom's 2.5D environment, a single column might
//! pass through multiple sector portals (e.g. looking through a window into another room).
//! Instead of allocating memory for every clip boundary, we track a short history
//! using a fixed-capacity array, dropping older elements if it overflows.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals.
///
/// # Examples
/// ```
/// use doom_renderer::sprite_clip::SpriteClipHistory;
///
/// let mut history = SpriteClipHistory::new();
/// assert_eq!(history.iter().count(), 0);
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

    /// Appends a new clip step to the history.
    ///
    /// If the internal fixed-capacity array is full, the incoming step is dropped
    /// to avoid heap allocations or panics. This gracefully degrades visual accuracy
    /// in impossibly deep sector stacks instead of crashing.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 10, silhouette_height: 32.0 });
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

    /// Retrieves the most recently added clip step, if any.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 10, silhouette_height: 32.0 });
    /// assert_eq!(history.last().unwrap().row, 10);
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the currently active clip steps.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    ///
    /// let history = SpriteClipHistory::new();
    /// for step in history.iter() {
    ///     // Process step...
    /// }
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
