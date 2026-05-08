//! Sprite clipping structures for portal rendering.
//!
//! When rendering sprites in Doom, they can be partially occluded by map geometry (like windows or ledges).
//! Because sprites are rendered after the walls (BSP traversal), the engine needs to record where the solid
//! walls were drawn so that sprites can be clipped against them.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
///
/// In Doom, a single column rarely clips through more than 4-8 portals. This structure provides
/// a fixed-size buffer to record the clipping steps for a sprite column.
///
/// ## Examples
/// ```
/// use doom_renderer::sprite_clip::SpriteClipHistory;
/// use doom_renderer::render::SpriteClipStep;
///
/// let mut history = SpriteClipHistory::new();
/// let step = SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 10.0 };
/// history.push(step);
/// assert_eq!(history.last().unwrap().depth, 100.0);
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
    /// ## Examples
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

    /// Pushes a new [`SpriteClipStep`] onto the history buffer.
    ///
    /// If the buffer is full (exceeds 8 steps), the step is silently dropped
    /// to avoid heap allocations. This is safe because deeply nested portals
    /// are visually insignificant and extremely rare in classic Doom architecture.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 10, silhouette_height: 5.0 });
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns a reference to the most recently pushed [`SpriteClipStep`], or `None` if the history is empty.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// assert_eq!(history.last(), None);
    /// history.push(SpriteClipStep { depth: 50.0, row: 10, silhouette_height: 5.0 });
    /// assert!(history.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the recorded [`SpriteClipStep`]s.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 10, silhouette_height: 5.0 });
    ///
    /// for step in history.iter() {
    ///     println!("Depth: {}", step.depth);
    /// }
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
