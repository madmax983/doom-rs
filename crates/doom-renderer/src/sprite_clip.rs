//! Structures for maintaining short clipping histories for sprite rendering.
//!
//! # Context
//! During sprite rendering, columns need to know if they are clipped by portals
//! (like windows) that they are drawn behind. `SpriteClipHistory` avoids heap
//! allocations by using a small fixed-size array.

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
    /// Initializes a blank clipping history ready for a new column rendering pass.
    ///
    /// # Examples
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

    /// Appends a new clipping boundary, representing a portal transition that this column has traversed.
    ///
    /// # Details
    /// Extra clips beyond the internal capacity (8) are silently dropped. In practice, Doom
    /// maps rarely have enough overlapping 3D portals to exceed this, and dropping them
    /// avoids expensive heap allocations and panics in the hot rendering loop.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// // Simulate crossing into a 128-unit tall window 50 pixels down the screen
    /// history.push(SpriteClipStep { depth: 10.0, row: 50, silhouette_height: 128.0 });
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Inspects the most recently traversed portal boundary to determine depth testing for sprites.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
    /// history.push(SpriteClipStep { depth: 10.0, row: 50, silhouette_height: 128.0 });
    /// assert!(history.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Traverses the accumulated portal boundaries to check if a sprite falls behind any of them.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 10.0, row: 50, silhouette_height: 128.0 });
    /// assert_eq!(history.iter().count(), 1);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
