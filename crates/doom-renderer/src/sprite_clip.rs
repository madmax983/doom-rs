//! Provides a specialized array-backed structure for tracking sprite clipping depth.

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
    /// ## Examples
    ///
    /// ```rust
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// let history = SpriteClipHistory::new();
    /// assert!(history.last().is_none());
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

    /// Records a new visual portal intersection.
    ///
    /// When rendering columns, the engine must track depth to properly obscure objects
    /// standing behind walls. This efficiently logs a new depth threshold, silently dropping
    /// extremely deep portal traversals to protect heap allocations.
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Retrieves the most recently recorded portal threshold.
    ///
    /// This is used during the final compositing phase to determine if an actor
    /// is standing in front of, or behind, the most recently drawn solid surface.
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Streams the accumulated portal depths from front to back.
    ///
    /// By scanning these layers during masked rendering, the engine can correctly slice
    /// tall sprites that intersect multiple floor heights or windows.
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
