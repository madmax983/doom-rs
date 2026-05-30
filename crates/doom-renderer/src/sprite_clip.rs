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
    /// Forges a pristine, empty history ledger, ready to record the clipping fate
    /// of a sprite column.
    ///
    /// In the chaotic render pipeline of Doom, where columns might be drawn behind
    /// multiple overlapping mid-textures or portals, this history acts as our memory.
    /// We start fresh for each new column traverse.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    ///
    /// // A blank slate for a newly cast sprite column ray.
    /// let history = SpriteClipHistory::new();
    /// assert_eq!(history.iter().count(), 0);
    /// assert_eq!(history.last(), None);
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

    /// Commits a new geometric clipping boundary to the memory banks.
    ///
    /// **The Fine Print:** Why silently drop clips if the array fills? Because in Doom's geometry,
    /// a single vertical column of pixels piercing through more than 8 layers of deep portals
    /// (like multiple 2S-linedefs or mid-textures) is extremely rare. Rather than forcing a costly
    /// heap allocation (`Vec::push`) mid-render loop just to support a microscopic edge case,
    /// we simply stop recording. The visual artifact is usually negligible, while the performance
    /// gain of using an 8-element stack-allocated array is immense.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// // The sprite column passes behind a window sill at depth 100.0.
    /// let step = SpriteClipStep { depth: 100.0, row: 10, silhouette_height: 50.0 };
    /// history.push(step);
    /// assert_eq!(history.last(), Some(&step));
    /// ```
    pub fn push(&mut self, step: SpriteClipStep) {
        if self.len < self.steps.len() {
            self.steps[self.len] = step;
            self.len += 1;
        } else {
            // Drop extra clips to avoid heap allocations
        }
    }

    /// Peeks at the most recent boundary we crossed.
    ///
    /// This is crucial for determining if the *next* piece of geometry we encounter
    /// is further away than the one we just passed, allowing us to build a Z-ordered
    /// silhouette without a full depth buffer.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// let step = SpriteClipStep { depth: 100.0, row: 10, silhouette_height: 50.0 };
    /// history.push(step);
    /// assert_eq!(history.last(), Some(&step));
    /// ```
    pub fn last(&self) -> Option<&SpriteClipStep> {
        if self.len > 0 {
            Some(&self.steps[self.len - 1])
        } else {
            None
        }
    }

    /// Unrolls the scroll, yielding every clipping step from nearest to farthest.
    ///
    /// When it finally comes time to paint a sprite pixel onto the screen, the renderer
    /// iterates through this history to ensure the pixel doesn't violate any of the recorded
    /// depth boundaries (e.g., drawing a distant Imp on top of a nearby wall).
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::sprite_clip::SpriteClipHistory;
    /// use doom_renderer::render::SpriteClipStep;
    ///
    /// let mut history = SpriteClipHistory::new();
    /// history.push(SpriteClipStep { depth: 100.0, row: 10, silhouette_height: 50.0 });
    /// history.push(SpriteClipStep { depth: 200.0, row: 20, silhouette_height: 100.0 });
    ///
    /// // The renderer processes these to mask out the sprite's pixels.
    /// let steps: Vec<_> = history.iter().collect();
    /// assert_eq!(steps.len(), 2);
    /// ```
    pub fn iter(&self) -> core::slice::Iter<'_, SpriteClipStep> {
        self.steps[..self.len].iter()
    }
}
