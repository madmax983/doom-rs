//! High-performance sprite clipping history buffer.
//!
//! When rendering sprites in Doom, a sprite might be drawn behind multiple portals
//! (like windows or ledges) that clip its visibility. To render the sprite correctly,
//! we must track these clipping events. This module provides a fast, stack-allocated
//! history buffer that records each time a sprite gets clipped, allowing the renderer
//! to slice the sprite correctly.

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
    /// Initializes an empty clip history on the stack.
    ///
    /// The buffer has a fixed capacity of 8 clips, which is more than enough
    /// for typical Doom maps where a single column rarely clips through
    /// more than a few portals simultaneously.
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

    /// Records a new clipping plane encountered while traversing BSP portals.
    ///
    /// If the history buffer is full (exceeds 8 clips), subsequent clips are safely
    /// ignored to prevent heap allocations. This fallback is acceptable because
    /// overflowing this limit in real gameplay is virtually non-existent.
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
    ///     row: 24,
    ///     silhouette_height: 32.0,
    /// });
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

    /// Retrieves the most recent clipping plane that was recorded, if any.
    ///
    /// This is typically used by the masked rendering pipeline to determine
    /// the active depth boundaries of the sprite fragment currently being drawn.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep {
    ///     depth: 50.0,
    ///     row: 10,
    ///     silhouette_height: 64.0,
    /// });
    ///
    /// let last = history.last().unwrap();
    /// assert_eq!(last.row, 10);
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Streams portal depths to slice tall sprites during masked rendering.
    ///
    /// Yields references to each recorded `SpriteClipStep` in the order they were pushed.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 20.0, row: 5, silhouette_height: 12.0 });
    /// history.push(SpriteClipStep { depth: 40.0, row: 15, silhouette_height: 32.0 });
    ///
    /// let depths: Vec<f32> = history.iter().map(|step| step.depth).collect();
    /// assert_eq!(depths, vec![20.0, 40.0]);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
