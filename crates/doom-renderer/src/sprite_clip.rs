//! Sprite clipping history for accurate depth-sorted rendering.
//!
//! Tracks how a vertical column of pixels for a sprite has been clipped by
//! level geometry (portals) as the renderer traverses front-to-back.

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

    /// Pushes a new clip step onto the history if there is capacity.
    ///
    /// If the history is already full (reached 8 steps), the step is silently
    /// dropped to avoid heap allocations, which is an acceptable degradation
    /// in Doom's heavily portal-based engine.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 128.0 });
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns the most recently added clip step, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// assert_eq!(history.last(), None);
    ///
    /// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 128.0 });
    /// assert!(history.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Iterates over all recorded clip steps from oldest to newest.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// let history = SpriteClipHistory::new();
    /// for step in history.iter() {
    ///     println!("{:?}", step);
    /// }
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
