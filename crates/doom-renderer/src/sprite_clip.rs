//! Sprite clipping history tracking to manage pixel overdraw.

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
    /// Creates a new, empty array for recording sprite column occlusion segments.
    ///
    /// # Details
    /// Because `SpriteClipHistory` is allocated heavily on the stack during rendering
    /// (e.g. one per vertical line per sprite cache block), this is implemented as a
    /// zero-cost array initialization.
    ///
    /// # Examples
    /// ```rust
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

    /// # Context
    /// Records an occlusion edge.
    ///
    /// # Details
    /// Pushes a new clip step into the history. If the internal stack buffer is
    /// completely filled, any further requests to record clips are silently dropped to avoid
    /// heap allocations and potential panics.
    ///
    /// # Examples
    /// ```rust
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 5, silhouette_height: 10.0 });
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

    /// # Context
    /// Used when clipping sprites against geometry that only obscure part of the sprite.
    ///
    /// # Details
    /// In doom rendering, sprites might only be partially obscured. This method gives you access to the last valid clip.
    ///
    /// # Examples
    /// ```rust
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
    /// assert_eq!(history.last().unwrap().row, 10);
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// # Context
    /// Iterates over the clipping steps applied to a sprite column.
    ///
    /// # Details
    /// Used during rendering to apply back-to-front overdraw limits.
    ///
    /// # Examples
    /// ```rust
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 5, silhouette_height: 10.0 });
    ///
    /// assert_eq!(history.iter().count(), 1);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
