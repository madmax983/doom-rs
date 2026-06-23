//! Manages short, allocation-free histories of sprite clipping portals.

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
    /// # Context
    /// Creates a new, empty history array with capacity for 8 clip steps.
    ///
    /// # Examples
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

    /// # Context
    /// Appends a new clip step to the history. If the internal array is full, subsequent pushes are silently dropped to avoid heap allocations.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 10.0, row: 50, silhouette_height: 32.0 });
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

    /// # Context
    /// Retrieves a reference to the most recently pushed clip step, or `None` if the history is empty.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
    /// history.push(SpriteClipStep { depth: 10.0, row: 50, silhouette_height: 32.0 });
    /// assert!(history.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// # Context
    /// Returns an iterator over the clip steps in the history, from oldest to newest.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 10.0, row: 50, silhouette_height: 32.0 });
    /// assert_eq!(history.iter().count(), 1);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
