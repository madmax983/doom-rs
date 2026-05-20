//! Sprite clipping history for the software renderer.
//!
//! # The Z-Buffer's Ancestor
//!
//! Doom's software renderer doesn't have a modern depth buffer. Instead, it draws the world
//! front-to-back. When drawing sprites (which are drawn back-to-front after the walls),
//! the renderer needs to know if a wall is occluding a given vertical column of the sprite.
//!
//! The `SpriteClipHistory` struct provides a fast, stack-allocated, `ArrayVec`-like storage
//! for clipping window depths. This avoids costly heap allocations (`Vec`) during the
//! critical hot path of sprite rasterization.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals.
///
/// ## Examples
///
/// ```
/// use doom_renderer::render::SpriteClipStep;
/// use doom_renderer::sprite_clip::SpriteClipHistory;
///
/// let mut history = SpriteClipHistory::new();
/// history.push(SpriteClipStep {
///     depth: 100.0,
///     row: 0,
///     silhouette_height: 50.0,
/// });
///
/// assert_eq!(history.iter().count(), 1);
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
    /// If the history is full (more than 8 clips), it ignores further pushes to avoid heap allocations.
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns the last pushed clip step, or `None` if the history is empty.
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the stored clip steps.
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
