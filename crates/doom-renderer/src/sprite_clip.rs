//! Sprite clipping structures for portal rendering.
//!
//! Because Doom uses a portal-based software renderer instead of a Z-buffer, sprites
//! must be clipped against the walls and floors they are rendered behind. This module
//! provides the structures used to track the clip regions during BSP traversal.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to track the depth and bounds of clips.
///
/// We avoid allocating `Vec`s on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals before
/// hitting a solid wall.
///
/// # Capacity Limitations
///
/// This structure will silently drop extra clips if more than 8 steps are pushed,
/// as a failsafe to avoid heap allocations in the hot rendering loop.
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
    /// If the history is already at its maximum capacity of 8 clips, the new clip
    /// is silently ignored.
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Returns the most recently pushed [`SpriteClipStep`], or `None` if empty.
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the active [`SpriteClipStep`]s.
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
