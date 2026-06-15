//! # Sprite Clipping
//!
//! Handles vertical boundary clipping for rendering sprites that are partially occluded
//! by mid-textures or sectors with different floor/ceiling heights.
//!
//! Rather than complex per-pixel depth buffers, Doom uses a list of 1D horizontal ranges
//! and vertical clipping posts for sprites. As the BSP traverses front-to-back, solid
//! walls occlude the view, and mid-textures/steps generate new clipping posts.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals.
///
/// # Context
/// During BSP traversal, if a solid wall or a portal with differing floor/ceiling heights is drawn,
/// it affects the vertical clipping boundaries for sprites rendered behind it.
/// Because sprites are drawn back-to-front later, we must store the clipping boundaries generated
/// by front-to-back wall rendering.
///
/// # Examples
/// ```
/// use doom_renderer::sprite_clip::SpriteClipHistory;
/// use doom_renderer::render::SpriteClipStep;
///
/// let mut history = SpriteClipHistory::new();
/// history.push(SpriteClipStep {
///     depth: 100.0,
///     row: 50,
///     silhouette_height: 32.0,
/// });
///
/// assert_eq!(history.last().unwrap().row, 50);
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
    /// # Details
    /// Pre-allocates a fixed size array of 8 clip steps. 8 is generally sufficient
    /// for vanilla Doom levels, as columns rarely intersect more than 8 distinct portals.
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
    /// If the fixed-size array is full (8 elements), subsequent pushes are silently ignored.
    /// This is an intentional optimization to avoid heap allocations in the hot path,
    /// trading visual accuracy in extreme edge cases for consistent performance.
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns the most recently pushed clip step, or `None` if the history is empty.
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the clip steps in the history.
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
