//! Fixed-capacity history for tracking sprite clipping against level geometry portals.
//!
//! When rendering sprites in a sector-based engine, a sprite might be drawn behind multiple
//! overlapping portals (like a window with upper and lower frames). The `SpriteClipHistory`
//! records the exact vertical bounds where a sprite column intersects these solid surfaces.
//!
//! # Examples
//!
//! ```
//! use doom_renderer::sprite_clip::SpriteClipHistory;
//! use doom_renderer::render::SpriteClipStep;
//!
//! let mut history = SpriteClipHistory::new();
//! history.push(SpriteClipStep { depth: 100.0, row: 10, silhouette_height: 50.0 });
//!
//! assert_eq!(history.iter().count(), 1);
//! assert_eq!(history.last().unwrap().depth, 100.0);
//! ```
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
    /// Initializes a blank slate for recording clipping boundaries.
    ///
    /// Because this uses `const fn`, it can be safely embedded in static arrays or thread-locals
    /// without requiring heap allocation overhead at runtime.
    ///
    /// ## Examples
    ///
    /// ```
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

    /// Records a new geometric intersection point.
    ///
    /// If the internal array capacity (8) is exceeded, additional clipping steps are silently
    /// ignored to prevent costly dynamic heap allocations in hot rendering paths. In standard Doom
    /// maps, a single column rarely overlaps more than 2-4 portals.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 50.0, row: 0, silhouette_height: 10.0 });
    /// assert_eq!(history.last().unwrap().depth, 50.0);
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Inspects the furthest back-to-front portal intersection recorded so far.
    ///
    /// This is typically used by the sprite rendering pipeline to determine if the current
    /// sprite fragment is fully occluded by the most recently processed solid wall.
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
    /// history.push(SpriteClipStep { depth: 100.0, row: 5, silhouette_height: 20.0 });
    /// assert!(history.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Traverses the sequence of recorded clipping steps from nearest to furthest.
    ///
    /// Yields references to each step in the order they were inserted, allowing the renderer
    /// to compose a final visibility mask for a given sprite vertical column.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 10.0, row: 1, silhouette_height: 5.0 });
    /// history.push(SpriteClipStep { depth: 20.0, row: 2, silhouette_height: 5.0 });
    ///
    /// let depths: Vec<_> = history.iter().map(|step| step.depth).collect();
    /// assert_eq!(depths, vec![10.0, 20.0]);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
