//! Provides structures to efficiently track portal clipping planes for sprites.
//!
//! # The Invisible Silhouette
//!
//! When rendering a multi-layered world from back to front, sprites need to properly
//! clip against floors and ceilings that are logically in front of them, even though
//! the floor itself may have been drawn long before the sprite.
//!
//! `SpriteClipHistory` solves this by recording an array of `SpriteClipStep` planes
//! for a particular screen column. Instead of allocating arbitrary vectors on the
//! heap for every visible sprite column (a performance disaster), it uses a fixed-capacity
//! stack-allocated array that perfectly matches the typical maximum portal depth
//! of the Doom engine.

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
    /// Creates a new, empty [`SpriteClipHistory`] stack.
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

    /// Pushes a new clipping step onto the top of the history stack.
    ///
    /// If the stack has reached its maximum capacity of 8 clips, additional clips
    /// are silently dropped. In the context of Doom's geometry, rendering through
    /// more than 8 simultaneous vertical portal transitions in a single column
    /// is extremely rare. Dropping clips prevents heap allocations while introducing
    /// negligible visual artifacts in extreme edge cases.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// let step = SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 64.0 };
    /// history.push(step);
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

    /// Returns a reference to the most recently added [`SpriteClipStep`],
    /// or `None` if the history is empty.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
    ///
    /// let step = SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 64.0 };
    /// history.push(step);
    /// assert_eq!(history.last().unwrap().depth, 100.0);
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Returns an iterator over the valid [`SpriteClipStep`]s in the history,
    /// in the order they were pushed.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// let step1 = SpriteClipStep { depth: 100.0, row: 50, silhouette_height: 64.0 };
    /// let step2 = SpriteClipStep { depth: 200.0, row: 60, silhouette_height: 128.0 };
    /// history.push(step1);
    /// history.push(step2);
    ///
    /// let mut iter = history.iter();
    /// assert_eq!(iter.next().unwrap().depth, 100.0);
    /// assert_eq!(iter.next().unwrap().depth, 200.0);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
