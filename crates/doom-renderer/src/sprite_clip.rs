//! Sprite clipping bounds.
//!
//! This module tracks clipping bounds against portals.

use crate::render::SpriteClipStep;

/// A manual ArrayVec-like structure to avoid allocating Vecs on the heap for short sprite clip histories.
/// In Doom, a single column rarely clips through more than 4-8 portals.
///
/// # Examples
/// ```
/// use doom_renderer::sprite_clip::SpriteClipHistory;
///
/// let history = SpriteClipHistory::new();
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
    /// Prepares a blank slate for tracing a sprite's journey behind portals.
    ///
    /// Used heavily during initialization of the `Framebuffer` before we begin painting the
    /// solid columns of the world that will inevitably occlude our beloved cacodemons.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    ///
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

    /// Records a critical moment when a sprite is partially obscured by a portal opening.
    ///
    /// By logging the exact `depth` and `row` of the occlusion boundary, the software renderer
    /// can later correctly cut away the invisible parts of the sprite before plotting pixels to the screen.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep {
    ///     depth: 10.0,
    ///     row: 50,
    ///     silhouette_height: 20.0,
    /// });
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Peeks into the deepest and most recent portal boundary this column has pierced.
    ///
    /// Because we traverse the BSP from front to back, the "last" step pushed is always the furthest
    /// occlusion plane from the camera, allowing us to quickly determine if a sprite is fully hidden.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep {
    ///     depth: 10.0,
    ///     row: 50,
    ///     silhouette_height: 20.0,
    /// });
    /// assert!(history.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Walks back through the layered history of window cutouts.
    ///
    /// Useful when resolving complex multi-tiered sector heights where a sprite might be
    /// visible through an upper window but obscured by the solid wall directly beneath it.
    ///
    /// # Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep {
    ///     depth: 10.0,
    ///     row: 50,
    ///     silhouette_height: 20.0,
    /// });
    /// assert_eq!(history.iter().count(), 1);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
