//! Sprite clipping history for rendering optimization.
//!
//! # Context
//!
//! When rendering sprites in Doom, a single column of a sprite might clip through multiple
//! window portals (e.g., looking through a series of windows). To avoid heap allocations
//! for tracking these clip steps, we use a fixed-capacity, stack-allocated structure.
//!
//! # Details
//!
//! The `SpriteClipHistory` acts as a specialized `ArrayVec`. It can store up to 8
//! clip steps, which is empirically sufficient for almost all Doom rendering scenarios.

use crate::render::SpriteClipStep;

/// A stack-allocated history of sprite clipping steps.
///
/// # Context
///
/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals.
///
/// # Examples
///
/// ```
/// use doom_renderer::sprite_clip::SpriteClipHistory;
/// use doom_renderer::render::SpriteClipStep;
///
/// let mut history = SpriteClipHistory::new();
/// history.push(SpriteClipStep { depth: 10.0, row: 5, silhouette_height: 20.0 });
/// assert!(history.last().is_some());
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

    /// Pushes a new clip step onto the history.
    ///
    /// # Details
    ///
    /// If the history is already full (contains 8 steps), the new step is silently ignored
    /// to prevent heap allocations or panics, as deeper clips are rarely visible.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 5.0, row: 10, silhouette_height: 15.0 });
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently pushed clip step, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    ///
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

    /// Returns an iterator over the stored clip steps.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    ///
    /// let history = SpriteClipHistory::new();
    /// for step in history.iter() {
    ///     // Process step
    /// }
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
