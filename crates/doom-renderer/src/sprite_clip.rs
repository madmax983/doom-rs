//! Sprite clipping history buffer.
//!
//! Provides a small stack-allocated history of sprite clipping steps.

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
    /// The history is allocated on the stack with a fixed capacity (currently 8).
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

    /// Records a new sprite clipping event.
    ///
    /// Used by the renderer during BSP traversal to keep track of portals the sprite is clipped against.
    /// If the fixed-size buffer is full, older clips are preserved and the new step is silently dropped
    /// to avoid heap allocations.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep {
    ///     depth: 100.0,
    ///     row: 50,
    ///     silhouette_height: 25.0,
    /// });
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

    /// Inspects the most recently recorded clipping event.
    ///
    /// This is frequently used during column rendering to determine if a new portal depth
    /// actually occludes the current sprite slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
    ///
    /// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 25.0 });
    /// assert_eq!(history.last().unwrap().row, 50);
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Iterates through the chronological history of sprite clipping events.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 25.0 });
    /// history.push(SpriteClipStep { depth: 200.0, row: 60, silhouette_height: 35.0 });
    ///
    /// for step in history.iter() {
    ///     // Process clip steps...
    /// }
    /// assert_eq!(history.iter().count(), 2);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
