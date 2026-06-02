//! Sprite clipping structures for rendering.
//!
//! As the software renderer traverses the BSP front-to-back, it encounters portal
//! openings (two-sided segs) that act as windows. Sprites behind these windows must
//! be clipped so they don't render over solid walls in front of them.
//!
//! [`SpriteClipHistory`] records these window boundary "steps" per-column.
//! Because a column rarely clips through more than 4-8 portals before hitting a solid
//! wall or reaching the back of the scene, this history is backed by a fixed-size array
//! to avoid heap allocations in the hot rendering loop.

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

    /// Pushes a new [`SpriteClipStep`] onto the history.
    ///
    /// If the history exceeds its fixed capacity (8 clips), the step is silently dropped.
    /// This prevents heap allocations and unbounded memory usage during deep portal traversals,
    /// at the extremely minor cost of slight visual artifacts if a sprite clips behind more
    /// than 8 successive narrow windows in a single column.
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
    ///     row: 40,
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

    /// Returns a reference to the most recently pushed [`SpriteClipStep`].
    ///
    /// This is used during BSP traversal to check if a new portal opening shares
    /// the same bounds as the previous one, allowing the renderer to avoid storing redundant clips.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 40, silhouette_height: 32.0 });
    ///
    /// let last = history.last().unwrap();
    /// assert_eq!(last.row, 40);
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
    /// When rendering a sprite column, the engine iterates over these clips front-to-back
    /// to calculate the final visible bounds of the sprite fragment.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 10, silhouette_height: 10.0 });
    /// history.push(SpriteClipStep { depth: 150.0, row: 20, silhouette_height: 15.0 });
    ///
    /// let depths: Vec<f32> = history.iter().map(|step| step.depth).collect();
    /// assert_eq!(depths, vec![50.0, 150.0]);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
